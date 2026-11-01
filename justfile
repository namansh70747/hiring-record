set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

local_db := "postgres://hiring_record:local_demo_only@127.0.0.1:55432/hiring_record"

# Build the frontend, start PostgreSQL and run the demo on localhost:8080.
up: web db
    DATABASE_URL={{local_db}} cargo run -p record-api

# Run the same experience without Docker; sessions reset when the server stops.
memory: web
    cargo run -p record-api

web:
    if [ ! -d apps/web/node_modules ]; then npm ci --prefix apps/web; fi
    npm run build --prefix apps/web

db:
    docker compose -p hiring-record up -d --wait

# Stop only this project's PostgreSQL container; its volume is retained.
down:
    docker compose -p hiring-record stop

test:
    cargo test --workspace
    npm run typecheck --prefix apps/web

# Run PostgreSQL-specific tests after `just db`.
test-db: db
    TEST_DATABASE_URL={{local_db}} cargo test -p record-api store::tests

play:
    cargo run -p record-cli -- play

sim seed="42" rounds="100":
    cargo run -p record-cli -- simulate {{seed}} {{rounds}}

# Run the end-to-end check against a running local server.
smoke:
    python3 scripts/smoke.py

# Compare WASM and API results against a running local server after building the web app.
wasm-parity:
    node scripts/wasm-parity.mjs
