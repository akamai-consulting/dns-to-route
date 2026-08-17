# ---------------------------------------------------------------------
# Stage 1: Compute a recipes file for dependency caching
# ---------------------------------------------------------------------
FROM rust:1.91-alpine AS planner
WORKDIR /app
RUN cargo install cargo-chef
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ---------------------------------------------------------------------
# Stage 2: Cache and build dependencies + application
# ---------------------------------------------------------------------
FROM rust:1.91-alpine AS builder
WORKDIR /app
RUN apk add --no-cache musl-dev
RUN cargo install cargo-chef

COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

COPY . .
RUN cargo build --release

# ---------------------------------------------------------------------
# Stage 3: Final Alpine runtime container
# ---------------------------------------------------------------------
FROM alpine:3.20 AS runtime

RUN apk add --no-cache ca-certificates tzdata

WORKDIR /app

COPY --from=builder /app/target/release/dns-to-route ./dns-to-route

ENV RUST_LOG=info
ENTRYPOINT ["./dns-to-route"]

