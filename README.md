# TermSense

**Context-aware inline command suggestions for Linux terminals.**

TermSense is a Linux-only terminal intelligence package. It discovers the commands available on the current machine and provides IDE-style command discovery without replacing Bash or executing anything automatically.

> Status: early implementation / v0.1 foundation.

## Current slice

Implemented:

- Linux-only Rust CLI;
- safe executable discovery from `PATH`;
- deterministic prefix ranking;
- plain-text and JSON suggestion output;
- `termsense list-commands`;
- `termsense suggest`;
- `termsense status`;
- `termsense doctor`;
- Bash initialization;
- **Ctrl+Space** command browser and insertion;
- optional `fzf` UI with a dependency-free numbered fallback.

Next implementation slice:

- suggestions shown automatically as the user types;
- inline ghost suffix;
- keyboard navigation and non-blocking redraw;
- context providers for paths, Git, systemd, Docker and projects.

The complete product contract is in [docs/CONCEPT.md](docs/CONCEPT.md).

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

For development, either invoke:

```bash
./target/release/termsense status
```

or install into your Cargo binary directory:

```bash
cargo install --path .
```

## Enable Bash integration

For the current shell:

```bash
eval "$(termsense init bash)"
```

Then press:

```text
Ctrl+Space
```

at an empty prompt to browse commands, or type a prefix such as:

```text
sys
```

and press `Ctrl+Space` to filter the command browser.

To enable on every interactive Bash session, add this to `~/.bashrc`:

```bash
if command -v termsense >/dev/null 2>&1; then
  eval "$(termsense init bash)"
fi
```

### Optional: fzf

If `fzf` is present, TermSense uses it for a polished searchable Ctrl+Space browser. Without it, TermSense uses its built-in numbered fallback. The core package does not require `fzf`.

## Engine examples

List installed commands:

```bash
termsense list-commands
```

Prefix suggestions:

```bash
termsense suggest sys
```

Machine-readable output:

```bash
termsense suggest git --json
```

Diagnostics:

```bash
termsense doctor
termsense status
```

## Safety

TermSense does not execute selected commands. It only inserts text into the shell line. Bash remains responsible for execution after the user presses Enter.

Base indexing reads executable metadata from directories already present in `PATH`; it does not run discovered binaries.

## CI policy

No GitHub Actions workflow is included in the repository at this stage. Development checks are intended to be run locally while the initial terminal interaction is being built.

## License

MIT license is planned for the package; the license file will be added before the first distributable release.
