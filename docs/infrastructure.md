# Infrastructure

This project's GitLab configuration (labels, issue boards, pipeline schedules and the bots' service accounts) is
managed as code with [OpenTofu](https://opentofu.org/). Change it by editing HCL and merging a merge request, not in
the GitLab UI: a manual change is reverted by the next apply.

## Layout

`terraform/live/<unit>/` holds one small root module per concern. Each one calls a versioned module that the
`infrastructure` repository publishes to the GitLab module registry (`gitlab.com/corthx/<module>/gitlab`).

| Unit                               | Manages                                                                                                               |
| ---------------------------------- | --------------------------------------------------------------------------------------------------------------------- |
| `labels`                           | Project labels, including `tofu:plan` / `tofu:apply`                                                                  |
| `issue-boards`                     | Project issue boards and their lists                                                                                  |
| `terraform-provider-bot`           | Maintainer service account whose token (`GITLAB_TOFU_PROVIDER_TOKEN`) authenticates the GitLab provider in every unit |
| `terraform-provider-rotator-bot`   | Second maintainer account (`GITLAB_TOFU_PROVIDER_ROTATOR_TOKEN`), used only to rotate the provider bot's own token    |
| `tofu-bot`                         | Reporter account (`GITLAB_TOFU_MR_PLAN_TOKEN`) that posts plans as MR comments                                        |
| `pipeline-schedule-token-rotation` | Weekly `SCHEDULE_TASK=token-rotation` schedule that re-applies the bot units                                          |
| `renovate-bot`                     | Developer account (`RENOVATE_TOKEN`, `RENOVATE_GIT_AUTHOR_EMAIL`) that Renovate runs as                               |
| `pipeline-schedule-renovate`       | Daily `SCHEDULE_TASK=renovate` schedule that runs Renovate                                                            |

GitLab manages the state: each unit has its own encrypted state, named after the unit. `backend "http" {}` stays empty
in code, and the OpenTofu CI component fills it in. No state file or `.terraform` directory is ever committed; the
`.terraform.lock.hcl` files are.

Out of scope: the `gitlab_project` resource (visibility, merge request settings, protected branches and tags). Adopting
it would mean importing the live project, and an apply could end up reconfiguring it.

## Day-to-day

- **Merge requests**: a unit's `plan` job runs when the merge request changes that unit's files. Add the `tofu:plan`
  label to plan every unit, or `tofu:apply` to plan every unit and expose a manual `apply` on the merge request.
- **Default branch**: after a merge, the changed units plan again and their `apply` job waits for a manual run.
- **Manual runs**: a pipeline run from the UI on the default branch plans every unit and exposes every `apply`.
- **Token rotation**: the weekly schedule re-applies the bot units, which rotates any token within 14 days of
  expiring.

Locally, `mise run tofu:format`, `mise run tofu:lint` and `mise run tofu:lock:all` format, lint and validate the units,
and refresh their provider lock files. When `.tf` files are staged, the prek hooks format them, autofix their lint
errors, then validate the units.

## Dependency updates (Renovate)

GitLab has no hosted Renovate app, so Renovate runs from this project's pipeline: the `renovate` job
(`.gitlab/ci/renovate.gitlab-ci.yml`) runs on the daily `SCHEDULE_TASK=renovate` schedule as the renovate bot, and
opens update merge requests from `renovate/*` branches, labeled `dependencies`. The update policy lives in
`renovate.json5`, validated by the `lint:renovate-config` job when it changes.

Renovate keeps the provider, the `corthx/*` modules, OpenTofu and the OpenTofu CI component up to date. OpenTofu and
the component are written twice (`mise.toml` and the component's `opentofu_version` input, the include ref and its
`version` input): each copy carries a `# renovate:` comment, and one merge request labeled `tofu:plan` bumps both.

## Destroying the infrastructure

Run a pipeline on the default branch with the `destroy_infra` input set to `true`. This exposes a manual `destroy`
job per unit. Destroying `labels` and `issue-boards` deletes the project's labels and boards.
