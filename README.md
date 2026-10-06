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
- optional `fzf` browser with a dependency-free numbered fallback;
- Git subcommand suggestions such as `git che` → `checkout` / `check-ignore`;
- current-repository Git ref suggestions for commands such as `git checkout`;
- systemctl subcommand suggestions and local systemd unit discovery;
- Docker subcommand suggestions and actual local container-name discovery;
- real filesystem directory completion for `cd`, including `~/...` paths;
- bounded local subprocesses for Git/Docker providers so provider failures do not hang the shell;
- full-command suggestion display while retaining token-safe replacement;
- generic Cargo subcommand intelligence;
- local `package.json` script discovery for pnpm/npm/yarn/bun;
- local Makefile target discovery;
- contextual flag/option suggestions for common Git, Docker, systemctl and Cargo flows;
- local accepted-suggestion ranking using privacy-safe derived keys rather than raw command history;
- generic filesystem completion for common file commands such as cat/cp/mv/rm/ls/nano/vim;
- SSH host completion from local ~/.ssh/config and unhashed known_hosts entries;
- journalctl option and local systemd-unit completion;
- APT subcommands, options and locally cached package-name completion.

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

The bracketed part above represents dim ghost text. Suggestion rows show the **complete command**, not only the token being completed.

For example:

```text
$ sudo git che[ckout]
    > sudo git checkout
      sudo git check-attr
      sudo git check-ignore
      sudo git check-ref-format
```

Internally TermSense still replaces only the active token, so the existing `sudo git ` prefix is preserved safely.

Option completion follows the same full-command display rule:

```text
$ git commit --a
    > git commit --all
      git commit --amend
      git commit --author=

$ docker logs --f
    > docker logs --follow

$ systemctl --u
    > systemctl --user
```

Broader argument intelligence follows the same rule:

```text
$ cat ~/Doc
    > cat ~/Documents/
      cat ~/Document.txt

$ sudo rm ./tem
    > sudo rm ./temp/
      sudo rm ./template.txt

$ ssh pro
    > ssh prod
      ssh proxy

$ ssh deploy@pro
    > ssh deploy@prod
      ssh deploy@proxy

$ journalctl -u ng
    > journalctl -u nginx.service

$ journalctl --unit=ng
    > journalctl --unit=nginx.service

$ apt ins
    > apt install

$ sudo apt install pos
    > sudo apt install postgresql
      sudo apt install postgresql-client
```

Filesystem entries, SSH hosts, units and package names are only shown when they exist in the current machine's local data sources.

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

## Genericity invariant

TermSense never ships personal project names, repository names, container names, service names, or user-specific paths as built-in suggestions.

Dynamic suggestions come only from the machine and context where TermSense is running:

```text
git checkout fe
             feature/...     # only if that ref exists in the current repository

systemctl restart ng
                  nginx.service   # only if that unit exists locally

docker logs re
            redis-cache      # only if that container exists in the local Docker daemon

cd ~/Doc
      ~/Documents/           # only if that directory exists
```

Static schemas contain only generic CLI vocabulary such as Git, systemctl, Docker, and Cargo subcommands.

Project-aware suggestions are also machine-derived. For example, inside a project TermSense can read local manifests:

```text
pnpm d
     → pnpm dev          # only if "dev" exists in package.json

npm run te
        → npm run test   # only if "test" exists in package.json

cargo bu
      → cargo build

make bu
     → make build        # only if "build" exists in the discovered Makefile
```

TermSense reads these files; it does not execute project scripts merely to discover their names.

## Local usage ranking

TermSense can learn which suggestions you actually accept. It does **not** import or copy your raw Bash history.

When a suggestion is accepted, the Bash adapter records a derived key such as:

```text
path:command:docker
git-schema:subcommand:checkout
docker-logs-schema:option:--follow
```

These keys are stored locally under:

```text
$XDG_STATE_HOME/termsense/usage-v1.json
```

or:

```text
~/.local/state/termsense/usage-v1.json
```

They do not contain the complete typed command line. Usage boosts are deliberately capped so an exact textual match still outranks a merely frequent prefix match.

Dynamic identifiers are excluded from adaptive persistence entirely. TermSense does not put SSH hosts, filesystem paths, Git refs, Docker container names, systemd unit names, APT package names, or project-specific script/target names into the usage-ranking state. Those values are discovered transiently when relevant.

Example:

```text
$ d
    > docker
      date
      df
      diff
      du
```

Only installed `d...` commands are candidates. If `docker` is repeatedly accepted, it can rise above other prefix matches without replacing exact-match semantics.

## Current Bash renderer boundary

The first automatic renderer hooks ASCII printable keystrokes through Readline macros so it can refresh after normal insertion. Bracketed paste remains handled by Readline as a single paste operation. Non-ASCII input remains native Bash input and can still use explicit Ctrl+Space discovery.

This renderer is intentionally an initial vertical slice. Context-aware arguments, multiline redraw hardening, and broader shell/keymap compatibility remain active implementation work.

## Next

The next provider work extends the same generic context model with:

- quoted/escaped shell-token parsing for paths containing whitespace;
- more option/flag schemas;
- SSH Include-file expansion and additional safe local host sources;
- short-lived caches for more expensive dynamic providers;
- additional safe project manifests and task runners;
- ranking decay/recency without storing raw shell history.

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


## APT package cache

APT package suggestions never perform a network request. When an APT package argument is first requested, TermSense tries the local `apt-cache pkgnames` command with a hard timeout and stores the result for up to 24 hours:

```text
$XDG_CACHE_HOME/termsense/apt-packages-v1.txt
```

or:

```text
~/.cache/termsense/apt-packages-v1.txt
```

If `apt-cache` is unavailable or does not return usable data, TermSense falls back to installed package names parsed from `/var/lib/dpkg/status`.
