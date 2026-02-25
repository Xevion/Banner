# Rust Style Guide (Backend)

General principles in [STYLE.md](STYLE.md).

## Architecture

### Layer Rules

Strict layering for data integrity:

```
web/ (HTTP handlers)
  -> services/ (business logic, background tasks)
    -> data/ (database access, domain queries)
      -> DB (PostgreSQL via SQLx)
```

- **Web handlers** handle HTTP concerns: extract params, call services/data, return responses.
- **Services** contain business logic that spans multiple data modules or has side effects (scraping, notifications, external API calls).
- **Data modules** are the only code that touches the database. All SQL lives here.
- Web handlers may call data modules directly for simple reads. A service layer is required when logic spans multiple data modules or has side effects beyond a single query.

### Module Organization

```
src/
+-- banner/       # Banner API client (UTSA course system)
+-- bot/          # Discord bot (Poise framework, slash commands)
+-- config/       # Figment-based configuration
+-- data/         # Domain queries and models
|   +-- models.rs # Core domain types, DTOs, request/response shapes
|   +-- courses.rs
|   +-- terms.rs
|   +-- users.rs
|   +-- rmp.rs
|   +-- scrape_jobs.rs
|   +-- reference.rs
|   +-- ...
+-- db/           # Pool initialization, migrations, DbContext
+-- error.rs      # AppError, AppResult
+-- events/       # Event buffer and publishing
+-- rmp/          # RateMyProfessors GraphQL client
+-- scraper/      # Scheduler + Worker, job queue processing
+-- services/     # Service orchestration, startup/shutdown
+-- state.rs      # AppState (Arc-wrapped)
+-- utils/        # Shared utilities
+-- web/          # HTTP routes, extractors, auth, WebSocket
|   +-- routes.rs # Route definitions and handlers
|   +-- auth.rs   # Discord OAuth
|   +-- ws.rs     # WebSocket handlers
|   +-- error.rs  # ApiError, ApiErrorCode
|   +-- extractors.rs
|   +-- ...
+-- main.rs       # Server startup, router assembly
```

Each route group lives in `web/`. Data modules expose functions that take `&PgPool`. The `DbContext` wrapper adds event emission for operations that need it.

## Error Handling

- `AppError` enum with `thiserror` for domain errors (NotFound, BadRequest, Conflict, Unauthorized, etc.)
- `anyhow::Error` for internal/unexpected failures, wrapped via `AppError::Internal`
- `AppResult<T>` alias for `Result<T, AppError>`
- `From<sqlx::Error>` and `From<anyhow::Error>` conversions for `?` propagation
- `anyhow::Context` for adding context to data/service errors

```rust
// Data layer: use anyhow context
let course = sqlx::query_as!(Course, "...")
    .fetch_optional(pool).await
    .context("Failed to fetch course")?
    .ok_or(AppError::NotFound)?;

// Web handler: errors convert automatically
async fn get_course(
    State(state): State<AppState>,
    Path((term, crn)): Path<(String, String)>,
) -> AppResult<Json<CourseResponse>> {
    let course = data::courses::get_course(state.db(), &term, &crn).await?;
    Ok(Json(course))
}
```

The web layer's `ApiError` struct (with `ApiErrorCode` enum and message) handles serialization to the JSON error shape described in STYLE.md. `AppError` variants map to `ApiError` responses with appropriate HTTP status codes.

## State Management

`AppState` wraps shared resources with `Arc` for concurrent access. Accessor methods provide typed access to each subsystem.

```rust
// Access via Axum extractor
async fn handler(State(state): State<AppState>) -> AppResult<Json<T>> {
    let db = state.db();
    let events = state.events();
}
```

Caches use `Arc<RwLock<T>>` for read-heavy data (reference cache) and `Arc<DashMap<K, V>>` for concurrent write access (search options cache). Optional services return `Option<&T>` -- handlers check availability before use.

## Database

- **SQLx with compile-time verification** -- prefer `sqlx::query_as!` and `sqlx::query!` macros for all queries. These are checked against the schema at build time.
- **Runtime `query_as`** (non-macro) is acceptable for dynamically constructed queries but should be the exception, not the default.
- **Migrations** run automatically on startup via `sqlx::migrate!()`
- Prefer `query_as!` for SELECT (maps to structs), `query!` for mutations
- Use `Option<T>` for nullable columns
- **Batch operations**: Use `UNNEST` for bulk inserts/upserts instead of looping single inserts
- **JSONB**: Used for nested structures (meeting times, enrollment). Query with `jsonb_array_elements` and lateral joins.

