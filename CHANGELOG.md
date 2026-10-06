# Changelog

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
