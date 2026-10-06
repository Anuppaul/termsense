# Compatibility

TermSense v0.1 is a Linux-only native application with a Bash/Readline integration.

This document separates **supported by design** from **verified in the release process**.

## Support matrix

| Area | Status | Notes |
| --- | --- | --- |
| Linux | Supported | The Rust binary intentionally exits on non-Linux platforms. |
| Bash 5.0+ | Supported | Required by the current interactive adapter. |
| Bash 4.x | Unsupported | Installer/adapter reject the unsupported baseline. |
| Zsh | Not yet supported | Planned as a separately scoped adapter. |
| Fish | Not yet supported | Planned as a separately scoped adapter. |
| x86_64 / amd64 | Supported | Release workflow publishes an amd64 Debian package and binary tarball. |
| aarch64 / arm64 | Package builder supported | Local Debian package script maps arm64; public v0.1 workflow currently publishes amd64 artifacts. |
| Debian/Ubuntu-style dpkg systems | Packaging supported | `dpkg-deb` is required to build the .deb. |
| Other Linux distributions | Source install supported | Native source install does not depend on apt/dpkg except for Debian packaging. |
| GNU Readline Bash terminals | Primary target | Current renderer uses Bash Readline bindings and ANSI terminal control sequences. |
| vi-mode Readline customizations | Partial/needs more testing | Broader keymap compatibility is post-v0.1 work. |
| Non-ASCII interactive typing | Conservative | Normal Bash input remains native; explicit Ctrl+Space discovery can still be used. |
| Bracketed paste | Native Readline behavior | TermSense does not attempt to reinterpret paste as individual keystrokes. |

## Release verification

The v0.1.0 release process verified:

- Rust formatting;
- Rust unit tests;
- optimized release build;
- Bash script syntax;
- generated Bash integration syntax;
- contextual suggestion smoke tests;
- installer/uninstaller lifecycle;
- Debian package metadata/content;
- tag-triggered GitHub release packaging.

The public release workflow runs on GitHub-hosted Ubuntu and publishes:

```text
termsense_<version>_amd64.deb
termsense_<version>_linux_amd64.tar.gz
SHA256SUMS
```

## Local environment requirements

Source build:

- Linux;
- Rust/Cargo toolchain.

Interactive Bash integration:

- Bash 5.0 or newer;
- an interactive terminal using Bash Readline;
- ANSI terminal behavior for the current overlay renderer.

Full release-readiness gate additionally expects tools such as:

- `git`;
- `awk`;
- `grep`;
- `find`;
- `install`;
- `dpkg-deb`.

## Optional provider dependencies

TermSense degrades gracefully when a local provider tool is absent.

Examples:

- Git-specific dynamic refs require `git`;
- Docker container discovery requires the Docker CLI/daemon to be usable;
- systemd unit discovery depends on systemd tooling;
- APT package metadata is richer when `apt-cache` is available;
- generic man-page option discovery requires `man`.

Missing optional provider tools should remove those candidates rather than break core command completion.

## Reporting compatibility issues

When filing a bug, include:

- TermSense version;
- Linux distribution;
- Bash version;
- terminal emulator;
- relevant Readline mode/custom bindings;
- minimal typed prefix and key sequence;
- sanitized `termsense status` / `termsense doctor` output when useful.

Do not include secrets, private hostnames, tokens, or sensitive shell history.
