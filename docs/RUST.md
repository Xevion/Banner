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

Two layers, two error types. There is no shared `AppError` enum.

**Data and service layers** return `anyhow::Result<T>`. Attach context at every fallible
boundary with `anyhow::Context`; the message becomes the operator-facing breadcrumb.

**Web layer** returns `Result<T, ApiError>` (`src/web/error.rs`). `ApiError` is a struct --
an `ApiErrorCode` enum, a human-readable `message`, and optional `details` JSON. It
implements `IntoResponse`, and `status_code()` maps each code to its HTTP status.

Handlers bridge the two explicitly. There is no blanket `From<anyhow::Error>` conversion:
crossing the boundary forces you to name the failure.

- `db_error(context, err)` logs the `anyhow::Error` and returns a generic internal error,
  so database details never leak to clients.
- `OptionNotFoundExt::or_not_found(entity, id)` turns `None` into a 404.
- `SqlxResultExt::conflict_on_unique(msg)` turns a PostgreSQL `23505` violation into a 409.

```rust
// Data layer: anyhow::Result plus context
pub async fn get_all_terms(db_pool: &PgPool) -> Result<Vec<DbTerm>> {
    let terms = sqlx::query_as::<_, DbTerm>("SELECT * FROM terms ORDER BY code DESC")
        .fetch_all(db_pool)
        .await
        .context("failed to fetch all terms")?;

    Ok(terms)
}

// Web handler: map anyhow into ApiError at the boundary
async fn get_instructor(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Json<InstructorResponse>, ApiError> {
    let instructor = data::instructors::get_instructor(state.db(), &slug)
        .await
        .map_err(|e| db_error("Get instructor", e))?
        .or_not_found("Instructor", &slug)?;

    Ok(Json(instructor))
}
```

`ApiError` serializes to the JSON error shape described in STYLE.md, and `ApiErrorCode` is
exported to TypeScript via ts-rs so the frontend can match on codes.

## State Management

`AppState` wraps shared resources with `Arc` for concurrent access. Accessor methods provide typed access to each subsystem.

```rust
// Access via Axum extractor
async fn handler(State(state): State<AppState>) -> Result<Json<T>, ApiError> {
    let db = state.db();
    let events = state.events();
}
```

Caches use `Arc<RwLock<T>>` for read-heavy data (reference cache) and `Arc<DashMap<K, V>>` for concurrent write access (search options cache). Optional services return `Option<&T>` -- handlers check availability before use.

## Database

- **Runtime queries are the default.** Use `sqlx::query_as::<_, T>(sql)` for SELECTs that
  map to a struct and `sqlx::query(sql)` for mutations, binding parameters with `.bind()`.
  Nearly every query in `src/data/` uses this form.
- **Do not add `query!`/`query_as!`/`query_scalar!` macros.** A few remain in `kv.rs` and
  `scoring.rs`; treat them as legacy, not as the pattern to follow.
- **Row structs** derive `sqlx::FromRow`. Column names must match field names (or be
  aliased in the SQL).
- **Migrations** run automatically on startup via `sqlx::migrate!()`
- Use `Option<T>` for nullable columns
- **Batch operations**: Use `UNNEST` for bulk inserts/upserts instead of looping single inserts
- **JSONB**: Used for nested structures (meeting times, enrollment). Query with `jsonb_array_elements` and lateral joins.

```rust
// Batch upsert with UNNEST
sqlx::query(
    r#"
    INSERT INTO reference_data (category, code, description)
    SELECT * FROM UNNEST($1::text[], $2::text[], $3::text[])
    ON CONFLICT (category, code)
    DO UPDATE SET description = EXCLUDED.description
    "#,
)
.bind(&categories)
.bind(&codes)
.bind(&descriptions)
.execute(pool)
.await
.context("failed to batch upsert reference data")?;
```

A handful of compile-time macro queries survive in `src/data/kv.rs` and
`src/data/scoring.rs`. They are the reason `.sqlx/` exists: tempo's preflight regenerates
that offline metadata when Rust sources or migrations change, so a `SQLX_OFFLINE=true`
build can still verify them without a live database. Do not add more -- every new query
uses the runtime form.

The trade-off is explicit: runtime queries are not checked against the schema at build
time, so a column rename surfaces as a runtime error. Cover new queries with tests.

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
) -> Result<Json<CourseResponse>, ApiError> {
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
