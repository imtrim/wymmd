# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-10-16

### Added

- `POST /v1/js` and `POST /v1/css` minification with SWC.
- `GET /healthz`.
- Media type and UTF-8 validation.
- Configurable body size limit.
- Concurrency capped at the number of CPUs.
- `x-request-id` propagation and JSON logs.
- Graceful shutdown on SIGINT and SIGTERM.
- Docker image running as a non-root user, published to GHCR from tags.

[unreleased]: https://github.com/imtrim/wymmd/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/imtrim/wymmd/releases/tag/v0.1.0
