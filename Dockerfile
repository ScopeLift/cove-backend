# -------- Builder Stage --------
FROM rust:latest AS builder
WORKDIR /app

# Install necessary packages.
#   - lld and clang are for linking.
#   - openssl and ca-certificates are needed to verify TLS certificates when
#     establishing HTTPS connections.
#   - curl is for downloading the forge binary.
#   - git is for downloading repos during verification.
RUN apt-get update && \
  apt-get install -y lld clang openssl ca-certificates curl git && \
  # Clean up.
  apt-get autoremove -y && \
  apt-get clean -y && \
  rm -rf /var/lib/apt/lists/*

# Install forge.
RUN curl -L https://foundry.paradigm.xyz | bash
ENV PATH="/root/.foundry/bin:${PATH}"
RUN foundryup

# Copy all files to our Docker image.
COPY . .

# Build the binary.
RUN cargo build --release

# -------- Runtime Stage --------
FROM debian:bullseye-slim AS runtime
WORKDIR /app

# Install necessary packages.
# This is similar but not identical to the builder stage's install.
RUN apt-get update && \
  apt-get install -y openssl ca-certificates git && \
  # Clean up.
  apt-get autoremove -y && \
  apt-get clean -y && \
  rm -rf /var/lib/apt/lists/*

# Copy the binary from the builder stage.
COPY --from=builder /app/target/release/cove cove
COPY --from=builder /root/.foundry/bin/forge /usr/local/bin/forge
COPY config config
ENV APP_ENVIRONMENT production

# Launch binary when `docker run` is executed.
ENTRYPOINT ["./cove"]
