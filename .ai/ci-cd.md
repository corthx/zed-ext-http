# CI/CD Configuration Guide

This document describes the architecture, patterns, and common tasks for this project GitLab CI/CD configuration.

## Overview

The CI/CD configuration spans different YAML files across `.gitlab-ci.yml` and `.gitlab/ci/`.

## Directory Structure

```text
.gitlab-ci.yml                    # Entry point: inputs, stages, workflow rules, includes
.gitlab/ci/
  global.gitlab-ci.yml            # Shared foundations: image, variables, retry, tool sets, before_script, caches
  rules.gitlab-ci.yml             # Centralized rules: conditions, change patterns, per-job composites
  mr.gitlab-ci.yml                # MR title validation job (mr-title-lint component)
  lint.gitlab-ci.yml              # prek linting job, renovate.json5 validation
  renovate.gitlab-ci.yml          # Scheduled Renovate run
  tofu.gitlab-ci.yml              # OpenTofu plan / apply / destroy, one set of jobs per terraform/live unit
```

## Include Mechanism

The main `.gitlab-ci.yml` uses a **wildcard include** to auto-register all top-level CI files:

```yaml
include:
  - local: .gitlab/ci/*.gitlab-ci.yml
```

Adding a new `.gitlab/ci/foo.gitlab-ci.yml` file automatically includes it in the pipeline. No manual registration
needed.

## Rules System

`rules.gitlab-ci.yml` is the single source of truth for all job conditions. It has three layers:

1. **Condition anchors** — bare mappings merged into a rule with `<<:`:
   - `&if-merge-request` — MR pipelines
   - `&if-merge-request-renovate` — MR pipelines from Renovate's `renovate/*` branches
   - `&if-merge-request-label-tofu-plan` / `&if-merge-request-label-tofu-apply` — MR pipelines with that label
   - `&if-default-branch` — pushes to the default branch
   - `&if-manual-run` — any other web pipeline on the default branch (`MANUAL_RUN=true`, set by the workflow)
   - `&if-destroy-infra` — `destroy_infra` input set to `true` on the default branch (`DESTROY_INFRA=true`)
   - `&if-schedule-renovate` — schedules with `SCHEDULE_TASK=renovate`
   - `&if-schedule-token-rotation` — schedules with `SCHEDULE_TASK=token-rotation`
2. **Change-pattern anchors** — bare lists aliased into a rule's `changes:`: `&gitlab-ci-patterns`, `&mise-patterns`,
   `&renovate-config-patterns`, `&lint-toolchain-patterns` (a change there re-lints the whole repo), and one
   `&tofu-<unit>-patterns` per `terraform/live/<unit>/`.
3. **Composite rule sets** — one `.<job>:rules` per job (`.lint:mr-title:rules`, `.lint:rules`,
   `.lint:renovate-config:rules`, `.renovate:rules`), plus `.tofu:rules:plan:<unit>` / `.tofu:rules:apply:<unit>` per
   OpenTofu unit and `.tofu:rules:destroy`, built from the anchors. These are the public API.

Jobs only ever `!reference [".<job>:rules", rules]`. Never reference the bare `.if-*` / `.*-patterns` anchors from
another file: anchors don't cross the wildcard include, and a referenced list can't be extended. Merging a condition
into a composite is what lets it add `changes:`, `when:` or `variables:` to a single rule.

### OpenTofu rules

A unit's plan runs when an MR or a default-branch push changes its files, on MRs labeled `tofu:plan` or `tofu:apply`,
on manual runs, in destroy pipelines, and (for the bot units) in the token-rotation schedule. Its apply is always
manual, except in the token-rotation schedule: on the default branch after a change, on manual runs, and on MRs
labeled `tofu:apply`. See [docs/infrastructure.md](../docs/infrastructure.md) for the units.

## Job Dependencies (DAG)

Stages exist purely for **visual grouping** in the GitLab pipeline UI — they have no effect on execution order. Every
job declares its real dependencies via `needs:`, and the actual execution timeline is the DAG formed by those
declarations.

Rules:

- Every job MUST declare a `needs:` key, even when empty. Never rely on implicit stage-by-stage ordering.
- `needs: []` — for jobs with no upstream dependency. They start at t=0 alongside every other `needs: []` job,
  regardless of which stage they sit in.
- `needs: [{job: <name>, artifacts: false}]` — for jobs that depend on another job's completion. Pass
  `artifacts: false` when you only care about completion and don't need the upstream job's artifact bundle.
- `needs: [{job: <name>, optional: true}]` — when the upstream job isn't in every pipeline (e.g. `apply:labels` before
  `apply:issue-boards`). A `needs` on a job absent from the pipeline is a config error otherwise.

## Shared Foundations

### `global.gitlab-ci.yml`

Defines reusable building blocks:

- **`default: image`** — `jdxcode/mise:<version>-debian`, with `entrypoint: [""]`. The bare tag has no shell and no
  git.
