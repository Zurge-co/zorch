FROM rust:1.96-bookworm AS planner
WORKDIR /app

RUN cargo install cargo-chef --locked

COPY . .

RUN cargo chef prepare --recipe-path recipe.json

FROM rust:1.96-bookworm AS builder
WORKDIR /app

RUN cargo install cargo-chef --locked

COPY --from=planner /app/recipe.json recipe.json

RUN cargo chef cook --release --recipe-path recipe.json

COPY . .

# SQLX_OFFLINE not needed: all queries use runtime sqlx::query() (not compile-time query! macros)
# If compile-time macros are added in future, set: ENV SQLX_OFFLINE=true
RUN cargo build --release --bin zorch-api

FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

RUN useradd --create-home --shell /bin/bash app

WORKDIR /app

COPY --from=builder /app/target/release/zorch-api /app/zorch-api

RUN chown app:app /app/zorch-api

USER app

EXPOSE 8080

ENTRYPOINT ["/app/zorch-api"]
