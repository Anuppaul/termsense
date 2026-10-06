# Governance

TermSense is currently a maintainer-led open-source project.

This document explains how decisions are made, how contributor responsibility can grow, and which changes require maintainer coordination.

## Project direction

The maintainer is responsible for:

- product scope and release direction;
- safety/privacy boundaries;
- shell-execution semantics;
- compatibility policy;
- release tags and published artifacts;
- final merge decisions.

The public roadmap is documented in [docs/ROADMAP.md](docs/ROADMAP.md).

## Contribution model

Contributors do not need prior permission for:

- small bug fixes;
- focused regression tests;
- documentation improvements;
- behavior-specific descriptions;
- narrowly scoped static command schemas.

Please open an issue before substantial work involving:

- a new shell adapter;
- new persistent state;
- privacy or telemetry behavior;
- network-dependent completion;
- new background processes;
- architecture changes spanning multiple subsystems;
- breaking CLI/config changes.

## Decision making

Technical decisions favor, in order:

1. command safety and user control;
2. privacy and local-first behavior;
3. predictable interactive latency;
4. correctness of shell context;
5. maintainability;
6. breadth of completion coverage.

For contested changes, maintainers may ask for a smaller prototype, benchmark, or regression test before accepting the design.

## Maintainer authority

Maintainers may:

- merge or close pull requests;
- request changes;
- label/triage issues;
- edit project documentation;
- publish releases;
- revert regressions;
- moderate project spaces under the Code of Conduct.

A technically valid contribution may still be declined when it expands scope too aggressively or conflicts with the product boundary.

## Growing contributor responsibility

Repeated high-quality contributions may lead to broader review or triage responsibility.

Signals include:

- reliable fixes with tests;
- thoughtful review of others' work;
- respect for project invariants;
- accurate issue triage;
- compatibility validation;
- sustained documentation or provider maintenance.

Any future maintainer additions will be recorded in [MAINTAINERS.md](MAINTAINERS.md).

## Releases

Version tags and release publication are maintainer responsibilities.

The release process is documented in [docs/RELEASE_PROCESS.md](docs/RELEASE_PROCESS.md).

## Conduct and security

Community behavior is governed by [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

Security-sensitive reports follow [SECURITY.md](SECURITY.md).
