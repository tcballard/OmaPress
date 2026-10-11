FROM rust:1.98-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo build --release --locked -p newsletter-service

FROM debian:bookworm-slim
ARG TARGETARCH
ARG AWS_CLI_VERSION=2.37.11
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl unzip \
    && case "$TARGETARCH" in arm64) arch=aarch64;; *) arch=x86_64;; esac \
    && curl --fail --location "https://awscli.amazonaws.com/awscli-exe-linux-${arch}-${AWS_CLI_VERSION}.zip" -o /tmp/aws.zip \
    && unzip -q /tmp/aws.zip -d /tmp && /tmp/aws/install \
    && rm -rf /tmp/aws /tmp/aws.zip /var/lib/apt/lists/* \
    && useradd --uid 10001 --create-home newsletter
COPY --from=build /src/target/release/newsletter-service /usr/local/bin/newsletter-service
USER newsletter
ENV NEWSLETTER_BIND=0.0.0.0:8787
EXPOSE 8787
ENTRYPOINT ["newsletter-service"]
