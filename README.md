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
- APT subcommands, options and locally cached package-name completion;
- shell-aware token parsing for open single/double quotes and backslash-escaped spaces;
- Bash builtin, alias and user-function command discovery;
- nested sudo context, including local user/group completion and nested command routing;
- common Linux CLI schemas for find, grep, tar and curl;
- option-value path completion such as ssh -i, tar -f and curl -o;
- command-name argument intelligence for which/whereis/type/command/man;
- active command-segment parsing across pipes, &&, ||, semicolons and background separators;
- native user-local install/uninstall scripts;
- local Debian package builder for amd64/arm64;
- filesystem completion for input/output redirection targets;
- innermost command routing inside open $() and backtick command substitutions;
- richer typed positional values for find/grep/tar/curl;
- process-substitution and command-group context routing;
- ephemeral runtime caching for expensive dynamic Git/Docker/systemd providers;
- local release-readiness gate with Rust, Bash, binary-smoke and Debian-package checks;
- heredoc-body suggestion suppression;
- recursive SSH Include discovery with bounded local glob expansion;
- adaptive ranking with bounded frequency + recency decay;
- safe per-user config file for renderer behavior.

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

Quoted and escaped paths are parsed as one shell token:

```text
$ cat "My Doc
    > cat "My Documents/
      cat "My Document.txt

$ cat My\\ Doc
    > cat My\\ Documents/
```

The active quote style is preserved while TermSense replaces only the logical token body.

Bash-native command discovery is also included:

```text
$ c
    > cd
      command
      compgen
      ...
```

The command pool merges `PATH` executables with Bash builtins, aliases and user-facing shell functions. Internal underscore-prefixed completion functions are intentionally excluded from the exported shell context.

Nested `sudo` context is parsed instead of treating every token after `sudo` as a raw command:

```text
$ sudo -u ro
    > sudo -u root

$ sudo -H git che
    > sudo -H git checkout
      sudo -H git check-ignore
```

More generic CLI intelligence now includes:

```text
$ find ./ -na
    > find ./ -name

$ grep -r
    > grep -r
      grep -R
      grep --recursive

$ tar -x
    > tar -xf
      tar -xvf

$ curl --hea
    > curl --head
      curl --header

$ ssh -i ~/.ssh/id_
    > ssh -i ~/.ssh/id_ed25519
      ssh -i ~/.ssh/id_rsa

$ which do
    > which docker
      which domainname

$ type c
    > type cd
      type command
      type curl
```

Filesystem entries, SSH hosts, units and package names are only shown when they exist in the current machine's local data sources.

SSH host discovery follows local `Include` directives recursively with a depth guard and visited-file set. Common include layouts such as:

```text
Include ~/.ssh/config.d/*.conf
```

are expanded locally. Wildcard `Host *` declarations are not surfaced as concrete host suggestions, and hashed known-host entries remain hidden.

Pipeline and command separators route suggestions only inside the active right-hand segment:

```text
$ cat file | gre
    > cat file | grep

$ git status && docker lo
    > git status && docker login
      git status && docker logout
      git status && docker logs

$ pwd; cd ~/Doc
    > pwd; cd ~/Documents/

$ grep "a|b" fi
    > grep "a|b" file.txt
```

Separators inside quotes or escaped separators are not treated as command boundaries.

Redirection targets are treated as filesystem arguments without losing the surrounding command:

```text
$ echo hello > lo
    > echo hello > logs/
      echo hello > local.txt

$ command 2>> /var/lo
    > command 2>> /var/log/

$ sort < da
    > sort < data.txt

$ command 2>/dev/nu
    > command 2>/dev/null
```

Descriptor duplication such as `2>&1` is recognized as descriptor syntax rather than a filesystem target.

Heredoc bodies are treated as data, not shell command text:

```text
cat <<EOF
plain text here
EOF
```

TermSense keeps normal completion on the heredoc declaration line, suppresses suggestions while the cursor is inside an open heredoc body, and resumes command completion after the matching delimiter. `<<-EOF` tab-stripping delimiters are also recognized.

Open command substitutions route completion to the innermost command while preserving the outer line:

```text
$ echo $(git che
    > echo $(git checkout
      echo $(git check-ignore

$ printf '%s' "$(docker lo
    > printf '%s' "$(docker logs

$ echo `git che
    > echo `git checkout
```

Process substitutions and open command groups also route to the innermost executable context:

```text
$ diff <(git che
    > diff <(git checkout

$ tee >(grep --r
    > tee >(grep --recursive

$ ( git che
    > ( git checkout

$ { docker lo
    > { docker logs
```

Nested forms use the innermost active frame. Arithmetic expansion such as `$((1 + 2))` is deliberately not treated as command substitution.

Richer positional models distinguish option values from ordinary paths:

```text
$ find ./ -type d
    > find ./ -type d

$ find ./ -user ro
    > find ./ -user root

$ find ./ -newer bu
    > find ./ -newer build.log

$ grep -f pat
    > grep -f patterns.txt

$ tar -czf arc
    > tar -czf archive.tar.gz

$ tar --file=arc
    > tar --file=archive.tar.gz

$ curl --output=do
    > curl --output=download.bin

$ curl -H "Accept:
    # free-form header value: no bogus filesystem suggestion
```

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

## Install from source

For a native user-local install:

```bash
bash scripts/install.sh
```

