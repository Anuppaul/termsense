# TermSense — Product & Engineering Concept

## 1. Product definition

**TermSense** is a Linux-only terminal intelligence package that provides local, context-aware command discovery and completion directly while a user types in an interactive shell.

It is not a chatbot and it is not a replacement terminal emulator.

TermSense sits between the user's shell line editor and a local suggestion engine so that normal terminal work gains IDE-style discovery:

- inline ghost-text completion;
- a ranked suggestion list that appears as the user types;
- `Ctrl+Space` to open the complete command browser, similar to VS Code IntelliSense;
- context-aware arguments, paths, services, branches, containers, packages and history;
- local-first indexing and ranking;
- optional richer intelligence later without making AI a runtime dependency.

Initial shell target: **Bash on Linux**. The engine is designed so Zsh and Fish adapters can be added without changing the core.

## 2. Core interaction contract

### 2.1 Automatic suggestions while typing

When the command line is non-empty, TermSense continuously evaluates the text around the cursor and presents a compact ranked list.

Example:

```text
$ systemctl res
  restart
  reset-failed
  rescue
```

For the highest-ranked completion, the untyped suffix may also appear as ghost text.

```text
$ systemctl res[tart nginx.service]
```

The bracketed section above represents dim ghost text, not characters that have already been inserted.

### 2.2 Ctrl+Space — command browser / IntelliSense

`Ctrl+Space` is the explicit discovery key.

At an empty command position it opens all known executable commands from the user's current Linux environment.

```text
$ <Ctrl+Space>

  apt
  awk
  cargo
  curl
  docker
  find
  git
  grep
  journalctl
  ...
```

If text already exists, `Ctrl+Space` opens the complete candidate set for that syntactic position.

Examples:

- after `systemctl restart ` -> services;
- after `git checkout ` -> branches and relevant refs;
- after `docker logs ` -> containers;
- after `cd ` -> directories;
- after `apt install ` -> packages when the package source is available.

The list is searchable simply by continuing to type.

### 2.3 Keyboard behavior

Baseline UX:

| Key | Behavior |
| --- | --- |
| typing | refresh ranked suggestions |
| Ctrl+Space | open/expand IntelliSense candidate browser |
| Up / Down | move selection |
| Tab | accept selected candidate |
| Right Arrow | accept ghost suffix when applicable |
| Enter | execute the actual shell line normally |
| Esc | dismiss suggestion UI without altering the command |
| Ctrl+C | retain normal shell cancellation semantics |

TermSense must never execute a suggestion merely because it was selected.

## 3. Product principles

### Linux only

TermSense intentionally targets Linux terminals and Linux system semantics. No Windows or macOS compatibility layer is required.

### Local first

Core completion must work without network access. The user's command line and shell history remain on the machine.

### Fast path is deterministic

Normal keystroke suggestions must not depend on an LLM, web request, or slow subprocess.

### Discover the machine, do not ship a giant stale command catalog

TermSense learns from what is actually installed and available on the current machine.

### Shell-native execution semantics

TermSense proposes and inserts text. Bash remains responsible for parsing and executing the final command.

### Safe by construction

A suggestion is data until the user explicitly executes it with the shell. Indexing must avoid executing arbitrary discovered commands simply to learn about them.

## 4. Knowledge sources

The engine uses layered providers.

### Tier 0 — always-fast local sources

- `PATH` executables;
- aliases/functions exported by the shell adapter;
- filesystem entries relevant to the cursor;
- cached command metadata;
- local usage/history statistics.

### Tier 1 — structured local context

- Git repositories, branches and refs;
- systemd units;
- running/stopped containers when Docker/compatible tools are present;
- project markers such as `Cargo.toml`, `package.json`, `pyproject.toml`, `go.mod`, `Makefile`;
- shell completion metadata where safely parseable.

### Tier 2 — background enrichment

- man-page metadata;
- help/completion descriptions gathered through explicitly safe adapters;
- package-manager metadata;
- richer command schemas.

Tier 2 enrichment must never block keystroke latency.

## 5. Candidate model

Every candidate has a structured form rather than being a raw string.

```text
Candidate
├─ insert_text
├─ display_text
├─ kind
├─ description
├─ source
├─ score
├─ replacement_range
└─ optional metadata
```

Candidate kinds include:

- command;
- subcommand;
- option;
- path;
- service;
- package;
- git ref;
- container;
- project task;
- history command;
- argument value.

## 6. Ranking

Initial ranking can combine:

```text
score =
    prefix_match
  + exact_context_match
  + source_priority
  + recency
  + frequency
  + cwd_relevance
  + project_relevance
```

Ranking must be deterministic for identical local state.

Later versions may add typo tolerance and learned local ranking, but exact/prefix behavior remains predictable.

## 7. Architecture

