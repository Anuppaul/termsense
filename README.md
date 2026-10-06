# TermSense

**Context-aware inline command suggestions for Linux terminals.**

TermSense is a Linux-only terminal intelligence package. It discovers commands available on the current machine and adds IDE-style discovery to an interactive shell without replacing Bash or executing suggestions automatically.

> Status: early implementation / v0.1 development.

## Implemented now

- Linux-only Rust CLI;
- executable discovery from `PATH`;
- cached local command index with PATH-directory fingerprints;
- deterministic prefix ranking;
- cursor-aware replacement ranges;
- command discovery after `sudo `;
- plain-text and JSON suggestion output;
- `termsense list-commands`;
- `termsense suggest`;
- `termsense index`;
- `termsense status`;
- `termsense doctor`;
- Bash initialization;
- automatic suggestion list while typing;
- inline dim ghost suffix for the selected suggestion;
- Up / Down selection while suggestions are active;
- Tab accepts the selected candidate;
- Right Arrow accepts the ghost candidate;
- Esc dismisses the live menu;
- **Ctrl+Space** opens the larger command browser;
- optional `fzf` browser with a dependency-free numbered fallback.

The complete product contract is in [docs/CONCEPT.md](docs/CONCEPT.md).

## Current terminal UX

Typing a prefix automatically queries the local index:

```text
$ sys[temctl]
    > systemctl
      sysctl
      systemd-run
      systemd-analyze
```

The bracketed part above represents dim ghost text.

Controls:

| Key | Action |
| --- | --- |
| type | refresh live suggestions |
| Up / Down | change selected suggestion |
| Tab | accept selected suggestion |
| Right Arrow | accept ghost suggestion |
| Esc | dismiss suggestions |
| Ctrl+Space | open the larger IntelliSense-style command browser |
| Enter | normal Bash execution |

At an empty prompt, TermSense intentionally stays quiet. Press `Ctrl+Space` to browse commands explicitly.

## Build locally

Requirements:

- Linux;
- Rust toolchain / Cargo;
- Bash for the initial shell integration.

```bash
git clone https://github.com/Anuppaul/termsense.git
cd termsense
cargo build --release
```

For development:

```bash
cargo install --path .
```

Refresh the command index:

```bash
termsense index
```

Inspect it:

```bash
termsense status
termsense doctor
```

## Enable Bash integration

For the current interactive shell:

```bash
eval "$(termsense init bash)"
```

To enable it on future Bash sessions, add:

```bash
if command -v termsense >/dev/null 2>&1; then
  eval "$(termsense init bash)"
fi
```

to `~/.bashrc`.

Most Linux terminal emulators encode **Ctrl+Space** as the same control byte as `Ctrl+@`; the Bash adapter binds both forms.

## Configuration

The first renderer exposes simple environment switches:

```bash
export TERMSENSE_AUTO_SUGGEST=1
export TERMSENSE_MAX_VISIBLE=5
export TERMSENSE_GHOST=1
```

Set `TERMSENSE_AUTO_SUGGEST=0` to keep explicit `Ctrl+Space` discovery while disabling automatic popup refresh.

Set `TERMSENSE_GHOST=0` to keep the list but hide inline ghost text.

## Optional fzf

If `fzf` is installed, Ctrl+Space uses it as a searchable larger browser. It is not required by the core package.

Without `fzf`, TermSense shows a dependency-free numbered candidate picker.

## Engine examples

List commands:

```bash
termsense list-commands
```

Prefix query:

```bash
termsense suggest sys
```

Pass an entire shell buffer and cursor:

```bash
termsense suggest "sudo sys" --cursor 8
```

Machine-readable result:

```bash
termsense suggest git --json
```

Force refresh after package installation:

```bash
termsense index
```

## Index behavior

The base index records executable commands reachable through the current `PATH`.

The cache lives under:

```text
$XDG_CACHE_HOME/termsense/commands-v1.json
```

or, when `XDG_CACHE_HOME` is unset:

```text
~/.cache/termsense/commands-v1.json
```

TermSense fingerprints PATH directories before reusing the cache, so ordinary package installs/removals can invalidate it without rescanning every directory on every keystroke.

## Safety and privacy

TermSense:

- does not execute a selected suggestion;
- does not require network access for core completion;
- does not upload command lines or shell history;
- does not run arbitrary discovered binaries during PATH indexing;
- only inserts text into Readline; Bash still executes the final line after the user presses Enter.

## Current Bash renderer boundary

The first automatic renderer hooks ASCII printable keystrokes through Readline macros so it can refresh after normal insertion. Bracketed paste remains handled by Readline as a single paste operation. Non-ASCII input remains native Bash input and can still use explicit Ctrl+Space discovery.

This renderer is intentionally an initial vertical slice. Context-aware arguments, multiline redraw hardening, and broader shell/keymap compatibility remain active implementation work.

## Next

The next provider slice adds context beyond the first command token:

- filesystem paths;
- Git branches/refs;
- systemd services;
- Docker containers;
- project tasks/scripts;
- local history ranking.

## Local verification

No GitHub Actions workflow is used.

Run development checks locally:

```bash
cargo fmt --check
cargo test
bash -n shell/termsense.bash
```

## CI policy

There is intentionally no GitHub Actions workflow in the repository at this stage. Initial development verification is local only.
