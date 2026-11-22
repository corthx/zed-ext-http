# Architecture

```text
.http file ──▶ gutter ▶ (runnables.scm) ──▶ task (tasks.json) ──▶ http-runner ──▶ httpyac
                                                                     ▲              ▲
Zed opens .http ──▶ extension (src/lib.rs) ── downloads ─────────────┘              │
                    starts `http-runner lsp` ── pre-downloads (checksum-verified) ──┘
```

## Why the extension pretends to be a language server

Zed tasks are plain shell commands, and Zed runs no extension code when a task starts: the
extension API (`zed_extension_api` 0.7) has hooks for language servers, debug adapters, MCP
servers and slash commands only, and no task variable points at the extension's directory.

So the extension declares an `HTTP Runner` language server. When an `.http` file opens, Zed
calls `language_server_command`, which is allowed to download files into the extension's
working directory. It fetches `http-runner` from the `http-runner-v<extension version>`
release into `bin/http-runner` and starts `http-runner lsp`, a small server
that pre-downloads httpyac in the background and lets the user pick the environment of the
nearest `http-client.env.json`:

- above each request, one code lens per environment (Zed renders lenses of a line side by side);
  clicking one runs `http-runner.selectEnvironment` and sends `workspace/codeLens/refresh`. Past five
  environments a single lens runs `http-runner.pickEnvironment`, which answers with a
  `window/showMessageRequest` (Zed shows it as a notification with one button per environment).
  An extension can't open a popup next to the lens: Zed only maps lens commands to its own UI for
  built-in languages (`LspAdapter::client_command`);
- code actions run `http-runner.selectEnvironment` / `http-runner.clearEnvironment`.

Selections go to the same store the CLI reads, so the next ▶ run uses them. Request lines for the
lenses come from a line scan of the synced document (`METHOD url`, or a bare URL right after `###`),
not from tree-sitter, which the server doesn't have.

## How tasks find the runner

Each task runs a short `sh -c` snippet that uses, in order:

1. `http-runner` from the `PATH` (a `cargo install`ed build wins, which is handy during development);
2. `~/Library/Application Support/Zed/extensions/work/http-runner/bin/http-runner` (macOS);
3. `${XDG_DATA_HOME:-~/.local/share}/zed/extensions/work/http-runner/bin/http-runner` (Linux).

Paths 2 and 3 are where Zed keeps extension working directories. They are not a documented
contract; if Zed moves them, update the snippet in `languages/http/tasks.json` and `BINARY` in
`src/lib.rs`.

Zed substitutes only `$ZED_*` variables, then joins `command` and `args` with spaces **without
quoting** and runs the result with `$SHELL -i -c` (`ShellBuilder::build_no_quote` in Zed's
`terminal_panel.rs`). The task arguments are therefore shell source: the snippet is wrapped in
single quotes (it must never contain one) and `"$ZED_FILE"` in double quotes, which works in
sh, bash, zsh and fish.

## Downloads and integrity

| Artifact                  | Downloaded by                           | Verified                                                                  |
| ------------------------- | --------------------------------------- | ------------------------------------------------------------------------- |
| `http-runner-<os>-<arch>` | the extension, via `zed::download_file` | no (the extension API has no hashing; same trust as the extension itself) |
| `httpyac-<os>-<arch>`     | `http-runner`                           | SHA-256 embedded in `runner/httpyac.sha256`                               |

`http-runner` writes only inside the extension's working directory when it is the copy the
extension installed (detected by the `bin/version` marker next to it): httpyac goes to
`httpyac-<version>/` and selections to `selected-envs.json` there, so uninstalling the extension
removes everything. A copy installed with `cargo install` uses the user's cache and config
directories (`<cache dir>/http-runner/`, `<config dir>/http-runner/`) instead.

Windows is not supported yet: the tasks rely on `sh`.
