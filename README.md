# VerdAnt Backend

**Server APIs, business logic, data, and authentication for the VerdAnt
ecosystem — open agricultural technology & financial infrastructure built on
Stellar/Soroban.**

This repository implements the farmer domain (AgriScout identity), SEP-40
wallet authentication, and the off-chain event indexer that rebuilds
projections from Soroban contract events.

## Prerequisites

- Rust (stable)
- PostgreSQL running locally
- `cargo-sqlx` (`cargo install sqlx-cli`) for migrations

## Setup

```bash
# 1. Create the database
createdb verdant_backend

# 2. Configure environment
cp .env.example .env

# 3. Run migrations
cargo sqlx migrate run --source migrations

# 4. Start the server
cargo run
```

Open http://127.0.0.1:8080/health to confirm the server is up.

## Scripts

| Command | Purpose |
|---------|---------|
| `cargo run` | Start the server |
| `cargo sqlx migrate run --source migrations` | Apply database migrations |
| `cargo test` | Run tests |
| `cargo fmt --check` | Check formatting |
| `cargo clippy --all-targets -- -D warnings` | Lint with warnings as errors |

## Architecture

```
                    ┌─────────────────────────────────────────────┐
 HTTP clients ─────▶│  Axum router (src/routes/)                  │
 (frontend, CLI)    │  ├── /health                                │
                    │  ├── /api/v1/auth/*   (SEP-40)              │
                    │  └── /api/v1/farmers/*                       │
                    └───────────┬─────────────────────────────────┘
                                ▼
                    ┌─────────────────────────────────────────────┐
                    │  Service layer (src/*/service.rs)           │
                    │  auth: challenge / verify / session         │
                    │  farmers: register / get / update / search  │
                    │  indexer: ingest (raw events → projections) │
                    └───────────┬─────────────────────────────────┘
                                ▼
              ┌─────────────────┴──────────────────┐
              ▼                                    ▼
   ┌────────────────────────┐          ┌──────────────────────────┐
   │  PostgreSQL             │          │  On-chain (via traits)    │
   │  farmers, auth_challenges│         │  ChainEvents / IdentityChain
   │  auth_sessions,         │          │  (StubChain / StubEventSource
   │  indexer.* tables       │          │   until the real chain)   │
   └────────────────────────┘          └──────────────────────────┘
```

**Data flow.**

- Farmer registration hashes metadata (sha256, AD-004), calls the identity
  contract through the `IdentityChain` trait (`StubChain` in development), and
  persists an off-chain `farmers` record keyed by the Stellar public key
  (AD-005/AD-009).
- SEP-40 auth issues single-use challenges, verifies Stellar `signMessage`
  signatures, and creates bearer-token sessions (`auth_sessions`). Farmer routes
  authenticate via the `AuthUser` extractor.
- The event indexer ingests Soroban contract events into an append-only
  `indexer.indexed_events` store and rebuilds derived projections (identity,
  verification, escrow, financing), with a finality cutoff and re-org rewind.

## Route map

| Route | Method | Purpose |
|-------|--------|---------|
| `/health` | GET | Health check |
| `/api/v1/auth/challenge` | POST | Issue a SEP-40 challenge `{ address }` → `{ domain, nonce, timestamp, address }` |
| `/api/v1/auth/verify` | POST | Verify a Stellar signature → `{ token, address, roles, expires_at }` |
| `/api/v1/auth/session` | GET | Fetch the session for a bearer token |
| `/api/v1/farmers` | GET | Search farmers (AD-010: `q`, `page`, `pageSize`) |
| `/api/v1/farmers/:address` | GET | Farmer profile |
| `/api/v1/farmers/register` | POST | Register a farmer |
| `/api/v1/farmers/:address/metadata` | PUT | Update farmer metadata |

Farmer routes require a bearer token (SEP-40 session). The canonical contract
for these endpoints is `docs/api/farmers.md` in the coordination root.

## Features

### SEP-40 wallet authentication

The backend implements the SEP-40 signed-payload flow:

1. `POST /api/v1/auth/challenge` issues a single-use challenge containing the
   domain, a nonce, and a timestamp.
2. The wallet signs the challenge message (built by `sep40_message`).
3. `POST /api/v1/auth/verify` verifies the ed25519 signature and returns a
   bearer token.
4. Farmer routes authenticate via the `AuthUser` axum extractor, which resolves
   the bearer token against `auth_sessions`.

### Farmer API (AgriScout)

Register, read, update, and search farmers. Metadata is hashed (sha256, AD-004)
before any on-chain call. Search uses GIN/trigram indexes (AD-010) with
pagination.

### Event indexer

Ingests Soroban contract events and rebuilds off-chain projections:

