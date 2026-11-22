# Development Setup

## Requirements

- **[Mise](https://mise.jdx.dev/installing-mise.html#homebrew)**

Entering the project directory makes mise install the required tools and the prek git hook automatically.  
See the `[tools]` table in `mise.toml` for the full list.

To see the available mise tasks, run `mise run`.

## Linting

The git hooks in `prek.toml` format and lint the files you commit (Rust, OpenTofu, YAML, Markdown, TOML and JSON). The
CI `lint` job runs the same hooks.

Run them on the changed files with `mise run prek:run`, or on every file with `mise run prek:run:all`.

When a formatter rewrites a file, the commit stops: review the change, stage it, and commit again.

## Building and Testing

The repository holds two Rust crates: the extension at the root, built for `wasm32-wasip2` like Zed does, and
`http-runner` in `runner/`. mise installs the Rust toolchain (with that target) and Bun, which builds httpyac.

The `rust:*` tasks format, lint, test and build the crates, and install `http-runner` on the `PATH`. The `httpyac:*`
tasks check that the httpyac patches apply and build httpyac for this platform (into `httpyac-dist/dist/`). In CI, the
tests and the build run when their files change.

## Trying the Extension in Zed

Without a published release, run everything from local builds:

1. Build httpyac and install `http-runner` on the `PATH`:

   ```sh
   mise run httpyac:build
   mise run rust:install:runner
   ```

2. Point `http-runner` at that httpyac build: add
   `export HTTP_RUNNER_HTTPYAC=<repository>/httpyac-dist/dist/httpyac` to your shell profile. Zed runs tasks through
   your interactive shell (`$SHELL -i -c`), so they see it.

3. In Zed, run `zed: install dev extension` and select the repository root. The extension and the tasks use the
   `http-runner` on the `PATH` instead of downloading one.

4. Open `examples/demo.http` to try the requests and environments.

See [architecture.md](architecture.md) for how the extension, `http-runner` and httpyac fit together.

## Infrastructure

The project's GitLab configuration is managed with OpenTofu under `terraform/`. See
[infrastructure.md](infrastructure.md).
