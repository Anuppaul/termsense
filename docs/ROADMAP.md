# TermSense Roadmap

This roadmap describes the direction of TermSense after the v0.1.0 release. It is a planning guide, not a promise of dates or exact version boundaries.

## Product principles

Work should preserve these invariants:

- suggestions never execute automatically;
- Bash or the active shell remains responsible for execution;
- core completion stays local-first and usable without network access;
- no telemetry or shell-history upload;
- dynamic machine/context values are discovered locally rather than hardcoded;
- dynamic identifiers are not persisted in adaptive ranking;
- interactive latency matters more than provider completeness;
- dedicated command intelligence should explain what a suggestion actually does.

## v0.1.x — hardening

Focus: make the released Bash experience more predictable without expanding the product boundary too aggressively.

### Parser and shell UX

- add regression coverage for more redirection combinations;
- harden multiline redraw behavior;
- improve compatibility with uncommon Readline bindings;
- test more quoted-path and nested substitution edge cases;
- improve terminal-width truncation and description alignment;
- preserve native behavior when TermSense has no active suggestions.

### Command intelligence

- expand dedicated schemas for common Linux/developer commands;
- replace remaining generic descriptions with behavior-specific one-liners;
- improve option-value models;
- improve local man-page metadata extraction without executing target commands.

### Reliability

- strengthen bounded subprocess behavior;
- add corruption recovery tests for local caches/state;
- document compatibility across major Linux distributions;
- improve packaging and installation diagnostics.

## v0.2 — broader context intelligence

Focus: richer local context while preserving the same privacy model.

Candidate work:

- more project/task-runner adapters;
- richer Git workflows and value completion;
- additional package-manager ecosystems;
- more structured systemd/journalctl contexts;
- smarter cache invalidation based on local source fingerprints;
- better ranking among semantically equivalent candidates;
- improved discovery of command-specific argument values.

Potential project adapters include tools such as Just, Go, Python project metadata, and other local manifests when they can be read safely without executing project code.

## v0.3 — additional shell adapters

Focus: reuse the Rust core rather than fork provider logic.

Candidate adapters:

- Zsh;
- Fish.

Each adapter must preserve the same safety boundary: suggestions insert text, never execute it.

The first adapter milestone should cover:

- current buffer + cursor query;
- suggestion rendering;
- navigation;
- accept without execution;
- dismissal;
- explicit command browser;
- native Enter behavior when no suggestion UI is active.

## Longer-term exploration

These are intentionally exploratory:

- typo-tolerant/fuzzy ranking;
- richer local documentation extraction;
- optional natural-language command assistance;
- signed release artifacts;
- distribution repository packaging;
- architecture for plugin-like provider modules;
- performance instrumentation that remains local and non-telemetric.

## Out of scope by default

The following should not be added casually:

- cloud-required completion;
- command telemetry;
- uploading shell history;
- automatic execution of suggested commands;
- silently running discovered project scripts to learn their names;
- persistent storage of local paths, hosts, containers, refs, or service names for ranking;
- autonomous command correction that changes a line without explicit user acceptance.

## How to contribute to the roadmap

Small fixes can go directly to a pull request.

For larger changes, open a feature request first and include:

1. the terminal workflow being improved;
2. an example before/after interaction;
3. any new local data source or subprocess involved;
4. latency implications;
5. privacy/persistence implications;
6. how the change will be tested.

See [CONTRIBUTING.md](../CONTRIBUTING.md).
