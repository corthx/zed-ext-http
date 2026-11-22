# Development Setup

## Requirements

- **[Mise](https://mise.jdx.dev/installing-mise.html#homebrew)**

Entering the project directory makes mise install the required tools and the prek git hook automatically.  
See the `[tools]` table in `mise.toml` for the full list.

To see the available mise tasks, run `mise run`.

## Linting

The git hooks in `prek.toml` format and lint the files you commit (OpenTofu, YAML, Markdown and TOML). The CI `lint`
job runs the same hooks.

| Task                   | Command                 |
| ---------------------- | ----------------------- |
| Lint the changed files | `mise run prek:run`     |
| Lint every file        | `mise run prek:run:all` |

When a formatter rewrites a file, the commit stops: review the change, stage it, and commit again.

## Infrastructure

The project's GitLab configuration is managed with OpenTofu under `terraform/`. See
[infrastructure.md](infrastructure.md).
