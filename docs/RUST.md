# Rust Style Guide (Backend)

General principles in [STYLE.md](STYLE.md).

## Architecture

### Layer Rules

Strict layering for data integrity:

```text
web/ (HTTP handlers)
  -> services/ (business logic)
    -> data/ (database access, domain queries)
      -> DB (PostgreSQL via SQLx)
```

- **Web handlers** handle HTTP concerns: extract params, call services/data, return responses.
- **Services** contain business logic that spans multiple data modules or has side effects (scraping, notifications, external API calls).
- **Data modules** are the only code that touches the database. All SQL lives here.
- Web handlers may call data modules directly for simple reads. A service layer is required when logic spans multiple data modules or has side effects beyond a single query.

### Module Organization

```text
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
+-- runtime/      # Process supervision: Service trait, ServiceManager, shutdown
+-- scraper/      # Scheduler + Worker, job queue processing
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

When a data-layer failure needs a specific status or carries structured detail, give the
module a `thiserror` enum and downcast it in the handler rather than matching on message
text. `AdminRmpError`, `MergeError` and `BluebookError` follow this shape.

```rust
// Data layer: a named failure, with the conflicting row as data
#[derive(Debug, thiserror::Error)]
pub enum AdminRmpError {
    #[error("instructor not found")]
    NoSuchInstructor,
    #[error("RMP profile already linked to {display_name}")]
    AlreadyLinked { instructor_id: i32, display_name: String },
}

// Web layer: downcast, then map each variant to its status
match err.downcast::<AdminRmpError>() {
    Ok(AdminRmpError::NoSuchInstructor) => ApiError::not_found("Instructor not found"),
    Ok(e @ AdminRmpError::AlreadyLinked { .. }) => ApiError::conflict(e.to_string()),
    Err(other) => db_error(context, other),
}
```

Downcasting survives an added `.context()`, so the data layer can still annotate the path.

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

Caches use `Arc<RwLock<T>>` for read-heavy data (reference cache) and `Arc<DashMap<K, V>>` for concurrent write access (search options cache). Optional services return `Option<&T>`: handlers check availability before use.

## Database

- **Compile-time macros are the default.** When the SQL is a string literal, use
  `sqlx::query!`, `sqlx::query_as!` or `sqlx::query_scalar!`. The macro checks the query
  against the live schema and reports each column's real nullability, which is what keeps
  domain types honest.
- **Runtime queries are the exception**, for queries whose _shape_ genuinely varies: a
  dynamic `ORDER BY` cannot be a bind parameter, and a WHERE clause assembled with
  `format!` is not a literal. Use `sqlx::query_as::<_, T>(sql)` / `sqlx::query(sql)` with
  `.bind()`, and leave a one-line comment at the call site saying why it cannot be a macro.
- **Row structs** derive `sqlx::FromRow`. Column names must match field names (or be
  aliased in the SQL). `query_as!` ignores `FromRow` entirely, so a struct with
  `#[sqlx(default)]` fields needs `query!` plus a hand-written map.
- **Migrations** run automatically on startup via `sqlx::migrate!()`
- **The macro's nullability is a strong hint, not proof.** Only declare `Option<T>` where
  the column is genuinely nullable; do not widen a type for convenience. Where sqlx is
  conservative and you know better, such as a view's `CASE` expression, `COUNT(*)` or a
  `COALESCE`d aggregate, assert it with `AS "col!"` rather than wrapping the field.
- **Verify the other direction by hand: sqlx over-reports non-null on outer joins.**
  Whether a `LEFT JOIN` column reads as nullable depends on the query plan, not the SQL
  text, so two selects differing only in their WHERE clause can disagree about the same
  column. A field declared `Option<T>` accepts a non-null report silently, because
  `From<T> for Option<T>` makes it compile, and then panics on the first real NULL. The
  build cannot catch this for you. Force it with `AS "col?"` on every column that reaches
  a row through a `LEFT JOIN`, a CTE or a `LATERAL`.
