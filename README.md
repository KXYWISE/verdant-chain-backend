# VerdAnt Backend

Server APIs, business logic, data, and authentication for the **VerdAnt**
ecosystem — open agricultural technology & financial infrastructure built on
Stellar/Soroban. This repository is owned by **Agent #1 (Backend Engineer)**.

It implements the farmer domain (AgriScout identity), SEP-40 wallet
authentication, and the off-chain event indexer that rebuilds projections from
Soroban contract events. It is one of three VerdAnt repositories
(`verdant-backend`, `verdant-frontend`, `verdant-contracts`); the master
architecture and Agent Responsibility Table live in
[`INSTRUCTIONS.md`](../INSTRUCTIONS.md) at the coordination root.

## Table of contents

- [Stack](#stack)
- [Architecture overview](#architecture-overview)
- [Repository role & interface contracts](#repository-role--interface-contracts)
- [Project layout](#project-layout)
- [Database schema (migrations)](#database-schema-migrations)
- [Local development](#local-development)
- [Configuration](#configuration)
- [Tests](#tests)
- [Lint / format](#lint--format)
- [CI](#ci)
- [Roadmap status](#roadmap-status)

## Stack

- **Rust** (edition 2021) · **Axum** web framework · **tokio** async runtime
- **PostgreSQL** database · **SQLx** (compile-time-checked SQL, async)
- **tracing** structured logging · **utoipa** OpenAPI contract output
- **stellar-strkey** for Stellar key validation · **ed25519-dalek** +
  **base64** for SEP-40 signature verification
- OpenAPI contract output is consumed by `verdant-frontend`; the canonical
  contract of record lives in `docs/api/` at the coordination root.

## Architecture overview

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

**Data flow:**

- Farmer registration hashes metadata (sha256, AD-004), calls the identity
  contract (`IdentityChain` trait, `StubChain` in dev), and persists an
  off-chain `farmers` record keyed by the Stellar public key (AD-005/AD-009).
- SEP-40 auth issues single-use challenges, verifies Stellar
  `signMessage` signatures, and creates bearer-token sessions
  (`auth_sessions`). Farmer routes authenticate via the `AuthUser` extractor.
- The event indexer ingests Soroban contract events into an append-only
  `indexer.indexed_events` store and rebuilds derived projections (identity,
  verification, escrow) — see `docs/architecture/event-indexing.md`.

## Repository role & interface contracts

One of three VerdAnt repositories. See
[`INSTRUCTIONS.md`](../INSTRUCTIONS.md) for the master architecture and the
Agent Responsibility Table (Agent #1 owns this repository).

Cross-repository interface contracts:

- **API contracts** (canonical): [`docs/api/`](../docs/api/) — Farmer API v1.1
  in [`docs/api/farmers.md`](../docs/api/farmers.md).
- **Auth flow**: [`docs/architecture/auth-flow.md`](../docs/architecture/auth-flow.md).
- **Event indexer plan**: [`docs/architecture/event-indexing.md`](../docs/architecture/event-indexing.md).
- **Identifiers**: [`docs/architecture/identifiers.md`](../docs/architecture/identifiers.md)
  (AD-009).
- **On-chain/off-chain boundary**: [`docs/architecture/boundaries.md`](../docs/architecture/boundaries.md)
  (AD-004).

Changes to any interface require an agent note in
[`docs/agent-notes/`](../docs/agent-notes/) and, if architecturally
significant, an AD.

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
│   ├── projections.rs #   identity / verification / escrow builders
│   └── mod.rs
└── routes/            # HTTP route modules
    ├── health.rs      #   GET /health
    ├── auth.rs        #   POST /api/v1/auth/challenge|verify, GET .../session
    ├── farmers.rs     #   farmer REST + search endpoints
    └── mod.rs
migrations/            # SQLx migrations (0001..0005)
tests/                 # integration tests (auth, farmers, health, indexer)
```

## Database schema (migrations)

| Migration | Content |
|-----------|---------|
| `0001_baseline.sql` | Baseline / platform tables |
| `0002_farmers.sql` | `farmers` table (on-chain fields + metadata block) |
| `0003_farmer_search_indexes.sql` | GIN/trigram indexes for AD-010 search |
| `0004_auth.sql` | `auth_challenges`, `auth_sessions` |
| `0005_indexer.sql` | `indexer` schema: `indexed_events`, `indexer_cursors`, verification/escrow projections |

Indexer tables are in a dedicated `indexer` schema (per Agent #4 decision)
separate from the `public` API tables. Run `cargo sqlx migrate run` to apply.

## Local development

Prerequisites: Rust (stable), PostgreSQL running locally.

```bash
# 1. Create the database
createdb verdant_backend

# 2. Configure environment
cp .env.example .env

# 3. Run migrations
cargo sqlx migrate run --source migrations

# 4. Start the server
cargo run

# 5. Check health
curl http://127.0.0.1:8080/health
```

### For integration tests

Tests need a dedicated database (default per CI:
`verdant_backend_test`) and `DATABASE_URL` set:

```bash
createdb verdant_backend_test
DATABASE_URL=postgres://postgres@127.0.0.1:5432/verdant_backend_test cargo test
```

Because integration test binaries share the test database, run the CI
invocation (`--test-threads=1`) to avoid cross-binary interference:

```bash
DATABASE_URL=postgres://postgres@127.0.0.1:5432/verdant_backend_test \
  cargo test --all-targets -- --test-threads=1
```

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

## Tests

```bash
# Unit + integration (full, CI-equivalent)
DATABASE_URL=postgres://postgres@127.0.0.1:5432/verdant_backend_test \
  cargo test --all-targets -- --test-threads=1
```

Current suite: **50 tests green** — 18 lib unit (incl. indexer decoders/chain/
ids), 8 auth, 17 farmers, 2 health, 5 indexer integration. Indexer coverage:
verification finality + cutoff, escrow accumulate/release/refund, identity
upsert, idempotent re-ingest, and re-org rewind.

## Lint / format

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

## CI

GitHub Actions (`.github/workflows/ci.yml`): a Postgres service container,
`sqlx migrate run`, then `cargo fmt --check`, `cargo clippy --all-targets -- -D
warnings`, and `cargo test --all-targets -- --test-threads=1`.

## Roadmap status

- [x] Phase 1: repository foundation, base tooling, health + DB smoke test
- [x] Shared identifier formats (AD-009, `docs/architecture/identifiers.md`)
- [x] Phase 2: shared cross-repo contracts (`docs/api/farmers.md` v1.1)
- [x] Phase 3: Farmer domain — REST API (register/get/update), AD-010 search,
      metadata hashing, DB schema
- [x] SEP-40 wallet auth (challenge/verify/session) + `AuthUser` middleware on
      farmer routes
- [x] Phase 9 foundation: event indexer (migration 0005, `ChainEvents`,
      identity/verification/escrow projections)
- [ ] Phase 9 remaining: real Soroban RPC subscriber behind `ChainEvents`
      (deferred until the real chain is live)
- [ ] Projection → API read endpoints (awaiting frontend contracts)
- [ ] Financing projection (awaiting contract deployment)
- [ ] Publish OpenAPI output to `docs/api/`

Full roadmap and current phase focus: `INSTRUCTIONS.md` §14.
