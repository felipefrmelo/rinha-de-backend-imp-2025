# syntax=docker/dockerfile:1

ARG RUST_VERSION=1.88.0

################################################################################
# Create a stage for building the application.

FROM rust:${RUST_VERSION}-alpine AS build
WORKDIR /app

# Install host build dependencies.

RUN apk add --no-cache clang lld musl-dev git pkgconfig openssl-dev openssl-libs-static 

# Build all applications.
# Copy the entire workspace and leverage cache mounts for dependencies
COPY . .

RUN --mount=type=cache,target=/app/target/ \
    --mount=type=cache,target=/usr/local/cargo/git/db \
    --mount=type=cache,target=/usr/local/cargo/registry/ \
    cargo build --locked --release && \
    cp ./target/release/health-checker /bin/health-checker && \
    cp ./target/release/api /bin/api && \
    cp ./target/release/payment-worker /bin/payment-worker

################################################################################
# Stage for health-checker service

FROM alpine:3.18 AS health-checker
RUN apk add --no-cache openssl ca-certificates

ARG UID=10001
RUN adduser \
    --disabled-password \
    --gecos "" \
    --home "/nonexistent" \
    --shell "/sbin/nologin" \
    --no-create-home \
    --uid "${UID}" \
    appuser
USER appuser

COPY --from=build /bin/health-checker /bin/
LABEL service=health-checker
CMD ["/bin/health-checker"]

################################################################################
# Stage for api service

FROM alpine:3.18 AS api
RUN apk add --no-cache openssl ca-certificates

ARG UID=10001
RUN adduser \
    --disabled-password \
    --gecos "" \
    --home "/nonexistent" \
    --shell "/sbin/nologin" \
    --no-create-home \
    --uid "${UID}" \
    appuser
USER appuser

COPY --from=build /bin/api /bin/
EXPOSE 3000
LABEL service=api
CMD ["/bin/api"]

################################################################################
# Stage for payment-worker service

FROM alpine:3.18 AS payment-worker
RUN apk add --no-cache openssl ca-certificates

ARG UID=10001
RUN adduser \
    --disabled-password \
    --gecos "" \
    --home "/nonexistent" \
    --shell "/sbin/nologin" \
    --no-create-home \
    --uid "${UID}" \
    appuser
USER appuser

COPY --from=build /bin/payment-worker /bin/
LABEL service=payment-worker
CMD ["/bin/payment-worker"]
