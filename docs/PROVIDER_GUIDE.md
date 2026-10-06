# Provider Development Guide

This guide explains how to add command intelligence to TermSense without breaking its local-first and suggestion-only model.

## Choose the right provider type

There are three common cases.

### 1. Static command schema

Use a static schema when the vocabulary is generic across machines.

Examples:

- `git commit --amend`
- `docker logs --follow`
- `grep --recursive`

Static schema values belong in `src/providers.rs`.

Every dedicated option/subcommand should have a behavior-specific one-line description.

Good:

```text
git commit --dry-run — Show what would be committed without creating a commit
```

Avoid:

```text
git commit --dry-run — Git commit option
```

### 2. Dynamic local provider

Use a dynamic provider when values depend on the current machine or project.

Examples:

- Git refs;
- Docker containers;
- systemd units;
- SSH hosts;
- filesystem paths;
- package.json scripts.

Never hardcode those values.

Dynamic providers should return no suggestions when the source is unavailable or times out.

### 3. Generic local metadata fallback

Use local documentation metadata when a dedicated schema would be too broad.

The current example is the man-page option cache.

Do not execute an arbitrary target command merely to discover completion metadata.

## Provider invariants

A provider must not:

- execute a suggested command;
- upload query text;
- require network access for core completion;
- hardcode machine-specific identifiers;
- persist dynamic values into adaptive ranking;
- block the interactive shell for an unbounded amount of time.

## Static schema pattern

A typical static schema has:

1. a constant slice of allowed values;
2. context routing that selects the schema;
3. calls to the common candidate push path;
4. descriptions in `candidate_description`;
5. contextual tests.

Conceptually:

```rust
const EXAMPLE_OPTIONS: &[&str] = &[
    "--dry-run",
    "--verbose",
];

match active_command {
    "example" => push_schema_matches(...),
    _ => {}
}
```

The exact helper names may evolve; follow nearby providers in `src/providers.rs`.

## Descriptions

Descriptions should answer “what does this do?” in one short line.

Prefer:

```text
--dry-run — Show planned changes without applying them
```

Avoid:

```text
--dry-run — Example option
```

Descriptions should:

- describe behavior, not category;
- avoid guarantees the underlying command does not make;
- stay concise enough for terminal display;
- avoid machine-specific values.

## Dynamic subprocesses

If a provider needs a local subprocess:

- use a hard timeout;
- close stdin;
- capture only required output;
- suppress irrelevant stderr;
- treat non-zero exit/timeout as no candidates;
- avoid network-triggering flags;
- do not invoke project scripts for discovery.

The Git/Docker/systemd providers are useful references.

## Dynamic cache

Use the runtime cache only for short-lived machine/context values.

The cache:

- lives under `$XDG_RUNTIME_DIR/termsense`;
- is optional;
- uses short TTLs;
- is cleared by `termsense index`;
- is not adaptive history.

Do not move sensitive dynamic data into the persistent usage state.

## Usage ranking

A generic candidate may return a derived usage key.

A dynamic candidate must return an empty usage key.

Examples of values that must not be persisted:

- `feature/customer-name` Git branches;
- `prod-server` SSH hosts;
- `redis-cache` containers;
- `/home/user/private-project` paths;
- project-specific npm scripts;
- locally installed systemd units.

## Context routing

Provider selection should be based on parsed tokens, not string slicing of the full shell line.

Use `shell_parse::active_context` so that nested contexts and separators remain correct.

A provider should preserve full display context while replacing only the active logical token.

## Tests

At minimum, add a contextual test:

```rust
#[test]
fn example_options_are_contextual() {
    let candidates = super::suggest(
        &[],
        &UsageState::default(),
        "example --dr",
        12,
        20,
    );

    assert!(candidates.iter().any(|candidate| {
        candidate.display_text == "example --dry-run"
            && candidate.description == "Show planned changes without applying them"
    }));
}
```

For a dedicated schema, also make sure options do not fall back to a category-only description.

Run:

```bash
cargo fmt
cargo test
bash scripts/check-local.sh
```

Before a release-level change:

```bash
bash scripts/release-readiness.sh
```

## Checklist for a new provider

- [ ] Generic vs dynamic boundary is correct
- [ ] No user/project-specific value is hardcoded
- [ ] Context is parser-derived
- [ ] Subprocesses are bounded
- [ ] Failure produces no candidates instead of shell breakage
- [ ] Descriptions explain behavior
- [ ] Active-token replacement remains correct
- [ ] Dynamic values have no persistent usage key
- [ ] Unit tests cover positive and irrelevant contexts
- [ ] Local checks pass
