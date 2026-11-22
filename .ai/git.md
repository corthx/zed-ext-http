# Git workflow rules

Before creating any branch or commit, or adding changelog entries, validate against all rules in this file.

## Branch naming rules

Allowed characters: lowercase letters, numbers, hyphens (`-`). No spaces. No uppercase.

Always add the issue number at the start of the branch name. This is required.

Use this pattern unless a different name is specified: `<issue-number>-<description>`

Do not use 40-character hexadecimal strings (these conflict with Git commit hashes).

## Branch base

Always create new branches from the latest `main` unless a different base is
explicitly specified or the current branch is already the correct base.

Before branching, run `git checkout main && git pull` to ensure the local
`main` is up to date.

## Commit message rules

### Conventional Commits (REQUIRED)

All commit subjects MUST follow conventional commit format:

**Format:** `<type>(<scope>): <description>`

**Types:**

- `feat:` - New features
- `fix:` - Bug fixes that affect the user
- `refactor:` - Code refactoring
- `docs:` - Documentation changes
- `style:` - Code style/formatting (no logic changes)
- `perf:` - Performance improvements
- `test:` - Testing changes
- `chore:` - Maintenance tasks, releases, dependency updates, CI/infrastructure changes, internal changes
- `security:` - Security-related changes

**Scopes:**

- For extension changes, use `extension`
- For internal config changes, use the name of the tool you changed configuration of (`mise`, `gitlab`, `terraform`,
  `renovate`, ...)

**Description Style:**

- Capitalize the first word of the description
- Use imperative mood ("add feature" not "added feature")
- Keep it concise but descriptive (72 characters maximum)
- No period at the end
- Minimum 3 words
- No emojis

**Examples:**

- `feat(extension): Add syntax highlighting for request files`
- `fix(extension): Keep header values on a single token`
- `docs(readme): Document the installation steps`
- `chore(mise): Pin tombi to 1.11.3`
- `chore(gitlab): Cache the tflint plugins`
- `chore(terraform): Manage the issue boards with OpenTofu`

### Body (conditional)

Required when the commit changes 10 or more files.

If a body is included:

- Separate from subject with one blank line
- 72 characters maximum per line
- Explain why the change is being made, not what it does

## Commit trailers

Do not add `Co-Authored-By:` trailers to commit messages.

## Commit granularity

Each commit should represent one coherent concern with a single review context and a clean rollback profile. A reviewer
should be able to evaluate a commit without needing to understand changes in sibling commits.

When a change spans multiple concern types, split proactively before the first commit — not after a reviewer flags it.
Common concern boundaries:

| Concern           | What it contains                                                                                                                                          |
| ----------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Documentation     | `*.md` files (README, guides, changelogs) — standalone doc-only changes, or docs that accompany a code change but are independently reviewable            |
| Implementation    | The functional change: the extension's code, grammars and configuration                                                                                   |
| Tests             | Tests for the implementation — travel with the implementation commit unless the suite is large enough to warrant a dedicated commit                       |
| Tooling / scripts | Shell scripts, CI pipeline config, build tooling, linting rules, mise tasks, OpenTofu units — anything that changes how the project is built, linted, run |

These are not mechanical rules — use judgment. The test: could this commit be reverted independently without breaking
the others? If not, it should probably be split further.

## Documentation requirements

When a commit adds a new user-facing feature or setting, the README must be updated to document it in the same commit.
Do not skip or defer this step — documentation is part of the feature, not an afterthought.

The README stays user-centric: it only documents how to install and use the extension. Documentation about this
repository and its processes goes in `CONTRIBUTING.md`, `docs/` or `.ai/`.

## Test requirements

When a commit adds a new user-facing feature, tests must be added or updated to exercise it in the same commit. The
tests must prove the new behavior — they must fail if the feature were silently inert or absent. Do not skip or defer
this step — tests are part of the feature, not an afterthought.

## Git hooks (prek)

This project uses [prek](https://prek.j178.dev/) to run its git hooks (formatters and linters, defined in `prek.toml`)
before each commit. The CI `lint` job runs the same hooks.

The hooks should already be installed by the mise enter hook, but you can make sure by running `mise run prek:setup`.

If committing fails, check the output for details — it is likely caused by one of the hooks. A formatter that rewrites
a file also stops the commit: review its change, stage it, and commit again.

### Linters not found (`Executable 'ryl' not found`, etc.)

The hooks rely on tools managed by mise (rumdl, ryl, tombi, tflint, …). When the shell that runs `git commit` does not
have the mise shims in its PATH, the hooks fail with "Executable not found".

Fix: run git commit through mise so all tools are available:

```bash
mise exec -- git commit -m "..."
```
