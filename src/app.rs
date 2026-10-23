use crate::{
    css::SwcCssMinifier,
    http::{minify_route, request_context},
    js::SwcJsMinifier,
    metrics::Metrics,
};
use axum::{
    Router,
    extract::DefaultBodyLimit,
    http::{StatusCode, header},
    routing::get,
};
use std::sync::Arc;
use tokio::sync::Semaphore;

const DEFAULT_MAX_BODY_SIZE: usize = 1024 * 1024 * 3;

pub(crate) fn parse_max_body_size(value: Option<&str>) -> usize {
    value
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_MAX_BODY_SIZE)
}

pub(crate) fn app(max_body_size: usize) -> Router {
    let limit = Arc::new(Semaphore::new(
        std::thread::available_parallelism().unwrap().get(),
    ));
    let metrics = Metrics::new();
    Router::new()
        .route("/healthz", get(async || StatusCode::OK))
        .route(
            "/metrics",
            get({
                let metrics = metrics.clone();
                let limit = limit.clone();
                async move || {
                    (
                        [(header::CONTENT_TYPE, "text/plain; version=0.0.4")],
                        metrics.render(&limit),
                    )
                }
            }),
        )
        .nest(
            "/v1",
            Router::new()
                .route(
                    "/js",
                    minify_route(
                        SwcJsMinifier,
                        "text/javascript",
                        limit.clone(),
                        metrics.clone(),
                    ),
                )
                .route(
                    "/css",
                    minify_route(SwcCssMinifier, "text/css", limit, metrics),
                ),
        )
        .layer(DefaultBodyLimit::max(max_body_size))
        .layer(axum::middleware::from_fn(request_context))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{css::SwcCssMinifier, js::SwcJsMinifier, minifier::Minifier};
    use axum::{
        body::Body,
        http::{Request, StatusCode, header},
    };
    use http_body_util::BodyExt;
    use tower::ServiceExt;
    use uuid::Uuid;

    const LIMIT: usize = 1024 * 1024;

    struct Resp {
        status: StatusCode,
        content_type: Option<String>,
        request_id: Option<String>,
        body: String,
    }

    async fn call(
        max_body_size: usize,
        method: &str,
        uri: &str,
        content_type: Option<&str>,
        request_id: Option<&str>,
        body: impl Into<Body>,
    ) -> Resp {
        let mut builder = Request::builder().method(method).uri(uri);
        if let Some(content_type) = content_type {
            builder = builder.header(header::CONTENT_TYPE, content_type);
        }
        if let Some(request_id) = request_id {
            builder = builder.header("x-request-id", request_id);
        }
        let response = app(max_body_size)
            .oneshot(builder.body(body.into()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let request_id = response
            .headers()
            .get("x-request-id")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        Resp {
            status,
            content_type,
            request_id,
            body: String::from_utf8_lossy(&bytes).into_owned(),
        }
    }

    #[test]
    fn default_body_size_is_three_mebibytes() {
        assert_eq!(parse_max_body_size(None), DEFAULT_MAX_BODY_SIZE);
        assert_eq!(parse_max_body_size(Some("")), DEFAULT_MAX_BODY_SIZE);
        assert_eq!(parse_max_body_size(Some("nope")), DEFAULT_MAX_BODY_SIZE);
        assert_eq!(DEFAULT_MAX_BODY_SIZE, 3 * 1024 * 1024);
    }

    #[test]
    fn parses_numeric_body_size() {
        assert_eq!(parse_max_body_size(Some("1024")), 1024);
        assert_eq!(parse_max_body_size(Some("0")), 0);
    }

    #[tokio::test]
    async fn minifies_css() {
        let src = "body { color: red; }";
        let res = call(LIMIT, "POST", "/v1/css", Some("text/css"), None, src).await;
        assert_eq!(res.status, StatusCode::OK);
        assert_eq!(res.content_type.as_deref(), Some("text/css"));
        assert_eq!(res.body, SwcCssMinifier.minify(src).unwrap());
        Uuid::parse_str(res.request_id.as_deref().unwrap()).unwrap();
    }

    #[tokio::test]
    async fn minifies_js() {
        let src = "console.log(1 + 2);";
        let res = call(LIMIT, "POST", "/v1/js", Some("text/javascript"), None, src).await;
        assert_eq!(res.status, StatusCode::OK);
        assert_eq!(res.content_type.as_deref(), Some("text/javascript"));
        assert_eq!(res.body, SwcJsMinifier.minify(src).unwrap());
    }

    #[tokio::test]
    async fn accepts_utf8_charset() {
        let src = "body { color: red; }";
        let res = call(
            LIMIT,
            "POST",
            "/v1/css",
            Some("text/css; charset=utf-8"),
            None,
            src,
        )
        .await;
        assert_eq!(res.status, StatusCode::OK);
        assert_eq!(res.body, SwcCssMinifier.minify(src).unwrap());
    }

    #[tokio::test]
    async fn rejects_missing_content_type() {
        let res = call(LIMIT, "POST", "/v1/css", None, Some("req-1"), "body{}").await;
        assert_eq!(res.status, StatusCode::BAD_REQUEST);
        assert_eq!(res.body, "Bad Request");
        assert_eq!(res.request_id.as_deref(), Some("req-1"));
    }

    #[tokio::test]
    async fn rejects_unparseable_content_type() {
        let res = call(LIMIT, "POST", "/v1/css", Some("%%%"), None, "body{}").await;
        assert_eq!(res.status, StatusCode::BAD_REQUEST);
        assert_eq!(res.body, "Bad Request");
    }

    #[tokio::test]
    async fn rejects_non_utf8_charset() {
        let res = call(
            LIMIT,
            "POST",
            "/v1/css",
            Some("text/css; charset=iso-8859-1"),
            None,
            "body{}",
        )
        .await;
        assert_eq!(res.status, StatusCode::BAD_REQUEST);
        assert_eq!(res.body, "Bad Request");
    }

    #[tokio::test]
    async fn rejects_non_utf8_body() {
        let res = call(
            LIMIT,
            "POST",
            "/v1/js",
            Some("text/javascript"),
            None,
            vec![0xff, 0xfe],
        )
        .await;
        assert_eq!(res.status, StatusCode::BAD_REQUEST);
        assert_eq!(res.body, "Bad Request");
    }

    #[tokio::test]
    async fn rejects_wrong_content_type() {
        for (uri, content_type) in [
            ("/v1/css", "application/json"),
            ("/v1/css", "application/x-www-form-urlencoded"),
            ("/v1/css", "text/javascript"),
            ("/v1/js", "text/css"),
        ] {
            let res = call(LIMIT, "POST", uri, Some(content_type), None, "body{}").await;
            assert_eq!(
                res.status,
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "{uri} {content_type}"
            );
            assert_eq!(res.body, "Unsupported Media Type");
        }
    }

    #[tokio::test]
    async fn recovers_unclosed_css_rule() {
        let src = "body {";
        let css = call(LIMIT, "POST", "/v1/css", Some("text/css"), None, src).await;
        assert_eq!(css.status, StatusCode::OK);
        assert_eq!(css.body, SwcCssMinifier.minify(src).unwrap());
    }

    #[tokio::test]
    async fn rejects_malformed_js() {
        let js = call(
            LIMIT,
            "POST",
            "/v1/js",
            Some("text/javascript"),
            None,
            "function (",
        )
        .await;
        assert_eq!(js.status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(js.body, "Unprocessable Entity");
    }

    #[tokio::test]
    async fn rejects_methods_other_than_post() {
        for (method, uri) in [
            ("GET", "/v1/css"),
            ("PUT", "/v1/css"),
            ("DELETE", "/v1/css"),
            ("GET", "/v1/js"),
            ("PUT", "/v1/js"),
            ("DELETE", "/v1/js"),
            ("POST", "/healthz"),
            ("POST", "/metrics"),
        ] {
            let res = call(LIMIT, method, uri, Some("text/css"), None, "body{}").await;
            assert_eq!(res.status, StatusCode::METHOD_NOT_ALLOWED, "{method} {uri}");
            assert!(res.request_id.is_some(), "{method} {uri}");
        }
    }

    #[tokio::test]
    async fn rejects_payload_over_the_limit() {
        let res = call(4, "POST", "/v1/css", Some("text/css"), None, "body{}").await;
        assert_eq!(res.status, StatusCode::PAYLOAD_TOO_LARGE);
        assert!(res.request_id.is_some());
    }

    #[tokio::test]
    async fn metrics_is_ok() {
        let res = call(LIMIT, "GET", "/metrics", None, None, "").await;
        assert_eq!(res.status, StatusCode::OK);
        assert_eq!(
            res.content_type.as_deref(),
            Some("text/plain; version=0.0.4")
        );
        assert!(res.body.contains("tokio_workers"));
        #[cfg(target_os = "linux")]
        assert!(res.body.contains("process_resident_memory_bytes"));
    }

    #[tokio::test]
    async fn metrics_count_minify_requests() {
        let app = app(LIMIT);
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/css")
                    .header(header::CONTENT_TYPE, "text/css")
                    .body(Body::from("body { color: red; }"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/metrics")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body = String::from_utf8_lossy(&body);
        assert!(
            body.contains("wymmd_minify_requests_total{media_type=\"text/css\",status=\"200\"} 1"),
            "{body}"
        );
        assert!(
            body.contains("wymmd_minify_input_bytes_count{media_type=\"text/css\"} 1"),
            "{body}"
        );
    }

    #[tokio::test]
    async fn healthz_is_ok() {
        let res = call(LIMIT, "GET", "/healthz", None, None, "").await;
        assert_eq!(res.status, StatusCode::OK);
        Uuid::parse_str(res.request_id.as_deref().unwrap()).unwrap();
    }

    #[tokio::test]
    async fn unknown_path_is_not_found() {
        let res = call(LIMIT, "GET", "/missing", None, None, "").await;
        assert_eq!(res.status, StatusCode::NOT_FOUND);
        assert!(res.request_id.is_some());
    }

    #[tokio::test]
    async fn echoes_request_id() {
        let res = call(
            LIMIT,
            "POST",
            "/v1/css",
            Some("text/css"),
            Some("req-123"),
            "body{}",
        )
        .await;
        assert_eq!(res.status, StatusCode::OK);
        assert_eq!(res.request_id.as_deref(), Some("req-123"));
    }
}
