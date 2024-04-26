FROM rustlang/rust:nightly-alpine as builder

WORKDIR /usr/src/polyfrost-api

# Use sparse registry because it is significantly faster and this already requires nightly
ENV CARGO_UNSTABLE_SPARSE_REGISTRY=true

# Install build tools
RUN apk add --no-cache g++

# Copy source files
COPY . .

# Build actual binary
RUN cargo build --package polyfrost_api --release --locked --all-features

# ---------------------------------------------------------------------------------------------

# :3 :3 :3
FROM alpine:3

COPY --from=builder /usr/src/polyfrost-api/target/release/polyfrost_api /usr/local/bin/polyfrost_api

# Use an unprivileged user
RUN adduser --home /nonexistent --no-create-home --disabled-password polyfrost-api
USER polyfrost-api

HEALTHCHECK --interval=10s --timeout=3s --retries=5 CMD wget --spider --q http://localhost:$PORT/ || exit 1

CMD ["polyfrost_api"]