- **Pin the driving table's columns too, with `AS "col!"`.** The plan decides which side of
  an outer join is the nullable one, and on empty tables Postgres reads `a LEFT JOIN b` as
  a hash right join, which inverts the verdict: every `a` column comes back nullable. A
  non-`Option` field then fails to compile, on an empty database only. Once a query carries
  a `?` on one side it needs a `!` on the other, so its column types stop depending on how
  much data happens to be around.
- **`.sqlx` is prepared against `banner_sqlx`, an empty database holding only the
  migrations.** Never against the dev database. The metadata records the plan-derived
  nullability verbatim, so preparing against real data writes that machine's row counts
  into the repository and CI's `cargo sqlx prepare --check`, which migrates an empty
  database, rejects it byte for byte. `just check` handles this; a hand-run
  `cargo sqlx prepare` against the dev database does not.
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

Macro queries are the reason `.sqlx/` exists: tempo's preflight regenerates that offline
metadata when Rust sources or migrations change, so a `SQLX_OFFLINE=true` build verifies
them without a live database, and CI fails when the checked-in metadata is stale.

The trade-off is explicit: a runtime query is not checked against the schema at build
time, so a column rename surfaces as a failed request instead of a failed build. Cover
every runtime query with a test.

## Closed Value Sets

A column that holds one of a fixed set of strings gets a Rust enum, never a `String`.
The column is `TEXT`/`VARCHAR` rather than a Postgres enum type, so the enum carries:

- `strum`'s `AsRefStr`, `EnumString` and `IntoStaticStr` for the string mapping, with
  `#[strum(serialize_all = ...)]` matching serde's `#[serde(rename_all = ...)]` so the
  column value and the JSON value cannot drift.
- `text_column_enum!` (`src/data/models.rs`) for the SQLx `Type`/`Decode`/`Encode` codec.
  An unrecognised row fails the decode with `UnknownVariant`, naming the type and value.
- `VariantArray` where a test or a UI list needs every variant without a hand-kept copy.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS,
         AsRefStr, EnumString, IntoStaticStr, VariantArray)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
#[ts(export)]
pub enum BluebookLinkStatus { Auto, Pending, Approved, Rejected }

text_column_enum!(BluebookLinkStatus);
```

The enum then binds and decodes directly, a macro query names it with
`AS "status: BluebookLinkStatus"`, and every `match` on it is exhaustive.

## Pagination

List endpoints return `Page<T>` (`src/data/models.rs`): `items`, `total`, `page`,
`per_page`. Responses that carry extra aggregates nest it as a `page` field rather than
inventing a second envelope shape. There is no second shape; add none.

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
- Background tasks log errors and continue. No panics.
- No explicit locking for DB access, since SQLx pool handles concurrency.
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
- **Priority ordering**: `priority DESC, execute_at ASC` (high-priority jobs run first, ties broken by age)
- **Refresh intervals**: Reference data (6h), RMP ratings (24h), terms (8h), all configurable

Rate limiting for the Banner API uses Governor with per-endpoint costs and conditional bursting.

## Logging

- Import macros at module top: `use tracing::{debug, error, info, warn};`
- Use `#[instrument]` on handlers and significant functions. Skip large/sensitive args.
- Log errors in structured fields: `error!(error = %e, "Failed to process")`
- Spans propagate context: child logs inherit parent span fields.

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
- **The toolchain is pinned in `rust-toolchain.toml`, and CI reads its `channel`.** Rustup
  honours the file automatically, so `cargo` inside this repository is that version whatever
  `rustup default` says. Bumping it is a deliberate commit: a new release adds lints, and with
  `--deny warnings` an added lint is a build failure, not a warning. The `Dockerfile`'s
  `RUST_VERSION` tracks the same value.

## Optionality

- Use `Option<T>` for genuinely optional data (nullable DB columns, optional config)
- Prefer requiring values when the domain demands them, rather than defaulting to `Option` for convenience
- Use newtypes for critical domain identifiers where type safety matters (e.g., term codes, CRNs)

## Testing

- **Runner**: `cargo nextest`
- **Integration tests** in `tests/` for handler-level testing
- **Unit tests** alongside code in `#[cfg(test)]` modules for data/service logic
- Name tests descriptively: `test_<action>_<condition>_<expected_result>`
- Use `assert2` crate when available
