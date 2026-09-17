# Architecture

Banner is a single Rust binary that supervises several long-running services, plus a SvelteKit
frontend it serves and manages as a child process. This describes the shape of the system and the
decisions behind it, not its current surface area; endpoints, schema, and feature lists live in the
code, which is the only copy that can't fall out of date.

## Services

`ServiceManager` (`src/runtime/manager.rs`) owns every long-running task. A service implements
`Service`, runs until cancelled, and reports back through a completion channel; the manager
broadcasts one shutdown signal and waits on all of them. Six register today: `web`, `ssr`,
`scraper`, `metrics`, `notifications`, and `bot`. Which ones start is configuration, so the same
binary can run the full stack or a single concern.

A service returning normally is treated as a fault, not success. The contract is to block forever,
so an unexpected return means something stopped that shouldn't have, and the manager logs it as
such.

## The SSR child process

The frontend renders through a Node process that Rust starts and supervises (`src/runtime/ssr.rs`),
rather than a separate container. Rust is PID 1, holds the public listener, and proxies non-API
routes downstream to it. `SSR_COMMAND` is what decides this: when set, the process is managed here;
when unset, Vite is already serving SSR and Rust stays out of the way. That single variable is the
whole difference between development and production topology.

## Layering

```text
web/     HTTP handlers, extraction, serialization
  -> data/   every query, and the only code that touches the database
    -> PostgreSQL
```

`src/data/` holds all SQL. Handlers may call it directly for straightforward reads; anything
spanning several data modules or carrying side effects (scraping, notifications, outbound API
calls) belongs above it. The rule exists so that a schema change has one blast radius.

Queries are verified at compile time by SQLx against the cached metadata in `.sqlx/`, which is why
a build needs no database and why that directory is regenerated rather than edited.

## Types across the boundary

Rust structs annotated with `ts-rs` generate the TypeScript definitions under
`web/src/lib/bindings/`. The frontend never hand-writes an API type, and a backend field rename
surfaces as a frontend type error rather than an undefined at runtime. Generation is checked in
preflight, so stale bindings fail before they reach a build.

The corollary is that structured columns must be structured all the way down: `JSONB` in Postgres
and `Json<T>` in Rust, never a JSON-encoded `TEXT`. A serialized string types as `string` in the
bindings, and the type system then cannot tell you that rendering it produces `["a","b"]` on the
page.

## Scraping

The scraper is a queue in Postgres rather than an in-process schedule, so work survives restarts
and is inspectable with a query. Jobs are per-subject, rate limited through `governor`, and
scheduled adaptively: how often a subject changes and how many courses it carries both feed the
interval. The intent is to hold total load on the upstream system roughly flat while keeping the
data that moves fresher than the data that doesn't.

## Live updates

Clients subscribe over a single WebSocket (`src/web/stream/`) to named streams rather than polling.
Server-side state changes fan out through a broadcast buffer, and each subscription filters the
feed it cares about.

## Instructor scoring

Instructor ratings are composed from independent sources (RateMyProfessors, BlueBook) into one
score. Matching upstream records to catalog instructors is fuzzy and therefore wrong sometimes, so
matches are proposed with a confidence score and confirmed or rejected through admin review rather
than trusted outright.
