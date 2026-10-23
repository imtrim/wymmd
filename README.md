# wymmd

HTTP service that minifies JavaScript and CSS. Version 1 of the API uses [SWC](https://swc.rs/) for both formats.

## Getting Started

These instructions get a copy of the project running on your local machine for development and testing. See [Deployment](#deployment) for notes on how to run the published image.

### Prerequisites

* [Docker](https://docs.docker.com/get-docker/), to run the published image.
* [Rust](https://www.rust-lang.org/) 1.99.0, to build and test from source. The same version is used in CI and in the Docker build image.

### Installing

Run the published image:

```sh
docker run -p 3000:3000 ghcr.io/imtrim/wymmd:0.1.0
```

Or build and run from source:

```sh
cargo run --release
```

The process listens on `0.0.0.0:3000` unless `PORT` is set.

### Usage

Send the source as the raw request body. The response body is the minified source, with the same media type.

```sh
curl -sS -X POST http://127.0.0.1:3000/v1/js \
  -H 'Content-Type: text/javascript' \
  --data-binary 'console.log(1 + 2);'
```

```sh
curl -sS -X POST http://127.0.0.1:3000/v1/css \
  -H 'Content-Type: text/css' \
  --data-binary 'body { color: red; }'
```

`charset` is optional. When it is present it must be `utf-8`, for example `text/css; charset=utf-8`.

### API reference

| Method | Path | Body |
| --- | --- | --- |
| `POST` | `/v1/js` | JavaScript, `Content-Type: text/javascript` |
| `POST` | `/v1/css` | CSS, `Content-Type: text/css` |
| `GET` | `/healthz` | none |
| `GET` | `/metrics` | none |

`x-request-id` is echoed on the response. If the request does not send one, the service generates a UUID.

| Status | When |
| --- | --- |
| 200 | Minified successfully. CSS recovers from some parse errors and still returns 200. |
| 400 | `Content-Type` is missing or cannot be parsed, `charset` is not `utf-8`, or the body is not UTF-8. |
| 405 | Method is not allowed for that path. |
| 413 | Body is larger than `MAX_BODY_SIZE`. |
| 415 | Media type does not match the path. |
| 422 | JavaScript does not parse. |
| 500 | Minification panicked. |

Minification runs on the blocking thread pool. The number of in-flight minify jobs is capped at the number of CPUs.

### Configuration

| Variable | Default | Meaning |
| --- | --- | --- |
| `PORT` | `3000` | TCP port. |
| `MAX_BODY_SIZE` | `3145728` | Maximum request body size, in bytes (3 MiB). A value that is missing or not an integer uses the default. |
| `RUST_LOG` | `info` | Tracing filter. |

Logs are one flat JSON object per event. `INFO` and more verbose levels go to stdout. `WARN` and `ERROR` go to stderr.

## Running the tests

```sh
cargo test --locked
```

`--locked` fails the run when `Cargo.lock` does not match `Cargo.toml`. CI runs the same command on pushes to `main` and on pull requests.

## Deployment

The image runs as `nobody`, so `PORT` must be 1024 or higher. The image `HEALTHCHECK` requests `GET /healthz`.

Images are published to `ghcr.io/imtrim/wymmd` only when a `v*.*.*` tag is pushed. A tag `vX.Y.Z` produces three image tags: `X.Y.Z`, `X.Y`, and `latest`.

## Built With

* [axum](https://github.com/tokio-rs/axum) - HTTP framework
* [tokio](https://tokio.rs/) - Async runtime
* [SWC](https://swc.rs/) - JavaScript (`swc_ecma_minifier`) and CSS (`swc_css_minifier`) minification
* [tracing](https://github.com/tokio-rs/tracing) - Structured logging

## Roadmap

* HTML minification.
* A `/v2` API with a per-format engine. CSS in `/v2` will use [Lightning CSS](https://lightningcss.dev/). Other formats will follow.

`/v1` stays on SWC. Its contract does not change when `/v2` is added.

## Contributing

Please read [CONTRIBUTING.md](CONTRIBUTING.md) for the code of conduct and the process for submitting pull requests.

## Versioning

We use [SemVer](https://semver.org/spec/v2.0.0.html). Versions are git tags of the form `vX.Y.Z`. Notable changes are recorded in [CHANGELOG.md](CHANGELOG.md). See the [tags on this repository](https://github.com/imtrim/wymmd/tags).

The tag must equal `version` in `Cargo.toml`. The release workflow compares them and does not publish the image when they differ.

## Authors

* **imtrim** - [imtrim](https://github.com/imtrim)

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.

## Acknowledgments

* [SWC](https://swc.rs/), which performs the minification.