- **`.default-retry`** — retry policy (max 2 retries for infra failures)
- **`.toolset:*`** — per-job tool sets (`.toolset:lint`). Each sets `TOOLSET` (cache prefix) and `TOOLSET_TOOLS` (the
  mise tools to install). A tool added to `mise.toml` is only available to the jobs whose set lists it.
- **`.mise-before_script`** — `mise trust` + `mise install $TOOLSET_TOOLS` + env activation. Pair it with a
  `.toolset:*` set.
- **`.interruptible`** / **`.not-interruptible`** — cancel-on-new-commit flags
- **`.no-checkout`** — `GIT_STRATEGY: none`, for jobs that don't read the repository (`renovate`)
- **`.renovate-image`** — the Renovate image, shared by `renovate` and `lint:renovate-config` so one bump covers both
- **Cache definitions** — `.mise-cache` (one archive per tool set via `prefix: $TOOLSET`), `.tflint-cache` (rulesets
  fetched by `tflint --init` from GitHub's rate-limited API), `.renovate-cache`

The OpenTofu jobs don't use mise: the OpenTofu component supplies its own image.

## Stages

| Stage         | Jobs                   | Condition                                                 |
| ------------- | ---------------------- | --------------------------------------------------------- |
| `maintenance` | `renovate`             | `SCHEDULE_TASK=renovate` schedule only                    |
| `lint`        | `mr-title-lint`        | MR pipelines, except Renovate's                           |
| `lint`        | `lint`                 | MR, default branch, and manual run                        |
| `lint`        | `lint:renovate-config` | `renovate.json5` or its job/image changes, and manual run |
| `plan`        | `plan:<unit>`          | Per unit, see `.tofu:rules:plan:<unit>`                   |
| `apply`       | `apply:<unit>`         | Per unit, see `.tofu:rules:apply:<unit>`                  |
| `cleanup`     | `destroy:<unit>`       | Destroy pipeline only                                     |

## Common Tasks

### Add a new CI job

1. Add the job to the appropriate `.gitlab/ci/*.gitlab-ci.yml` file (or create a new one — it auto-includes via
   wildcard).
2. Add a `.<job>:rules` composite to `rules.gitlab-ci.yml` and reference it with
   `!reference [".<job>:rules", rules]`.
3. Extend shared foundations (`.default-retry`, `.interruptible`). If the job uses mise, also extend a `.toolset:*`
   set (add one if none fits) and set `before_script: [!reference [".mise-before_script", before_script]]`.
4. Declare `needs:` explicitly. Use `needs: []` if there is no upstream dependency.

### Add an OpenTofu unit

1. Add `terraform/live/<unit>/` (`main.tf`, `outputs.tf`, `variables.tf`) and its lock file
   (`mise run tofu:lock:all`).
2. In `rules.gitlab-ci.yml`, add a `&tofu-<unit>-patterns` anchor and the `.tofu:rules:plan:<unit>` /
   `.tofu:rules:apply:<unit>` composites, copied from an existing unit.
3. In `tofu.gitlab-ci.yml`, add a `.unit:<unit>` (auth + `GITLAB_TOFU_ROOT_DIR` + `GITLAB_TOFU_STATE_NAME`) and its
   `plan:` / `apply:` / `destroy:` jobs. The apply waits for `apply:terraform-provider-bot` (optional), so a token
   rotation can't invalidate its token mid-run.
4. List the unit in [docs/infrastructure.md](../docs/infrastructure.md).

## Workflow Rules

The `workflow:rules:` block in `.gitlab-ci.yml` determines whether a pipeline runs at all:

- **Scheduled pipelines** — run only when the schedule sets `SCHEDULE_TASK`, named `Scheduled pipeline: <task>`
- **MR pipelines** — triggered by merge request events
- **Default branch pipelines** — pushes/merges to main
- **Destroy pipelines** — manual web trigger with the `destroy_infra` input set to `true`, named `Destroy pipeline`.
  Sets `DESTROY_INFRA=true`, which exposes the `destroy:<unit>` jobs.
- **Manual runs** — any other web trigger on the default branch, named `Manual run`. Sets `MANUAL_RUN=true`. This
  rule must stay last so the more specific web rules above it win.

Pushes to other branches don't create a pipeline until a merge request exists. `workflow:name: "$PIPELINE_NAME"`
applies the name each rule sets.

## Gotchas

- **Wildcard auto-include**: Any new `*.gitlab-ci.yml` at the top level of `.gitlab/ci/` is automatically included.
  Files in subdirectories are NOT auto-included.
- **Rules first-match-wins**: GitLab CI `rules:` uses first-match-wins semantics. Order matters.
- **`!reference` vs YAML anchors**: Use `!reference [".some-key", rules]` to reference keys across files included at
  the same level. YAML anchors (`*anchor`) only work within the same file.
- **Jobs are fixed when the pipeline is created**: adding or removing an MR label (e.g. `tofu:apply`) only affects the
  next pipeline. Retrigger one to pick up the change.
- **Blocking manual jobs**: the `apply:<unit>` jobs are manual with `allow_failure: false`, so a pipeline that exposes
  them stays "blocked" until they run.
