# Mise Configuration Guide

## Overview

This project uses [mise](https://mise.jdx.dev/) to manage tools and tasks. Configuration is split across several files
to separate what is needed everywhere from what is only useful locally or in CI.

## Configuration Files

| File            | Purpose                                                                        |
| --------------- | ------------------------------------------------------------------------------ |
| `mise.toml`     | Tools and tasks needed in both CI and local dev                                |
| `mise.dev.toml` | Dev-only tools and hooks, loaded only when `MISE_ENV=dev`                      |
| `mise.ci.toml`  | CI-only settings, tools and env, loaded only when `MISE_ENV=ci`                |
| `.miserc.toml`  | Sets the default env to `dev` for local dev environments                       |
| `.mise/*.toml`  | Topical task definitions, included via `[task_config] includes` in `mise.toml` |

`mise.toml` sets `min_version` to the oldest mise release that supports every feature the configuration uses. Raise it
when you start using a newer feature, and never use a feature that still requires `experimental = true`.

## Envs

The project uses mise envs to load different configuration depending on the environment:

- **Local dev** (default): `.miserc.toml` sets `env = ["dev"]`, so mise loads `mise.toml` + `mise.dev.toml`.
  The enter hook runs automatically, installing tools and setting up the prek git hook.
- **CI**: `MISE_ENV=ci` is set in `.gitlab/ci/global.gitlab-ci.yml`, so mise loads `mise.toml` + `mise.ci.toml`.
  No enter hook, no dev-only tools. `mise.ci.toml` turns off auto-install so `mise run` doesn't install tools outside
  the job's tool set.

## Adding Tools

- Add tools used in **both CI and local dev** to `mise.toml`.
- Add tools used **only locally** (e.g. linters not run in CI) to `mise.dev.toml`.
- CI jobs only install the tools listed in their `.toolset:*` set in `.gitlab/ci/global.gitlab-ci.yml`. When a CI job
  needs a new tool, add it to that set too.
- Prefer tools shipped as a single binary (the current linters are prek, rumdl, ryl, tombi, tflint and actionlint):
  the CI image ships neither `python` nor `node`. If a tool needs a runtime, declare it with `depends` and pin the
  runtime too.
- Use the mise registry name when the tool has one (`mise registry | grep <tool>`); otherwise use an explicit backend
  such as `"github:<owner>/<repo>"`.
- All tools must use **pinned versions** (e.g. `tombi = "1.11.3"`). Never use `latest` or other floating references.
  Look up the current stable version and pin it explicitly.

## Adding Tasks

Task definitions live in topical files under `.mise/` (e.g. `.mise/prek.toml` groups the git hook tasks,
`.mise/tofu.toml` the OpenTofu ones). They are loaded via the `[task_config] includes` array in `mise.toml`.

When adding a new task:

1. Put it in the topical `.mise/*.toml` file that fits its domain.
2. Create a new file if no suitable one exists, and add it to the `includes` array in `mise.toml`.
3. Never add tasks directly to `mise.toml`.
