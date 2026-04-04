# ── Stage 1: Build the Rust engine ───────────────────────────────────────────
FROM rust:latest AS builder

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src/ ./src/

RUN cargo build --release

# ── Stage 2: Runtime — Python 3.11 + lichess-bot ─────────────────────────────
FROM python:3.11-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
        git \
    && rm -rf /var/lib/apt/lists/*

# Clone lichess-bot and install dependencies
RUN git clone --depth 1 https://github.com/lichess-bot-devs/lichess-bot /lichess-bot
RUN pip install --no-cache-dir -r /lichess-bot/requirements.txt

# Copy engine binary (Linux build, no .exe)
COPY --from=builder /build/target/release/limaubali /lichess-bot/engines/limaubali
RUN chmod +x /lichess-bot/engines/limaubali

# Copy config template and startup script
COPY deploy/config.yml   /lichess-bot/config.yml
COPY deploy/start.sh     /start.sh
RUN chmod +x /start.sh

WORKDIR /lichess-bot

CMD ["/start.sh"]
