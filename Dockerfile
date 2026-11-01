FROM node:24-bookworm AS node
FROM rust:1.91.1-bookworm AS build
COPY --from=node /usr/local/bin/node /usr/local/bin/node
COPY --from=node /usr/local/lib/node_modules /usr/local/lib/node_modules
RUN ln -s ../lib/node_modules/npm/bin/npm-cli.js /usr/local/bin/npm && ln -s ../lib/node_modules/npm/bin/npx-cli.js /usr/local/bin/npx && rustup target add wasm32-unknown-unknown
RUN cargo install wasm-bindgen-cli --version 0.2.130 --locked
WORKDIR /src
COPY . .
RUN npm ci --ignore-scripts --prefix apps/web && cargo build --release -p record-wasm --target wasm32-unknown-unknown && wasm-bindgen target/wasm32-unknown-unknown/release/record_wasm.wasm --target web --out-dir apps/web/public/engine --out-name record_wasm && cd apps/web && npm exec next build && cd /src && cargo build --release -p record-api

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/* && useradd --uid 10001 --create-home app && mkdir -p /data && chown app:app /data
WORKDIR /app
COPY --from=build /src/target/release/record-api /app/record-api
COPY --from=build /src/apps/web/out /app/web
COPY --from=build /src/fixtures/role.json /app/fixtures/role.json
ENV BIND_ADDRESS=0.0.0.0:8080 RECORD_FIXTURE_PATH=/app/fixtures/role.json RECORD_WEB_ROOT=/app/web SIGNING_KEY_FILE=/data/signing.key
USER app
EXPOSE 8080
CMD ["/app/record-api"]
