# Code Style Guide

Style guides for each subsystem: [Rust (backend)](RUST.md) | [Svelte (frontend)](SVELTE.md)

## Formatting

Automated. Rust uses `rustfmt`, frontend uses Biome. Don't think about it -- `just format` handles everything.

## Naming & Domain Vocabulary

Use language-idiomatic casing (snake_case in Rust, camelCase in TypeScript). Use consistent domain terms across all stacks:

| Concept | Name | Notes |
|---------|------|-------|
| An academic semester | `Term` | Identified by a term code (e.g. `"202430"`) |
| A course offering | `Course` | A specific section of a class in a term |
| A unique section identifier | `CRN` | Course Reference Number -- always a string |
| An academic department | `Subject` | E.g. `"CS"`, `"MAT"` -- the code, not the full name |
| A class meeting slot | `MeetingTime` | Days + time range + location + date range |
| A professor or TA | `Instructor` | Not "professor", "teacher", or "faculty" |
| A background scrape unit | `ScrapeJob` | A queued unit of work for the scraper |
| A user's course watch | `Subscription` | A monitored course that triggers notifications |
| A weekly course layout | `Schedule` | The visual timeline representation of a user's courses |
| A lookup table entry | `ReferenceData` | Campuses, instruction methods, session types, etc. |
| A RateMyProfessors record | `RmpRating` | Not "review" or "score" |

These names are used in types, API endpoints, database tables, and UI copy. When in doubt, check the Rust backend models -- they're the source of truth.

## Comments

Explain **why**, not **what**. Code should be self-documenting through clear names and small functions. Comments exist for:

- Non-obvious decisions or trade-offs
- Workarounds with context on what they're working around
- Domain knowledge that isn't obvious from the code (Banner API quirks, term code format, etc.)

Never reference old implementations, migrations, or refactoring history in comments. Never add banner comments (`===`, `---`).

## Logging

### Principles

Static messages, structured fields. All dynamic content goes in fields, never interpolated into the message string. This makes logs greppable and machine-parseable.

### Log Levels

| Level | Use for | Examples |
|-------|---------|----------|
| ERROR | Failures requiring attention | Database connection lost, scrape job failed permanently |
| WARN  | Recoverable issues | Retry succeeded, Banner API rate limit hit, fallback used |
| INFO  | Significant lifecycle events | Service started, scrape job completed, term sync finished |
| DEBUG | Routine operations | Cache hit, query executed, polling tick |
| TRACE | Verbose internals | Request/response bodies, full state dumps, SQL parameters |

Default to quiet. If an operation happens regularly without issue, it's DEBUG or TRACE.

### Standard Field Names

Use consistent field names across all stacks for values that may be aggregated or queried:

| Field | Type | Description |
|-------|------|-------------|
| `duration` | string | Operation timing (via `fmt_duration`) |
| `count` | number | Item counts |
| `bytes` | number | Data sizes |
| `term` | string | Term code (e.g. `"202430"`) |
| `crn` | string | Course reference number |
| `subject` | string | Subject code |
| `job_id` | number | Scrape job identifier |
| `instructor_id` | number | Instructor identifier |
| `error` | string | Error with chain |

### Duration Formatting

Use `crate::utils::fmt_duration` for all duration fields in tracing calls. It produces
human-readable output like `1.94ms`, `2.34s`, `150.00uss` with automatic unit scaling.

```rust
use crate::utils::fmt_duration;

// Good
debug!(duration = fmt_duration(elapsed), "query completed");
warn!(wait = fmt_duration(wait_duration), "rate limit delay");

// Bad
debug!(duration_ms = elapsed.as_millis(), "query completed");
```

Field naming: Use `duration` for general timing. Use contextual names (`elapsed`, `wait`,
`latency`, `remaining`) when clearer. Never suffix with `_ms` or `_us`.

Slow-operation thresholds use `Duration` constants:

```rust
const SLOW_THRESHOLD: Duration = Duration::from_secs(1);
if elapsed > SLOW_THRESHOLD {
    warn!(duration = fmt_duration(elapsed), "slow operation");
}
```

## Error Handling

### Philosophy

Errors are values, not exceptions. Each stack uses its own idiomatic pattern, but the principles are shared:

- **Expected failures** (not found, validation, invalid term) are handled explicitly with typed errors
- **Unexpected failures** (I/O, serialization, connection loss) are wrapped and propagated
- **Never swallow errors silently** -- log or propagate, never `catch {}` empty
- **User-facing error messages** are separate from internal error details

### API Error Responses

All API errors return a consistent JSON shape:

```json
{
  "code": "NOT_FOUND",
  "message": "Human-readable description",
  "details": null
}
```

`code` is `SCREAMING_SNAKE_CASE`. Known codes: `NOT_FOUND`, `BAD_REQUEST`, `INTERNAL_ERROR`, `INVALID_TERM`, `INVALID_RANGE`, `UNAUTHORIZED`, `FORBIDDEN`, `NO_TERMS`.

HTTP status codes map to error categories: 400 (validation/bad request), 401 (unauthorized), 403 (forbidden), 404 (not found), 500 (internal).

## API Design

- **Casing**: All JSON fields use `camelCase`
- **Search responses**: `{ courses: [...], totalCount: N, ... }` -- search-specific pagination with offset/limit
- **Single responses**: Return the object directly (no wrapper)
- **IDs in URLs**: Use the resource's natural identifiers: `/api/courses/:term/:crn`
- **Verbs via HTTP methods**: GET (read), POST (create), PUT (full update), PATCH (partial update), DELETE (remove)
- **Query params**: `camelCase`, arrays as repeated keys (`subject=CS&subject=MAT`)

## TypeScript Bindings

`ts-rs` generates TypeScript types from Rust structs into `web/src/lib/bindings/`. These are the canonical API types.

- Never duplicate or hand-write types that `ts-rs` generates
- `i64` fields serialize as strings to avoid JavaScript precision loss
- `DateTime<Utc>` serializes as ISO 8601 strings (`#[ts(type = "string")]`)
- Optional DateTime becomes `#[ts(type = "string | null")]`
- Barrel export from `$lib/bindings/index.ts` -- import from there, not individual files

## WebSocket Conventions

Real-time data uses a subscription-based WebSocket protocol:

- **Client messages**: `subscribe`, `modify`, `unsubscribe` -- each targets a named stream with a typed filter
- **Server messages**: `ready`, `subscribed`, `snapshot`, `delta`, `error`, `pong`, `modified`, `unsubscribed`
- **Snapshots** deliver full state on subscribe; **deltas** deliver incremental updates
- Stream names and filter shapes are typed end-to-end via shared types
- Reconnection uses exponential backoff (max 30s, max 10 attempts), auto-resubscribes on reconnect

## Discord Bot Commands

- **Naming**: Lowercase, single-word when possible (`search`, `terms`), hyphenated for multi-word (`ics-export`)
- **Descriptions**: Short imperative phrases (`"Search for courses"`, `"List available terms"`)
- **Long operations**: Always `defer()` before doing async work -- users see a loading state instead of a timeout
- **Responses**: Ephemeral for user-specific data, public for shareable results
- **Error messages**: User-friendly, no stack traces -- log the full error server-side

## Testing

### Principles

- Test behavior, not implementation. Tests should survive refactors.
- Prefer integration tests that exercise real code paths over unit tests with heavy mocking.
- Name tests descriptively: `test_searching_courses_with_invalid_term_returns_bad_request`.
- Each stack has its own test runner and conventions -- see the language-specific guides.
