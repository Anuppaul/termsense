# TermSense Architecture

TermSense is split into a native Rust suggestion engine and a thin Bash/Readline integration layer.

The design goal is simple: understand the current command-line context, produce ranked text candidates, and let the shell adapter render or insert them without taking control of command execution.

## High-level flow

```text
interactive Bash
    |
    | current READLINE_LINE + cursor
    v
shell/termsense.bash
    |
    | termsense suggest <buffer> --cursor <byte> --limit <n>
    v
Rust CLI (src/main.rs)
    |
    +--> command index
    +--> shell parser
    +--> context providers
    +--> local caches
    +--> usage ranking
    |
    v
ranked Candidate[]
    |
    | TSV protocol (or JSON for explicit CLI use)
    v
Bash renderer
    |
    +--> ghost suffix
    +--> visible suggestion rows
    +--> navigation
    +--> active-token replacement
    |
    v
Readline buffer

Bash executes only after the user explicitly submits the final line.
```

## Rust entry point — `src/main.rs`

The CLI exposes the core engine:

- `list-commands`
- `suggest`
- `index`
- `status`
- `doctor`
- `init bash`
- hidden `record` for accepted privacy-safe usage keys

The command index scans executable entries reachable through `PATH` and caches the result with PATH-directory fingerprints.

The `suggest` command accepts the full shell buffer and a UTF-8 byte cursor. Candidates are produced by `providers::suggest`.

### Plain shell protocol

The Bash adapter consumes tab-separated candidate rows with nine fields:

1. insertion text;
2. full display text;
3. candidate kind;
4. source/provider;
5. score;
6. replacement start;
7. replacement end;
8. privacy-safe usage key;
9. one-line description.

Replacement offsets are converted to character offsets for Bash 5 Readline compatibility while the Rust parser internally uses byte-safe UTF-8 boundaries.

## Shell parser — `src/shell_parse.rs`

The parser is intentionally narrower than a complete Bash AST.

It identifies the active command context around the cursor and understands enough shell structure to avoid obvious context mistakes.

Current capabilities include:

- single and double quotes;
- escaped characters;
- pipelines;
- `&&`, `||`, semicolons, background separators, and newlines;
- common redirections;
- `$()` command substitution;
- backtick substitution;
- process substitution;
- parenthesis and brace command groups;
- heredoc-body suppression;
- arithmetic-expansion suppression.

The parser returns an `ActiveContext` containing logical tokens, redirection-target state, and a suppression flag.

The key architectural rule is **fail safe**: contexts that TermSense cannot interpret confidently may suppress suggestions rather than invent shell semantics.

## Providers — `src/providers.rs`

Providers turn an `ActiveContext` into candidate values.

### Static schemas

Static schemas contain generic command vocabulary such as:

- Git subcommands and selected options;
- systemctl;
- Docker;
- Cargo;
- APT;
- journalctl;
- SSH;
- sudo;
- find;
- grep;
- tar;
- curl;
- TermSense itself.

Static schemas may include option descriptions, but they must never contain personal project names or machine-specific resources.

### Dynamic local providers

Dynamic candidates are read from the current machine/context, including:

- Git refs from the active repository;
- Docker containers;
- systemd units;
- SSH configuration;
- filesystem entries;
- local APT metadata;
- package.json scripts;
- Makefile targets;
- local users/groups where relevant.

Dynamic values are not shipped as fixtures and are not persisted into adaptive ranking.

### Generic fallback

Commands without a dedicated option schema may use local man-page metadata.

TermSense invokes `man`, not the target command, with a bounded timeout and a local cache.

## Candidate model

A candidate carries:

- insertion text;
- full line display text;
- one-line description;
- semantic kind;
- provider source;
- score;
- replacement range;
- derived usage key.

Display and insertion are deliberately separate.

For example:

```text
buffer:       sudo git che
display:      sudo git checkout
insert_text:  checkout
replace:      only the active "che" token
```

This preserves surrounding shell syntax.

## Ranking — `src/usage.rs`

TermSense combines deterministic prefix relevance with a bounded local usage boost.

Accepted generic suggestions may record derived keys such as:

```text
git-schema:subcommand:checkout
docker-logs-schema:option:--follow
```

TermSense does not persist the complete command line.

Dynamic identifiers such as paths, hosts, Git refs, container names, systemd units, package names, and project-specific tasks receive no persistent usage key.

The usage state is capped and stored under the user's XDG state directory.

## Caches

TermSense uses separate cache classes because they have different privacy and freshness properties.

### Command index

Persistent cache under the XDG cache directory.

Stores generic executable discovery and PATH fingerprints.

### APT/man metadata

Persistent metadata caches with bounded freshness.

These contain generic package/command metadata, not shell history.

### Dynamic runtime cache — `src/runtime_cache.rs`

Short-lived dynamic provider data is stored only under `$XDG_RUNTIME_DIR/termsense` when available.

Examples:

- Git refs;
- Docker container names;
- SSH hosts;
- systemd units.

Runtime-cache keys are validated to prevent path traversal.

## Bash adapter — `shell/termsense.bash`

The Bash adapter is responsible for presentation and Readline bindings.

It:

- reads allowlisted config keys;
- exports shell-native builtins/aliases/functions as local context;
- calls the Rust engine;
- draws suggestion rows and ghost text;
- tracks the selected row;
- replaces only the candidate's active range;
- records privacy-safe accepted usage keys;
- restores previous Readline bindings when TermSense dismisses its menu.

Current controls:

- Up/Down — selection;
- Tab — accept;
- Enter — accept while the suggestion menu is active;
- Right Arrow — accept ghost suggestion;
- Esc — dismiss;
- Ctrl+Space — explicit command browser.

When there is no active suggestion UI, Bash keeps normal execution behavior.

## Installation boundary

`scripts/install.sh` builds/installs the binary and manages one idempotent block in the user's Bash rc file.

Existing TermSense config is never overwritten.

The Debian package deliberately does not mutate a specific user's dotfiles while installing as root.

## Release boundary

Normal pushes and pull requests rely on local verification.

A single tag-only workflow under `.github/workflows/release.yml` runs for version tags, executes the full release-readiness gate, builds artifacts, writes checksums, and publishes the GitHub Release.

## Adding architecture

Prefer extending an existing layer over bypassing it:

- shell syntax/context → `shell_parse.rs`;
- suggestion semantics → `providers.rs`;
- persistent generic ranking → `usage.rs`;
- ephemeral provider caching → `runtime_cache.rs`;
- renderer/key behavior → `shell/termsense.bash`;
- packaging/install → `scripts/`.

See [PROVIDER_GUIDE.md](PROVIDER_GUIDE.md) for provider-specific guidance.
