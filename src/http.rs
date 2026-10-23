use crate::{metrics::Metrics, minifier::Minifier};
use axum::{
    body::Bytes,
    extract::Request,
    http::{HeaderMap, StatusCode, header},
    middleware::Next,
    response::Response,
    routing::{MethodRouter, post},
};
use mime::Mime;
use std::sync::Arc;
use tokio::{sync::Semaphore, task::spawn_blocking};
use tracing::{Instrument, info, info_span, warn};
use uuid::Uuid;

pub async fn request_context(req: Request, next: Next) -> Response {
    let request_id = req
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .map(String::from)
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    let span = info_span!(
        "http",
        request_id = %request_id,
    );

    let mut res = next.run(req).instrument(span).await;
    res.headers_mut()
        .insert("x-request-id", request_id.parse().unwrap());
    res
}

pub fn minify_route<M>(
    minifier: M,
    mtype: &'static str,
    limit: Arc<Semaphore>,
    metrics: Arc<Metrics>,
) -> MethodRouter
where
    M: Minifier + Clone + Send + Sync + 'static,
{
    post(move |headers: HeaderMap, body: Bytes| {
        let minifier = minifier.clone();
        let limit = Arc::clone(&limit);
        let metrics = Arc::clone(&metrics);
        async move {
            let outcome = async {
                let raw_mime = match headers
                    .get(header::CONTENT_TYPE)
                    .and_then(|v| v.to_str().ok())
                {
                    Some(m) => m,
                    None => {
                        warn!("missing content type");
                        return Err(with_reason(StatusCode::BAD_REQUEST));
                    }
                };
                let parsed_mime = match raw_mime.parse::<Mime>() {
                    Ok(m) => m,
                    Err(e) => {
                        warn!(error = e.to_string(), "cannot parse content type");
                        return Err(with_reason(StatusCode::BAD_REQUEST));
                    }
                };
                if parsed_mime.essence_str() != mtype {
                    warn!("request content type invalid");
                    return Err(with_reason(StatusCode::UNSUPPORTED_MEDIA_TYPE));
                }
                if let Some(charset) = parsed_mime.get_param(mime::CHARSET)
                    && charset != mime::UTF_8
                {
                    warn!("charset is not utf-8");
                    return Err(with_reason(StatusCode::BAD_REQUEST));
                }
                let input = match String::from_utf8(body.into()) {
                    Ok(s) => s,
                    Err(e) => {
                        warn!(error = e.to_string(), "request body not in utf8");
                        return Err(with_reason(StatusCode::BAD_REQUEST));
                    }
                };
                let input_len = input.len();
                metrics.observe_input(mtype, input_len);

                let perm = Arc::clone(&limit).acquire_owned().await.unwrap();
                let started = std::time::Instant::now();
                let joined = spawn_blocking(move || {
                    let _perm = perm;
                    minifier.minify(&input)
                })
                .await;
                metrics.observe_duration(mtype, started.elapsed().as_secs_f64());
                let res = match joined {
                    Ok(r) => r,
                    Err(e) => {
                        warn!(error = e.to_string(), "error while minifying");
                        return Err(with_reason(StatusCode::INTERNAL_SERVER_ERROR));
                    }
                };

                match res {
                    Ok(r) => {
                        metrics.observe_output(mtype, r.len());
                        info!(
                            media_type = mtype,
                            input = input_len,
                            output = r.len(),
                            "minified"
                        );
                        Ok(([(header::CONTENT_TYPE, mtype)], r))
                    }
                    Err(e) => {
                        warn!(error = e.to_string(), "error while minifying");
                        Err(with_reason(StatusCode::UNPROCESSABLE_ENTITY))
                    }
                }
            }
            .await;

            let status = match &outcome {
                Ok(_) => StatusCode::OK,
                Err((status, _)) => *status,
            };
            metrics.record_request(mtype, status.as_str());
            outcome
        }
    })
}

fn with_reason(status: StatusCode) -> (StatusCode, String) {
    (
        status,
        match status.canonical_reason() {
            Some(s) => s.to_string(),
            None => "Unknown error".to_string(),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::minifier::Minifier;
    use anyhow::Result;
    use axum::{
        Router,
        body::Body,
        http::{Request, StatusCode, header},
    };
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    #[derive(Clone)]
    struct PanicMinifier;

    impl Minifier for PanicMinifier {
        fn minify(&self, _input: &str) -> Result<String> {
            panic!("minify failed");
        }
    }

    #[tokio::test]
    async fn panicking_minifier_is_internal_error() {
        let metrics = Metrics::new();
        let limit = Arc::new(Semaphore::new(1));
        let app = Router::new().route(
            "/css",
            minify_route(
                PanicMinifier,
                "text/css",
                Arc::clone(&limit),
                metrics.clone(),
            ),
        );
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/css")
                    .header(header::CONTENT_TYPE, "text/css")
                    .body(Body::from("body{color:red}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(body.as_ref(), b"Internal Server Error");
        let rendered = metrics.render(&limit);
        assert!(
            rendered
                .contains("wymmd_minify_requests_total{media_type=\"text/css\",status=\"500\"} 1"),
            "{rendered}"
        );
    }
}
