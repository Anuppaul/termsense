# Contributing to TermSense

Thanks for considering a contribution to TermSense.

TermSense is a local-first Linux terminal assistant written in Rust with a Bash/Readline adapter. Contributions are welcome for bug fixes, shell UX improvements, command intelligence, parser coverage, documentation, packaging, and carefully scoped new providers.

## Before you start

Please open an issue first for large behavioral changes, new shell adapters, new persistent data, network-dependent features, or changes to privacy/safety behavior.

Small bug fixes, tests, documentation improvements, and narrowly scoped command-schema additions can go directly to a pull request.

## Project invariants

Please preserve these rules:

- suggestions never execute automatically;
- Bash remains in control of command execution;
- core completion remains local-first and does not require network access;
- no telemetry or command-history upload;
- dynamic identifiers such as filesystem paths, Git refs, Docker containers, SSH hosts, systemd units, package names, and project scripts must come from the current machine/context rather than hardcoded fixtures;
- accepted dynamic identifiers must not be persisted in adaptive ranking;
- existing user config must not be overwritten by installers;
- Bash 5.0+ remains the supported shell baseline for the v0.x Bash adapter;
- normal pushes and pull requests do not run CI; contributors must run the local checks below.

## Development setup

Requirements:

- Linux
- Rust toolchain with rustfmt
- Bash 5.0+
- Git
- dpkg-deb for the full release-readiness gate

Clone and verify:

```bash
git clone https://github.com/Anuppaul/termsense.git
cd termsense
cargo fmt --check
cargo test
bash scripts/check-local.sh
```

Before opening a pull request, run the full gate when your environment supports it:

```bash
bash scripts/release-readiness.sh
```

## Making a change

1. Create a focused branch.
2. Keep the change narrowly scoped.
3. Add or update tests for behavior changes.
4. Run rustfmt and the relevant local checks.
5. Do not commit generated personal caches, local paths, machine resources, or shell history.
6. Open a pull request describing the user-visible behavior before and after the change.

Example:

```bash
git checkout -b feat/rsync-schema
cargo fmt
cargo test
bash scripts/check-local.sh
git status
```

## Command schemas and descriptions

For dedicated command schemas, every known option or subcommand should have a useful one-line description of what it actually does.

Good:

```text
git commit --amend — Replace the tip commit with a new commit
```

Avoid category-only labels such as:

```text
git commit --amend — Git commit option
```

When adding a schema:

- keep it generic, never machine- or project-specific;
- add contextual tests;
- add behavior-specific descriptions;
- preserve active-token-only replacement;
- avoid executing the target command merely to discover completions.

## Dynamic providers

Dynamic providers must be bounded and failure-tolerant. If a provider shells out to a local tool, use a short timeout and treat failure as “no suggestions” rather than blocking the interactive shell.

Do not persist dynamic values into the usage-ranking state.

## Bash adapter changes

The Bash adapter is latency-sensitive and stateful. When changing `shell/termsense.bash`:

- run `bash -n shell/termsense.bash`;
- verify only one copy of each core function remains;
- test Tab, Enter, Right Arrow, Up/Down, Esc, and Ctrl+Space interactively;
- verify normal Enter still executes when no suggestion menu is active;
- test quoted paths and at least one nested context such as `sudo git che`.

## Pull request checklist

- [ ] Change is scoped and explained
- [ ] `cargo fmt --check` passes
- [ ] `cargo test` passes
- [ ] `bash scripts/check-local.sh` passes
- [ ] Relevant tests were added or updated
- [ ] No user-specific or machine-specific data was hardcoded
- [ ] No telemetry or automatic execution was introduced
- [ ] Documentation was updated when user-visible behavior changed

## Release workflow

The repository's GitHub Actions workflow is intentionally tag-only. Do not add normal push or pull-request CI without discussing it first.

Version tags are reserved for maintainers.

## Where help is especially welcome

Good contribution areas include:

- additional dedicated CLI schemas and exact one-line descriptions;
- shell-parser regression tests and edge cases;
- Bash renderer hardening;
- additional project/task-runner adapters;
- smarter cache invalidation;
- Zsh/Fish adapters as separately scoped work;
- documentation, examples, and packaging improvements.

If you are unsure whether an idea fits, open a feature request before implementing it.
