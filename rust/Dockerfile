# Monica: the Rust server and the SPA in one image.
#   docker build -f rust/Dockerfile -t monica .
#   docker run -p 8080:8080 -e APP_KEY=… -v monica-data:/var/lib/monica monica
# Context: the repository root (the server embeds lang/ and serves the SPA).

## The SPA
FROM node:22 AS spa
WORKDIR /src
COPY package.json yarn.lock .yarnrc.yml ./
COPY .yarn ./.yarn
RUN corepack enable && HUSKY=0 yarn install --immutable
COPY index.html vite.config.js eslint.config.js ./
COPY public ./public
COPY resources ./resources
COPY lang ./lang
RUN yarn build

## The server
FROM rust:1 AS server
WORKDIR /src
ARG MONICA_VERSION=
ARG MONICA_COMMIT=
COPY lang ./lang
COPY rust ./rust
RUN mkdir -p config \
    && if [ -n "$MONICA_VERSION" ]; then echo "$MONICA_VERSION" > config/.version; fi \
    && if [ -n "$MONICA_COMMIT" ]; then echo "$MONICA_COMMIT" > config/.commit; fi \
    && cd rust && cargo build --release --locked

## The image
FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates tzdata \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --home /var/lib/monica monica \
    && mkdir -p /var/lib/monica && chown monica /var/lib/monica
COPY --from=server /src/rust/target/release/monica /usr/local/bin/monica
COPY --from=spa /src/dist /usr/share/monica/spa
ENV BIND=0.0.0.0:8080 \
    DATA_DIR=/var/lib/monica \
    SPA_DIR=/usr/share/monica/spa
USER monica
VOLUME /var/lib/monica
EXPOSE 8080
CMD ["monica", "serve"]
