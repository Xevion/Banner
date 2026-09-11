# banner

A complex multi-service system providing a Discord bot and browser-based interface to UTSA's course data.

## Services

The application consists of three modular services that can be run independently or together:

- Discord Bot ([`bot`][src-bot])

  - Primary interface for course monitoring and data queries
  - Built with [Serenity][serenity] and [Poise][poise] frameworks for robust command handling
  - Uses slash commands with comprehensive error handling and logging

- Web Server ([`web`][src-web])

  - [Axum][axum]-based server with Vite/React-based frontend
  - Serves the client build from disk via [`ServeDir`][tower-http], negotiating pre-compressed variants

- Scraper ([`scraper`][src-scraper])

  - Intelligent data collection system with priority-based queuing inside PostgreSQL via [`sqlx`][sqlx]
  - Rate-limited scraping with burst handling to respect UTSA's systems
  - Handles course data updates, availability changes, and metadata synchronization

## Quick Start

```bash
bun install --cwd web  # Install frontend dependencies
cargo build  # Build the backend

just dev # Runs auto-reloading dev build with all services
just dev-build # Development build with release characteristics (non-auto-reloading)

just build # Production build
```

## Documentation

Comprehensive documentation is available in the [`docs/`][documentation] folder.

[documentation]: docs/README.md
[src-bot]: src/bot
[src-web]: src/web
[src-scraper]: src/scraper
[serenity]: https://github.com/serenity-rs/serenity
[poise]: https://github.com/serenity-rs/poise
[axum]: https://github.com/tokio-rs/axum
[tower-http]: https://lib.rs/crates/tower-http
[sqlx]: https://github.com/launchbadge/sqlx