```rust
// Batch upsert with UNNEST
sqlx::query!(
    r#"
    INSERT INTO reference_data (category, code, description)
    SELECT * FROM UNNEST($1::text[], $2::text[], $3::text[])
    ON CONFLICT (category, code)
    DO UPDATE SET description = EXCLUDED.description
    "#,
    &categories, &codes, &descriptions,
)
.execute(pool).await?;
```

## Serialization

- All public-facing types use `#[serde(rename_all = "camelCase")]`
- Types exported to frontend derive `TS` with `#[ts(export)]`
- `DateTime<Utc>` serializes as ISO 8601 strings (`#[ts(type = "string")]` for TypeScript)
- `i64` fields use a custom serializer to emit strings, avoiding JavaScript number precision loss
- Request types: derive `Deserialize`. Response types: derive `Serialize`. Shared types: both.

```rust
#[derive(Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CourseResponse {
    pub crn: String,
    pub term_code: String,
    pub subject: String,
    #[ts(type = "string")]
    pub last_updated: DateTime<Utc>,
    #[serde(serialize_with = "serialize_i64_as_string")]
    pub enrollment_max: i64,
}
```

## Async

- `tokio` runtime. All I/O is async.
- `tokio::spawn` for background tasks (scraper workers, scheduler, heartbeat).
- Background tasks log errors and continue -- no panics.
- No explicit locking for DB access -- SQLx pool handles concurrency.
- Use `tokio::select!` for tasks that need cancellation (shutdown signals).

## Discord Bot

Poise framework for Discord integration:

```rust
pub struct Data {
    pub app_state: AppState,
}
pub type Context<'a> = poise::Context<'a, Data, Error>;
```

- Commands are registered via a `get_commands()` function returning `Vec<poise::Command<Data, Error>>`
- Each command is `#[poise::command(slash_command, prefix_command)]`
- Always `ctx.defer().await?` before async work to avoid interaction timeouts
- Access application state via `ctx.data().app_state`
- Command errors use the application-level `Error` type (anyhow)

## Scraper

PostgreSQL-backed job queue with priority scheduling:

- **Scheduler**: Runs on a fixed interval (60s), analyzes data staleness, enqueues prioritized `ScrapeJob` rows
- **Worker**: Fetches and processes jobs atomically using `FOR UPDATE SKIP LOCKED`
- **Job trait**: Each job type implements `Job` with `process()` returning `UpsertCounts`
- **Lock expiry**: 10-minute safety net for dead workers
- **Priority ordering**: `priority DESC, execute_at ASC` -- high-priority jobs run first, ties broken by age
- **Refresh intervals**: Reference data (6h), RMP ratings (24h), terms (8h) -- configurable

Rate limiting for the Banner API uses Governor with per-endpoint costs and conditional bursting.

## Logging

- Import macros at module top: `use tracing::{debug, error, info, warn};`
- Use `#[instrument]` on handlers and significant functions. Skip large/sensitive args.
- Log errors in structured fields: `error!(error = %e, "Failed to process")`
- Spans propagate context -- child logs inherit parent span fields.

```rust
#[instrument(skip(state, body), fields(term = %term, crn = %crn))]
async fn update_course(
    State(state): State<AppState>,
    Path((term, crn)): Path<(String, String)>,
    Json(body): Json<UpdateRequest>,
) -> AppResult<Json<CourseResponse>> {
    // tracing context automatically includes term and crn
}
```

Per-module log levels are configured via `RUST_LOG` env var or the default filter. Noisy modules (rate limiter, session management) default to `warn`.

## Linting

- Zero clippy warnings allowed (`--deny warnings`)
- Run `just check` to validate (includes clippy)

## Optionality

- Use `Option<T>` for genuinely optional data (nullable DB columns, optional config)
- Prefer requiring values when the domain demands them -- don't default to `Option` for convenience
- Use newtypes for critical domain identifiers where type safety matters (e.g., term codes, CRNs)

## Testing

- **Runner**: `cargo nextest`
- **Integration tests** in `tests/` for handler-level testing
- **Unit tests** alongside code in `#[cfg(test)]` modules for data/service logic
- Name tests descriptively: `test_<action>_<condition>_<expected_result>`
- Use `assert2` crate when available
