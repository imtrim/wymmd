FROM rust:1.99.0-alpine3.24 AS build
WORKDIR /app
RUN mkdir src && echo "fn main() {}" > src/main.rs
COPY Cargo.toml Cargo.lock .
ENV CARGO_PROFILE_RELEASE_STRIP=symbols
RUN --mount=type=cache,target=/usr/local/cargo/registry \
  cargo build --release --locked
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
  touch src/main.rs && \
  cargo build --release --locked

FROM alpine:3.24 AS app
WORKDIR /app
COPY --from=build /app/target/release/wymmd .
USER nobody
LABEL org.opencontainers.image.description="HTTP service that minifies CSS and JavaScript"
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
  CMD wget -q -O /dev/null "http://127.0.0.1:${PORT:-3000}/healthz" || exit 1
CMD ["./wymmd"]
