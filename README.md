# HTTP Zed Extension

[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![GitLab](https://img.shields.io/badge/source-GitLab-orange.svg)](https://gitlab.com/corthx/zed-ext-http)

Developed on [GitLab](https://gitlab.com/corthx/zed-ext-http): the GitHub repository is a read-only mirror, so please
open issues and merge requests on GitLab.

Run requests from `.http` / `.rest` files directly in [Zed](https://zed.dev):

- a **▶ run button in the gutter** next to every request;
- **environments** from `http-client.env.json` (+ `http-client.private.env.json` for secrets), with a per-project
  selection you can change at any time.

Requests are executed by [httpyac](https://httpyac.github.io), so its whole `.http` syntax works: variables,
`{{$uuid}}`-style dynamic values, request chaining, pre-request and response scripts, cookies, GraphQL, `.env` files…

Nothing to install besides the extension, and no Node.js: when you open an `.http` file, the extension downloads its
`http-runner` helper for your platform, which in turn downloads a standalone httpyac build (~90 MB, SHA-256-verified)
the first time. Both live in the extension's folder, so uninstalling the extension removes them.

Supported: macOS and Linux (arm64, x64). Windows is not supported yet.

## Install

The extension isn't published to Zed's extension registry yet. Until then, install it as a dev extension: command
palette → `zed: install dev extension` → select a clone of this repository (needs Rust with the `wasm32-wasip2` target).

The status bar shows the download the first time you open an `.http` file. An `http-runner` already on your `PATH`
(e.g. `cargo install --path runner`) is used instead of the download.

> If the **REST Client** (`restclient`) extension is installed, uninstall it: both register the `HTTP` language for
> `.http` files.

## Usage

Open an `.http` file and click **▶** next to a request. With the cursor in a request, `editor: spawn nearest task` does
the same from the keyboard.

| Task (on a request) | Effect                              |
| ------------------- | ----------------------------------- |
| **Send request**    | Sends with the current environment. |

| Task (`task: spawn` in an `.http` file) | Effect                          |
| --------------------------------------- | ------------------------------- |
| **HTTP: send all requests in file**     | Runs every request of the file. |

### Choosing the environment in Zed

**Code lens (recommended).** Turn on code lenses in your Zed settings (they're off by default, and Zed doesn't let
extensions turn them on):

```json
{ "code_lens": "on" }
```

Every request then shows your environments, with the one the ▶ button will use marked `●`:

```text
env: ● dev | ○ staging | ○ prod
GET {{baseUrl}}/users
```

Click an environment to switch to it. With more than five environments the lens shows a single `env: dev ▾` instead,
which opens a picker (Zed shows it as a notification with one button per environment).

**Code actions.** Without code lenses, put the cursor in an `.http` file and open the code actions menu (`cmd-.` /
`ctrl-.`, or click the lightbulb):

```text
✓ HTTP environment: dev (current)
HTTP: use environment "prod"
HTTP: clear environment selection
```

The choice is saved for the project and used by the next ▶ run.

### Environment resolution

1. `--env NAME` or the `HTTP_ENV` variable;
2. the saved selection for the project (set from the code lens, the code actions menu, or
   `http-runner env set FILE NAME`);
3. the only environment, if there is just one;
4. otherwise an interactive prompt in the task terminal (the choice is saved).

Env files are looked up from the `.http` file's folder upwards (each folder and its `env/` subfolder); the closest
wins. Selections are stored per env folder in `selected-envs.json`, outside your repository: in the extension's folder
for the copy of `http-runner` the extension installed, or in `<config dir>/http-runner/` for one you installed
yourself.

```json
{
  "$shared": { "userAgent": "http-runner" },
  "local":   { "baseUrl": "http://localhost:8080" },
  "prod":    { "baseUrl": "https://api.example.com" }
}
```

httpyac ignores `$shared` in `http-client.env.json`; `http-runner` passes those values with `--var` when the selected
environment doesn't define them. File variables (`@name = value`) still take precedence.

## CLI

```text
http-runner run FILE ROW [--env NAME] [--select-env] [-- HTTPYAC_ARGS...]
http-runner all FILE     [--env NAME] [--select-env] [-- HTTPYAC_ARGS...]
http-runner env select FILE
http-runner env set FILE NAME|none
http-runner env list FILE
http-runner setup        # pre-download httpyac
```

`ROW` is 1-based and may be any line of the request's block. Arguments after `--` go to `httpyac send` unchanged, e.g.
`-- -o exchange` or `-- --insecure`.

| Variable                   | Effect                                                                                          |
| -------------------------- | ----------------------------------------------------------------------------------------------- |
| `HTTP_ENV`                 | Environment to use, like `--env`.                                                               |
| `HTTP_RUNNER_HTTPYAC`      | Use this httpyac executable instead of the managed download (e.g. an `npm install -g httpyac`). |
| `HTTP_RUNNER_RELEASES_URL` | Download httpyac from a mirror: a URL template with `{tag}` and `{asset}`.                      |
| `HTTP_RUNNER_CONFIG_DIR`   | Where the environment selections are stored.                                                    |

## Contributing

Interested in contributing? See [CONTRIBUTING](CONTRIBUTING.md).

## License

Licensed under the [Apache License 2.0](LICENSE).  
Copyright 2026 Corentin Hervaud.