Default install location:

```text
~/.local/bin/termsense
```

The installer builds the release binary, installs it, creates the default user config only when missing, indexes local commands, and manages one idempotent block in `~/.bashrc`. Existing TermSense config is never overwritten.

Useful variants:

```bash
bash scripts/install.sh --no-shell
bash scripts/install.sh --prefix /custom/prefix
bash scripts/install.sh --skip-build
```

Uninstall:

```bash
bash scripts/uninstall.sh
```

Remove binary, Bash integration, cache/state/config:

```bash
bash scripts/uninstall.sh --purge
```

The installer/uninstaller preserve the existing Bash rc file permissions.

## Debian package

Build a local `.deb`:

```bash
bash scripts/package-deb.sh
```

Output example:

```text
dist/termsense_0.1.0_amd64.deb
```

Install it:

```bash
sudo apt install ./dist/termsense_0.1.0_amd64.deb
```

The Debian package installs the native binary under `/usr/bin/termsense` and a reference default config under `/usr/share/termsense/default.conf`, but deliberately does **not** modify a specific user's dotfiles while running as root. Enable Bash for the user explicitly:

```bash
echo 'eval "$(termsense init bash)"' >> ~/.bashrc
source ~/.bashrc
```

The package builder auto-maps `x86_64 → amd64` and `aarch64/arm64 → arm64`. No GitHub Actions or remote build service is involved.

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

User-local installs create this file only when it does not already exist:

```text
~/.config/termsense/config.conf
```

or under `$XDG_CONFIG_HOME/termsense/config.conf`.

Default:

```text
auto_suggest=1
max_visible=5
ghost=1
ctrl_space=1
```

The Bash adapter parses only these allowlisted keys. The config file is **not sourced or eval'd**.

Environment variables override config-file values when already set:

```bash
export TERMSENSE_AUTO_SUGGEST=0
export TERMSENSE_MAX_VISIBLE=8
export TERMSENSE_GHOST=0
export TERMSENSE_CTRL_SPACE=1
```

`max_visible` is clamped to 1–20. Set `ctrl_space=0` to leave Ctrl+Space unbound by TermSense.

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

A manual `termsense index` also clears short-lived dynamic provider cache entries so Git refs, Docker containers and systemd units are refreshed immediately.

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

Dynamic providers use a separate **ephemeral runtime cache** only when `$XDG_RUNTIME_DIR` exists:

```text
$XDG_RUNTIME_DIR/termsense/
```

Current TTLs are intentionally short:

```text
Git refs          ~2 seconds
Docker containers ~2 seconds
systemd units     ~10 seconds
```

This cache is session-scoped and is not used as adaptive history. Sensitive dynamic values are not copied into the persistent ranking state. If no XDG runtime directory exists, TermSense simply runs those providers without this cache.

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
$XDG_STATE_HOME/termsense/usage-v2.json
```

or:

```text
~/.local/state/termsense/usage-v2.json
```

They do not contain the complete typed command line. Ranking combines a bounded acceptance-frequency boost with a bounded recency boost. Recent accepted generic suggestions can rise within otherwise similar prefix matches, while exact textual matches still dominate.

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

The automatic renderer hooks ASCII printable keystrokes through Readline macros so it can refresh after normal insertion. Bracketed paste remains handled by Readline as a single paste operation. Non-ASCII input remains native Bash input and can still use explicit Ctrl+Space discovery.

The parser now understands open single/double quotes, backslash-escaped characters, active segments separated by `|`, `&&`, `||`, `;`, or background `&`, common file redirections, and open `$(...)` / backtick command substitutions. Separator characters inside quotes or escaped separators do not split the active context.

It is still deliberately not a full Bash AST. Heredoc bodies, closed-group execution semantics, arithmetic expansion semantics, every process/redirection edge case, and complete compound-shell grammar remain future parser work.

Multiline redraw hardening and broader shell/keymap compatibility also remain active implementation work.

## Next

The next provider work extends the same generic context model with:

- richer closed-group/compound-shell semantics beyond the active open-frame model;
- more option-value schemas and positional argument models;
- additional safe local host sources beyond SSH config/known_hosts;
- additional provider cache invalidation signals beyond short TTLs;
- additional safe project manifests and task runners;
- reproducible release metadata and signed package publishing.

## Local verification

No GitHub Actions workflow is used.

Fast local checks:

```bash
bash scripts/check-local.sh
```

Full release-readiness gate:

```bash
bash scripts/release-readiness.sh
```

The release gate verifies package identity/version/license, forbids workflow files under the current no-CI policy, runs rustfmt/tests/release build, checks all Bash scripts, smoke-tests contextual suggestions, generates Bash integration and syntax-checks it, builds a temporary Debian package, and verifies its package/version metadata.

## CI policy

There is intentionally no GitHub Actions workflow in the repository at this stage. Initial development verification is local only.


## APT package cache

APT package suggestions never perform a network request. When an APT package argument is first requested, TermSense tries the local `apt-cache pkgnames` command with a 450 ms hard timeout and stores the result for up to 24 hours:

```text
$XDG_CACHE_HOME/termsense/apt-packages-v1.txt
```

or:

```text
~/.cache/termsense/apt-packages-v1.txt
```

If `apt-cache` is unavailable or does not return usable data, TermSense falls back to installed package names parsed from `/var/lib/dpkg/status`.
