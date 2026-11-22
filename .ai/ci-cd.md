# CI/CD Configuration Guide

How this project's GitLab CI/CD configuration is organized, and the conventions to follow when changing it.

## Layout

- `.gitlab-ci.yml` — the entry point: inputs, stages, workflow rules, and a wildcard include of
  `.gitlab/ci/*.gitlab-ci.yml`. A new file there is included automatically; files in subdirectories are not.
- `.gitlab/ci/global.gitlab-ci.yml` — the shared building blocks jobs extend: image, variables, tool sets, caches.
- `.gitlab/ci/rules.gitlab-ci.yml` — the rules of every job.
- The other files hold the jobs, grouped by concern (linting, tests, builds, OpenTofu, Renovate).

## Pipelines

The workflow rules in `.gitlab-ci.yml` decide whether a pipeline runs at all:

- **MR pipelines** and **default-branch pushes**. Pushes to other branches get no pipeline until an MR exists.
- **Schedules**, only when the schedule sets `SCHEDULE_TASK`. Jobs pick their schedule by its value.
- **Web runs on the default branch**: a destroy pipeline when the `destroy_infra` input is `true` (sets
  `DESTROY_INFRA=true`), a manual run otherwise (sets `MANUAL_RUN=true`). The manual-run rule must stay last so the more
  specific web rules above it win.

Included files can't read inputs, so the workflow turns them into variables for the job rules. Each rule names its
pipeline through `PIPELINE_NAME`.

## Rules

`rules.gitlab-ci.yml` is the single source of truth for job conditions. It has three layers:

1. **Condition anchors** (`&if-*`) — bare mappings merged into a rule with `<<:`, one per pipeline kind, MR label or
   schedule task.
2. **Change-pattern anchors** (`&*-patterns`) — bare lists aliased into a rule's `changes:`.
3. **Composite rule sets** (`.<job>:rules`) — one per job, built from the anchors. These are the public API.

Jobs only ever `!reference [".<job>:rules", rules]`. Never reference the bare anchors from another file: anchors don't
cross the wildcard include, and a referenced list can't be extended. Merging a condition into a composite is what lets
it add `changes:`, `when:` or `variables:` to a single rule.

A job's patterns must cover everything that can change its outcome, not only its sources: the files pinning the
versions of the tools it runs (`mise.toml`, `mise.ci.toml`), the mise tasks and scripts it calls, and the CI files
defining it, its image and its components. Renovate bumps those files on their own, and a bump nothing tests merges
untested.

Most jobs run on MRs and default-branch pushes that change their patterns, and on every manual run, which checks
everything. OpenTofu plans run the same way; applies stay manual, except in the token-rotation schedule. See
[docs/infrastructure.md](../docs/infrastructure.md).

## Job Dependencies (DAG)

Stages only group jobs in the pipeline UI. The execution order is the DAG formed by `needs:`:

- Every job MUST declare `needs:`, even when empty. Never rely on stage-by-stage ordering.
- `needs: []` — no upstream dependency: the job starts right away, whatever its stage.
- `needs: [{job: <name>, artifacts: false}]` — wait for another job without downloading its artifacts.
- `needs: [{job: <name>, optional: true}]` — the upstream job isn't in every pipeline. A `needs` on a job absent from
  the pipeline is a config error otherwise.

## Shared Foundations

- **Image** — `jdxcode/mise:<version>-debian` with `entrypoint: [""]`. The bare tag has no shell, and its `mise`
  entrypoint would reject the runner's `-c` flag.
- **Tool sets** — each `.toolset:<name>` sets `TOOLSET` (the cache prefix) and `TOOLSET_TOOLS` (the mise tools to
  install). `.mise-before_script` installs them and activates the mise env. A job only gets the tools its set lists, so
  a tool added to `mise.toml` must also be added to the sets that need it. `mise.ci.toml` (`MISE_ENV=ci`) holds the
  CI-only tools and env.
- **Zig as the C toolchain** — the image has no compiler, which every job compiling Rust needs. Their tool sets include
  `zig`, and `mise.ci.toml` points `CC`, `AR` and the host linker at the `ci/zig-cc` / `ci/zig-ar` wrappers.
- **Caches** — one hidden `.<name>-cache` each, keyed on the files that determine its content so it's only rebuilt when
  they change. The mise cache keeps one archive per tool set: a shared key would let the last job overwrite it with
  its own subset. A tool keeping its own cache outside `MISE_DATA_DIR` gets a separate cache, with its directory moved
  under `$CI_PROJECT_DIR` by a global variable (GitLab only caches paths inside the project).
- **Job flags** — `.default-retry`, `.interruptible` / `.not-interruptible`, `.no-checkout` (for jobs that don't read
  the repository).
- **Shared images** — an image used by several jobs gets its own hidden key (e.g. `.renovate-image`), so one bump
  covers them all.

The OpenTofu jobs don't use mise: the OpenTofu component supplies its own image.

## Common Tasks

### Add a new CI job

1. Add the job to the matching `.gitlab/ci/*.gitlab-ci.yml` file, or create a new one.
2. In `rules.gitlab-ci.yml`, add its `&<job>-patterns` anchor if it needs one, and its `.<job>:rules` composite.
   Reference it with `!reference [".<job>:rules", rules]`.
3. Extend the shared foundations (`.default-retry`, `.interruptible`). If the job uses mise, also extend a `.toolset:*`
   set (add one if none fits) and set `before_script: [!reference [".mise-before_script", before_script]]`.
4. Declare `needs:` explicitly.

### Add an OpenTofu unit

1. Add `terraform/live/<unit>/` (`main.tf`, `outputs.tf`, `variables.tf`) and its lock file
   (`mise run tofu:lock:all`).
2. In `rules.gitlab-ci.yml`, add a `&tofu-<unit>-patterns` anchor and the `.tofu:rules:plan:<unit>` /
   `.tofu:rules:apply:<unit>` composites, copied from an existing unit.
3. In `tofu.gitlab-ci.yml`, add a `.unit:<unit>` (auth + `GITLAB_TOFU_ROOT_DIR` + `GITLAB_TOFU_STATE_NAME`) and its
   `plan:` / `apply:` / `destroy:` jobs. The apply waits for `apply:terraform-provider-bot` (optional), so a token
   rotation can't invalidate its token mid-run.

## Gotchas

- **Rules first-match-wins**: GitLab CI `rules:` stops at the first matching rule. Order matters.
- **`!reference` vs YAML anchors**: `!reference` works across included files; YAML anchors only within one file.
- **Jobs are fixed when the pipeline is created**: adding or removing an MR label (e.g. `tofu:apply`) only affects the
  next pipeline. Retrigger one to pick up the change.
- **Blocking manual jobs**: a manual job with `allow_failure: false` (the OpenTofu applies) leaves its pipeline
  "blocked" until it runs.
