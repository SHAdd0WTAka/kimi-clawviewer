# Multi-stage build for ClawViewer Enterprise
FROM rust:1.75-slim-bookworm AS builder

RUN apt-get update && apt-get install -y     libwebkit2gtk-4.0-dev     libappindicator3-dev     librsvg2-dev     patchelf     libssl-dev     pkg-config     && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/
COPY src-tauri/ ./src-tauri/

RUN cargo build --release --features headless

# Runtime image
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y     libwebkit2gtk-4.0-37     libappindicator3-1     ca-certificates     && rm -rf /var/lib/apt/lists/*     && apt-get clean

RUN groupadd -r clawviewer && useradd -r -g clawviewer clawviewer

WORKDIR /app
COPY --from=builder /app/target/release/clawviewer /usr/local/bin/
COPY --from=builder /app/target/release/*.so /usr/local/lib/ || true

RUN mkdir -p /var/lib/clawviewer /var/log/clawviewer &&     chown -R clawviewer:clawviewer /var/lib/clawviewer /var/log/clawviewer

USER clawviewer

EXPOSE 9090

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3     CMD curl -f http://localhost:9090/health || exit 1

ENTRYPOINT ["clawviewer"]
CMD ["--headless"]