- Raw events appended idempotently to `indexer.indexed_events`.
- Finality cutoff (10 ledgers) before projection apply.
- Re-org rewind drops events from the divergent ledger onward and resumes.
- Projections: identity, verification, escrow, financing.

## Configuration

Environment variables (see `.env.example`):

| Variable | Default | Purpose |
|----------|---------|---------|
| `VERDANT_BACKEND_HOST` | `127.0.0.1` | Bind host |
| `VERDANT_BACKEND_PORT` | `8080` | Bind port |
| `VERDANT_BACKEND_LOG_LEVEL` | `info` | tracing filter |
| `DATABASE_URL` | — | PostgreSQL connection string |
| `VERDANT_BACKEND_DOMAIN` | — | Domain used in SEP-40 challenge messages |
| `VERDANT_BACKEND_SESSION_TTL_SECS` | — | Bearer session TTL (seconds) |

## Database schema (migrations)

| Migration | Content |
|-----------|---------|
| `0001_baseline.sql` | Baseline / platform tables |
| `0002_farmers.sql` | `farmers` table (on-chain fields + metadata block) |
| `0003_farmer_search_indexes.sql` | GIN/trigram indexes for AD-010 search |
| `0004_auth.sql` | `auth_challenges`, `auth_sessions` |
| `0005_indexer.sql` | `indexer` schema: `indexed_events`, `indexer_cursors`, verification/escrow projections |
| `0006_financing_projection.sql` | `indexer.financing_projection` table + indexes |

Indexer tables live in a dedicated `indexer` schema, separate from the `public`
API tables.

## Tests

Integration tests need a dedicated database, and CI uses a serial test runner
to avoid cross-binary interference:

```bash
createdb verdant_backend_test
DATABASE_URL=postgres://postgres@127.0.0.1:5432/verdant_backend_test \
  cargo test --all-targets -- --test-threads=1
```

Current suite: **53 tests green** — lib unit (incl. indexer decoders/chain/
ids), auth, farmers, health, and indexer integration. Indexer coverage:
verification finality + cutoff, escrow accumulate/release/refund, financing
drawdown/default, identity upsert, idempotent re-ingest, and re-org rewind.

## Lint / format

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

## CI/CD

GitHub Actions (`.github/workflows/ci.yml`): a Postgres service container,
`sqlx migrate run`, then `cargo fmt --check`, `cargo clippy --all-targets -- -D
warnings`, and `cargo test --all-targets -- --test-threads=1`.

## Project layout

```
src/
├── main.rs            # binary entrypoint: config, tracing, migrate, serve
├── lib.rs             # library root: app construction, router, migrate/connect
├── config.rs          # typed environment-based configuration
├── error.rs           # uniform AppError -> HTTP error responses
├── state.rs           # shared AppState (DB pool, chain/indexer clients)
├── ids.rs             # shared AD-009 counter rendering (va:verification:<12>, parse)
├── auth/              # SEP-40 wallet authentication
│   ├── model.rs       #   challenge/session row types
│   ├── service.rs     #   issue_challenge, verify, session_by_token, sep40_message
│   ├── extractor.rs   #   AuthUser (bearer-token) axum extractor
│   └── mod.rs
├── farmers/           # Farmer domain (AgriScout identity)
│   ├── model.rs       #   Farmer, RegisterFarmerRequest, UpdateMetadataRequest
│   ├── service.rs     #   register/get/update/search against DB + chain
│   ├── chain.rs       #   IdentityChain trait + StubChain (dev)
│   ├── hash.rs        #   metadata hashing (sha256, AD-004)
│   ├── ids.rs         #   farmer id rendering (va:farmer:<G…>)
│   └── mod.rs
├── indexer/           # Phase 9 event indexer
│   ├── chain.rs       #   ChainEvents trait + StubEventSource
│   ├── model.rs       #   ChainEvent, Cursor, projection row types
│   ├── store.rs       #   idempotent raw-event append, cursors, re-org rewind
│   ├── service.rs     #   ingest (finality cutoff, projection apply)
│   ├── projections.rs #   identity / verification / escrow / financing builders
│   └── mod.rs
└── routes/            # HTTP route modules
    ├── health.rs      #   GET /health
    ├── auth.rs        #   POST /api/v1/auth/challenge|verify, GET .../session
    ├── farmers.rs     #   farmer REST + search endpoints
    └── mod.rs
migrations/            # SQLx migrations (0001..0006)
tests/                 # integration tests (auth, farmers, health, indexer)
```

## Contributing

1. Fork the repo and create a branch from `main`.
2. Install deps, run migrations, and verify: `cargo fmt --check`, `cargo clippy
   --all-targets -- -D warnings`, and the test suite.
3. Open a pull request. CI runs format, lint, and tests on push/PR to `main`.

## License

Apache License 2.0. See the `LICENSE` file.