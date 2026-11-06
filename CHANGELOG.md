# Changelog

## Unreleased

- restore native filesystem/folder suggestions for path-shaped command names and arguments (including `./`, `../`, `~/`, absolute paths, `git add`, editors, scripts, `sudo`, and chained commands); preserve empty dynamic usage keys correctly in the Bash suggestion protocol;
- add conditional `&&` full-command completion immediately after the operator (with or without a space), and suggest `sudo apt update && sudo apt upgrade` as a safe text-only continuation;
- prioritize full `sudo apt ...` templates over the exact `su` command when typing `su`, with regression coverage for the selected insertion range;
- fix PATH indexing of executable symlinks (including `sudo`/`apt`), invalidate stale command indexes, and fall back to local PATH resolution for full-command templates;
- progressively complete reusable full commands (e.g. `s`, `sudo`, `sudo a`, `sudo apt u` → `sudo apt update`), preserving the active-token insertion range and descriptions;
- keep the live suggestion overlay visible near the bottom of the terminal by reserving viewport rows before drawing;
- prioritize `sudo apt update` / `apt update` in a fresh APT subcommand palette;
- keep the APT palette available after a completed subcommand such as `sudo apt update ` by offering contextual options;
- warn when a custom install prefix is outside PATH using canonical directory comparison.

## 0.1.0 — 2026-10-06

Initial TermSense release scope:

- Linux-only native Rust suggestion engine;
- Bash/Readline live suggestion renderer with ghost text;
- Ctrl+Space IntelliSense-style command browser;
- PATH executables plus Bash builtins, aliases and user functions;
- full-command display with active-token-only insertion;
- shell-aware quotes, escaping, pipelines, separators and redirections;
- command, process and backtick substitution routing;
- open parenthesis/brace command-group routing;
- heredoc-body suppression;
- Git, systemd, journalctl, Docker, APT and SSH context providers;
- recursive SSH Include support with bounded glob expansion;
- filesystem, package, branch/ref, container, service/unit, user/group and project-task values;
- package.json, Cargo and Makefile project intelligence;
- common Git/Docker/systemctl/Cargo/find/grep/tar/curl schemas;
- generic local man-page option fallback;
- privacy-safe acceptance ranking with frequency and recency;
- persistent command/APT/man caches and session-scoped sensitive provider caches;
- native source installer/uninstaller;
- Debian package builder for amd64 and arm64;
- safe per-user configuration;
- local release-readiness gate;
- tag-only GitHub Actions release automation; normal pushes and pull requests remain local-verification only.

Released after local automated release-readiness verification passed with 66/66 tests, a clean release build, Bash syntax checks, binary smoke tests, install/uninstall lifecycle checks, Debian package generation, and interactive Bash UX validation.
