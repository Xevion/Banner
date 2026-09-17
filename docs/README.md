# Documentation

## Style guides

- [`STYLE.md`](STYLE.md): cross-cutting conventions: naming, logging, errors, API design
- [`RUST.md`](RUST.md): backend layering, error types, SQLx, the scraper, the Discord bot
- [`SVELTE.md`](SVELTE.md): frontend runes, components, data fetching, bits-ui, TanStack Table

## Reference

- [`ARCHITECTURE.md`](ARCHITECTURE.md): how the services fit together and why
- [`BANNER.md`](BANNER.md): what we know about Ellucian's Banner system itself
- [`../CHANGELOG.md`](../CHANGELOG.md): notable changes by version

## Samples

[`samples/`](samples) holds captured Banner API responses, grouped by the call that produced them:
`search/` for course searches, `meta/` for reference lookups, and `course/` for section detail. They
are reference material for understanding response shapes, and fixtures for development.

Filenames mirror Banner's own endpoint names, which is why they mix conventions. `get_subject` and
`getTerms` are both spelled the way upstream spells them. That inconsistency is Ellucian's, and
normalizing it here would lose the mapping.
