# Contributing

Thank you for your interest in contributing! This project is hosted on
[GitLab](https://gitlab.com/corthx/zed-ext-http): please open issues and merge requests there. The
[GitHub repository](https://github.com/corthx/zed-ext-http) is a read-only mirror.

## Reporting Bugs

Open an issue using
the [Bug Report](https://gitlab.com/corthx/zed-ext-http/-/work_items/new?description_template=Bug%20Report)
template. Include as much detail as possible: your Zed and extension versions, your OS, the steps to reproduce, and a
minimal file that shows the problem.

## Suggesting Features

Open an issue using
the [Feature Request](https://gitlab.com/corthx/zed-ext-http/-/work_items/new?description_template=Feature%20Request)
template. Describe the problem you're trying to solve and your proposed solution.

## Asking Questions

Open an issue using
the [Question](https://gitlab.com/corthx/zed-ext-http/-/work_items/new?description_template=Question) template.

## Submitting Code

1. Fork the project on GitLab.
2. Create a branch from `main`.
3. Make your changes.
4. Check that every lint passes: `mise run prek:run:all`.
5. Open a Merge Request against `main` using the provided template.

## Development Setup

See [docs/development.md](docs/development.md).

## Commit Messages

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```text
<type>(<scope>): <short description>
```

Common types: `feat`, `fix`, `chore`, `docs`, `refactor`, `test`.

Examples:

```text
feat(extension): Add syntax highlighting for request files
fix(extension): Keep header values on a single token
docs: Update README installation steps
chore(mise): Pin tombi version
```

Merge request titles follow the same format, with the issue number at the end (e.g.
`feat(extension): Add syntax highlighting for request files #12`): the title becomes the commit on `main`.

## License

By submitting a contribution, you agree that your code will be licensed under the [Apache License 2.0](LICENSE).
