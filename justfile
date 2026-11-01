set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

local_db := "postgres://hiring_record:local_demo_only@127.0.0.1:55432/hiring_record"

# Build the frontend, start PostgreSQL and run the demo on localhost:8080.
up: web db
    ./scripts/register-workflow.sh
    DATABASE_URL={{local_db}} RESTATE_INGRESS=http://127.0.0.1:19080 cargo run -p record-api

# Run the same experience without Docker; sessions reset when the server stops.
memory: web
    cargo run -p record-api

web:
    if [ ! -d apps/web/node_modules ]; then npm ci --prefix apps/web; fi
    npm run build --prefix apps/web

db:
    docker compose -p hiring-record up -d --wait

# Stop this project’s containers; their data volumes are retained.
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

# Verify the real synthetic outbox and Restate workflow against the running stack.
workflow-smoke:
    RESTART_NORTHWIND=1 python3 scripts/workflow-smoke.py

# Bounded model check; requires Java and access to the Apalache distribution.
spec:
    npx --yes @informalsystems/quint@0.33.1 verify spec/record.qnt --invariant=safety --max-steps=20

e2e:
    npm run test:e2e --prefix apps/web

generate-api:
    npm run generate:api --prefix apps/web

model-traces:
    python3 scripts/validate-model-traces.py

sim-sweep seeds="10000" rounds="1":
    cargo run -q -p record-cli -- sweep {{seeds}} {{rounds}}
