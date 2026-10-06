# Security Policy

## Supported versions

Security fixes are currently targeted at the latest released TermSense version and the current `main` branch.

| Version | Supported |
| --- | --- |
| 0.1.x | Yes |
| Older unreleased snapshots | Best effort |

## Reporting a vulnerability

Please do **not** open a public issue for a vulnerability that could expose user data, execute commands unexpectedly, cross a trust boundary, or compromise the local machine.

Use GitHub's private security advisory flow for this repository:

https://github.com/Anuppaul/termsense/security/advisories/new

Include, when available:

- affected TermSense version/commit;
- Linux/Bash environment;
- reproduction steps;
- expected vs actual behavior;
- security impact;
- whether command execution is possible;
- whether sensitive local data can be exposed or persisted;
- a minimal proof of concept that avoids real secrets.

Please remove passwords, access tokens, private SSH material, personal paths, and other unrelated sensitive information.

## Security-sensitive areas

TermSense treats these areas as security-sensitive:

- shell parsing and active-token replacement;
- Readline bindings that could execute rather than insert text;
- subprocess invocation in dynamic providers;
- filesystem and SSH-host discovery;
- persistent usage state;
- cache path construction;
- installer modifications to shell rc files;
- release/package integrity.

## Security invariants

A valid change must preserve these properties:

- suggestions do not execute automatically;
- TermSense inserts text only after explicit user acceptance;
- dynamic provider subprocesses are bounded;
- arbitrary discovered binaries are not executed during indexing;
- shell history is not uploaded;
- no telemetry is required for completion;
- sensitive dynamic identifiers are not persisted for adaptive ranking;
- cache keys cannot escape their intended directories;
- existing user configuration is not silently overwritten.

## Disclosure process

After receiving a private report, maintainers will attempt to:

1. reproduce and assess the issue;
2. determine affected versions;
3. prepare a fix and regression coverage;
4. release the fix;
5. coordinate public disclosure when appropriate.

Response times are best effort; TermSense is currently maintained as an open-source project rather than a staffed security service.