```text
                    Linux interactive shell
                            │
                            ▼
                    Shell adapter (Bash)
                            │
          current buffer + cursor + shell context
                            │
                            ▼
                 termsense suggestion engine
                    ├─ parser/context
                    ├─ provider registry
                    ├─ candidate merger
                    ├─ ranker
                    └─ local cache/index
                            │
                            ▼
                    shell UI renderer
                  ├─ ghost suggestion
                  └─ candidate menu
```

### Core language

The core implementation is **Rust** for:

- low startup and query latency;
- a single deployable native binary;
- safe concurrency;
- predictable memory use;
- straightforward Linux packaging.

### Shell adapters

The core exposes stable machine-readable commands so adapters stay thin.

Initial internal CLI surface:

```text
termsense suggest
termsense list-commands
termsense index
termsense init bash
termsense doctor
termsense status
```

Shell-specific rendering/keybindings live under `shell/`.

## 8. Bash integration strategy

Bash is the first supported shell because it is the default interactive shell on many Ubuntu/Linux installations.

The Bash adapter owns:

- reading `READLINE_LINE` and `READLINE_POINT`;
- `Ctrl+Space` binding;
- candidate insertion;
- menu dismissal/navigation;
- forwarding shell context to the Rust engine;
- rendering without taking over command execution.

The integration must preserve normal Readline behavior and avoid stealing standard shortcuts except the documented TermSense bindings.

Automatic per-keystroke UI will be implemented incrementally: the engine API is built first, then the Bash renderer/keybinding layer adds live refresh without coupling ranking logic to Readline.

## 9. Indexing strategy

### Command index

Enumerate executable files reachable through `PATH`.

Rules:

- deduplicate by command name;
- preserve source path for diagnostics;
- skip unreadable/broken entries safely;
- never execute the binary during base indexing;
- refresh when PATH fingerprint changes or on explicit `termsense index`.

### History

History can improve ranking but must be optional and local.

TermSense should store minimal derived usage data rather than copying the user's entire shell history into a new database where possible.

### Cache

Start with a compact on-disk cache under XDG directories.

Expected locations:

```text
$XDG_CACHE_HOME/termsense/
$XDG_CONFIG_HOME/termsense/
$XDG_STATE_HOME/termsense/
```

with standard `~/.cache`, `~/.config`, and `~/.local/state` fallbacks.

SQLite may be introduced when provider/state complexity justifies it; the first vertical slice does not require a database.

## 10. Performance targets

Interactive latency is a product requirement.

Initial targets on a normal Linux workstation:

- cached prefix query: p50 < 5 ms;
- cached prefix query: p95 < 15 ms;
- command index load: < 50 ms after warm filesystem cache;
- no network on keystroke path;
- no arbitrary `--help` process spawning on keystroke path;
- expensive providers must be cached or backgrounded.

The renderer should debounce expensive context refreshes while keeping prefix filtering immediate.

## 11. Privacy and security

- no telemetry by default;
- no command/history upload;
- no network requirement for core use;
- no automatic command execution;
- no `eval` of generated suggestions;
- base PATH indexing reads metadata only;
- provider subprocesses, when later added, use explicit allowlisted adapters and timeouts;
- secrets typed on command lines must never be persisted as suggestion training data by default;
- sensitive command patterns should be eligible for history exclusion.

## 12. MVP roadmap

### Slice 1 — foundation

- Rust binary;
- Linux guard;
- PATH command discovery;
- deterministic prefix suggestions;
- JSON/plain output;
- `termsense list-commands`;
- `termsense suggest <buffer>`;
- Bash init script;
- `Ctrl+Space` candidate menu;
- local unit tests.

### Slice 2 — live terminal UX

- suggestion list refresh while typing;
- ghost suffix rendering;
- Up/Down selection;
- Tab / Right Arrow acceptance;
- Esc dismissal;
- robust redraw around resize, prompt changes and multiline input.

### Slice 3 — context providers

- filesystem/path completion;
- shell aliases/functions;
- Git refs;
- systemd units;
- Docker containers;
- project command providers.

### Slice 4 — enrichment and packaging

- cached completion metadata;
- man/help enrichment;
- DEB packaging;
- install/uninstall lifecycle;
- `termsense doctor`;
- configuration file and keybinding customization.

### Later

- Zsh adapter;
- Fish adapter;
- typo/fuzzy ranking;
- optional natural-language-to-command provider;
- package repositories beyond the first supported package manager.

## 13. Non-goals for the first release

- replacing the terminal emulator;
- replacing Bash itself;
- executing commands autonomously;
- cloud accounts;
- LLM dependency;
- cross-platform compatibility;
- a hard-coded database claiming to contain every Linux command.

## 14. Definition of success

A new Linux user can install TermSense, enable it for Bash, open a terminal and immediately:

1. press `Ctrl+Space` on an empty prompt to browse installed commands;
2. type a prefix and see useful candidates automatically;
3. accept a suggestion without retyping it;
4. receive context-aware candidates for common Linux workflows;
5. keep normal Bash execution, history and key behavior;
6. use the core experience fully offline.

That interaction is the product contract. All implementation decisions should protect it.
