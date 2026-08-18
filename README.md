# VerdAnt Backend — Project Proposal

**Server APIs, business logic, data, and authentication for the VerdAnt
ecosystem — open agricultural technology & financial infrastructure built on
Stellar/Soroban.**

**Document status:** 2026-08-18 · Revision 2
**Owner:** Agent #1 (Backend Engineer)
**Part of:** the VerdAnt three-repository system (this repo, `verdant-contracts`,
`verdant-frontend`).

---

## 1. Background

VerdAnt anchors farm identity, verification, equipment leasing, financing, and
livestock provenance on the Stellar blockchain. For that vision to serve real
users, there must be a reliable, well-defined server layer: one that authenticates
wallets, serves the farmer domain, and turns raw on-chain contract activity into
queryable off-chain projections. This repository is that layer.

## 2. Objectives

1. Provide a typed, documented REST API for the farmer domain (AgriScout
   identity) with pagination and search.
2. Authenticate Stellar wallets via SEP-40 (challenge / signature verification /
   bearer sessions) and gate farmer routes on that identity.
3. Ingest Soroban contract events and rebuild off-chain projections (identity,
   verification, escrow) for discovery and search.
4. Keep on-chain/off-chain responsibilities separated: integrity-sensitive state
   stays on-chain; the backend manages data, search, and API presentation.

## 3. Scope

**In scope.** Farmer registration/profile/search, SEP-40 wallet authentication,
bearer-token session management, event indexing and projections, PostgreSQL
schema, health/ops endpoints, OpenAPI contract output, CI.

**Out of scope.** On-chain state (handled by `verdant-contracts`); frontend
surfaces (handled by `verdant-frontend`); a live Soroban RPC subscriber — the
indexer currently runs on a `StubEventSource` until the real chain is live.

## 4. Proposed solution & architecture

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
  verification, escrow), with a finality cutoff and re-org rewind.

**Stack.** Rust (edition 2021) · Axum · tokio · PostgreSQL · SQLx
(compile-time-checked SQL) · tracing · utoipa (OpenAPI) · stellar-strkey ·
ed25519-dalek + base64 (SEP-40 signature verification).

## 5. Deliverables

### 5.1 Delivered

- **Farmer REST API v1.1** — register / get / update / search (AD-010:
  `q`, `page`, `pageSize`), with GIN/trigram search indexes.
- **SEP-40 wallet auth** — `POST /api/v1/auth/challenge`, `POST
  /api/v1/auth/verify`, `GET /api/v1/auth/session`; bearer sessions gated by the
  `AuthUser` axum extractor on farmer routes.
- **Event indexer foundation** — migration `0005_indexer.sql`, `ChainEvents`
  trait, identity/verification/escrow projections, idempotent re-ingest, re-org
  rewind.
- **PostgreSQL schema** — migrations 0001–0005 covering baseline, farmers,
  search indexes, auth, and the dedicated `indexer` schema.
- **Health endpoint** and uniform `AppError` → HTTP error mapping.

### 5.2 Planned

- Real Soroban RPC subscriber behind `ChainEvents` (deferred until the real
  chain is live).
- Projection → API read endpoints (awaiting frontend contracts).
- Financing projection (awaiting contract deployment).
- Publish OpenAPI output to the coordination `docs/api/` tree.

## 6. Design constraints & standards

- **On-chain/off-chain boundary (AD-004).** Only integrity-sensitive state goes
  on-chain; documents/media stay off-chain referenced by sha256 hashes.
- **Identifiers (AD-009).** Backend renders contract-issued IDs as 12-digit
  zero-padded `u64` counters (`va:verification:000000000042`) and issues its own
  UUIDv7 reference keys (`va:batch:`, `va:booking:`, `va:asset:`). On-chain keys
  are never `va:`-prefixed.
- **Auth.** SEP-40 signed-payload flow; `X-Verdant-Actor` header was dropped in
  favor of the `AuthUser` bearer-token middleware (no fallback).
- **Interface-first.** API contracts are documented in the coordination
  `docs/api/` tree before implementation and are the contract of record for the
  frontend. Changes require an agent note and, if architecturally significant,
  an architecture decision (AD).

## 7. Timeline / roadmap

| Phase | Scope | Status |
|-------|-------|--------|
| 1 | Repository foundation, tooling, health + DB smoke test | Done |
| — | Shared identifier formats (AD-009) | Done |
| 2 | Shared cross-repo API contracts v1.1 | Done |
| 3 | Farmer REST API (register/get/update/search), metadata hashing, schema | Done |
| — | SEP-40 wallet auth + `AuthUser` middleware on farmer routes | Done |
| 9 | Event indexer foundation (migration 0005, `ChainEvents`, projections) | Done |
| 9 | Live Soroban RPC subscriber | Pending |
| — | Projection read endpoints | Pending |
| — | Financing projection | Pending |

## 8. Development & operations

### Local development

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

### Configuration

| Variable | Default | Purpose |
|----------|---------|---------|
| `VERDANT_BACKEND_HOST` | `127.0.0.1` | Bind host |
| `VERDANT_BACKEND_PORT` | `8080` | Bind port |
| `VERDANT_BACKEND_LOG_LEVEL` | `info` | tracing filter |
| `DATABASE_URL` | — | PostgreSQL connection string |
| `VERDANT_BACKEND_DOMAIN` | — | Domain used in SEP-40 challenge messages |
| `VERDANT_BACKEND_SESSION_TTL_SECS` | — | Bearer session TTL (seconds) |

### Tests

Integration tests need a dedicated database and CI uses a serial test runner to
avoid cross-binary interference:

```bash
createdb verdant_backend_test
DATABASE_URL=postgres://postgres@127.0.0.1:5432/verdant_backend_test \
  cargo test --all-targets -- --test-threads=1
```

Current suite: **50 tests green** — 18 lib unit (incl. indexer decoders/chain/
ids), 8 auth, 17 farmers, 2 health, 5 indexer integration. Indexer coverage:
verification finality + cutoff, escrow accumulate/release/refund, identity
upsert, idempotent re-ingest, and re-org rewind.

### Lint / format / CI

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

GitHub Actions (`ci.yml`): Postgres service container, `sqlx migrate run`, then
`fmt --check`, `clippy -D warnings`, and `cargo test --all-targets --
--test-threads=1`.

## 9. Project layout

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

## 10. Ownership

Owned and maintained by **Agent #1 (Backend Engineer)** as part of the VerdAnt
program. Interface contracts are coordinated through the program's integration
lead (Agent #4) and recorded in the coordination root.