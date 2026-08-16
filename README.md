# VerdAnt Backend

Server APIs, business logic, data, and authentication for the VerdAnt ecosystem.

## Stack

- **Rust** · **Axum** · **PostgreSQL** · **SQLx**
- OpenAPI contract output consumed by `verdant-frontend` (canonical in `docs/api/`)
- Environment-based configuration, structured logging (tracing)

## Repository role

One of three VerdAnt repositories (`verdant-backend`, `verdant-frontend`,
`verdant-contracts`). See [`INSTRUCTIONS.md`](../INSTRUCTIONS.md) at the
coordination root for the master architecture and the Agent Responsibility
Table (Agent #1 owns this repository).

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

### Tests

```bash
cargo test
```

Integration tests use the local Postgres instance and the `DATABASE_URL`
from your environment. Test databases are created per-run and dropped on
completion.

### Lint / format

```bash
cargo fmt --check
cargo clippy -- -D warnings
```

## Project layout

```
src/
├── main.rs        # entrypoint: config, tracing, migrate, serve
├── lib.rs         # library root: app construction + router
├── config.rs      # typed environment-based configuration
├── error.rs       # uniform AppError -> HTTP responses
├── state.rs       # shared application state (DB pool, chain client)
├── farmers/       # Farmer domain (model, service, chain, hash, ids)
└── routes/        # HTTP route modules (health, farmers)
migrations/        # SQLx migrations
tests/             # integration tests
```

## Roadmap status

- [x] Phase 1: repository foundation, base tooling, health + DB smoke test
- [x] Shared identifier formats (accepted v1.0 in `docs/architecture/identifiers.md`, AD-009)
- [x] Phase 2: shared cross-repo contracts (Agent #4 published Farmer API)
- [x] Phase 3: core agricultural identity — Farmer domain (REST API, metadata hashing, DB schema)