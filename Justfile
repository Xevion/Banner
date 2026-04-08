set dotenv-load

# Aliases
alias c := check
alias d := dev
alias t := test
alias f := format
alias fmt := format
alias l := lint
alias s := search
alias bld := build
alias bind := bindings
alias b := bun

default:
    just --list

# Run all checks in parallel. Targets: backend,frontend. Pass --fix to auto-format first.
check *args:
    tempo check {{args}}

# Auto-format code. Targets: backend,frontend
format *targets:
    tempo fmt {{targets}}

# Lint code. Targets: backend,frontend
lint *targets:
    tempo lint {{targets}}

# Run tests. Usage: just test [rust|web|<nextest filter args>]
test *args:
    tempo test {{args}}

# Generate TypeScript bindings from Rust types (ts-rs)
bindings:
    tempo bindings

# Run the Banner API search demo (hits live UTSA API, ~20s)
search *ARGS:
    tempo search {{ARGS}}

# Dev server. Flags: -f(rontend) -b(ackend) -W(no-watch) -n(o-build) -r(elease) -e(mbed) -I(no-interrupt) -V(erbose-build) --tracing <fmt>
# Pass args to binary after --: just dev -n -- --some-flag
[no-exit-message]
dev *flags:
    exec tempo dev {{flags}}

# Production build. Flags: -d(ebug) -f(rontend-only) -b(ackend-only)
build *flags:
    tempo build {{flags}}

# Start PostgreSQL in Docker and update .env with connection string
# Commands: start (default), reset, rm
db cmd="start":
    tempo db {{cmd}}

bun *ARGS:
	cd web && bun {{ ARGS }}

# Run Storybook development server on port 6006
storybook: (bun "storybook")
# Build Storybook static site
storybook-build: (bun "storybook:build")
# Run Storybook tests
storybook-test: (bun "storybook:test")

sql *ARGS:
	lazysql ${DATABASE_URL}

# Install git pre-commit hooks
install-hooks:
    #!/usr/bin/env bash
    set -euo pipefail
    mkdir -p .husky
    echo "tempo pre-commit" > .husky/pre-commit
    chmod +x .husky/pre-commit
    echo "(ok) Pre-commit hook installed"
