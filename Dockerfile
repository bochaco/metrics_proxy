FROM rust:1-alpine AS builder
WORKDIR /
COPY . .

RUN apk add pkgconfig openssl-dev libc-dev

RUN cargo build --release
RUN ls -la /target/release

FROM alpine AS runtime
WORKDIR /app

COPY --from=builder /target/release/metrics_proxy /app/
RUN ls -la /app/

EXPOSE 8080

CMD ["/app/metrics_proxy"]
