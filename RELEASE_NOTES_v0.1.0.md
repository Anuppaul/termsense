# TermSense v0.1.0

TermSense brings IDE-style command discovery to interactive Bash on Linux without replacing the shell or executing suggestions automatically.

## Highlights

- Live command suggestions while typing
- Up to 20 visible suggestion rows
- One-line descriptions beside every suggestion
- Inline ghost text for the selected candidate
- Ctrl+Space command browser
- Up/Down navigation
- Tab or Right Arrow to accept
- Esc to dismiss
- Git, Docker, systemd, journalctl, APT, SSH, Cargo, npm/pnpm/yarn/bun, Make and filesystem-aware suggestions
- Dynamic local Git refs, Docker containers, systemd units and SSH hosts
- Shell-aware parsing for quotes, escapes, pipelines, separators, redirections, command substitutions and heredocs
- Local-first operation with no telemetry and no network requirement for core completion
- Privacy-safe adaptive ranking without storing raw shell history
- Native source installer/uninstaller
- Debian package builder for amd64 and arm64

## Verification

The v0.1.0 release-readiness gate passed locally on Linux with:

- 66/66 unit tests passing
- clean optimized release build
- Bash syntax checks
- generated Bash integration syntax checks
- binary smoke tests
- install/uninstall lifecycle checks
- Debian package generation
- interactive Bash UX validation

## Install from source

```bash
git clone https://github.com/Anuppaul/termsense.git
cd termsense
bash scripts/install.sh
exec bash
```

## Controls

| Key | Action |
| --- | --- |
| Type | Refresh live suggestions |
| Up / Down | Select a suggestion |
| Tab | Accept selected suggestion |
| Right Arrow | Accept ghost suggestion |
| Esc | Dismiss suggestions |
| Ctrl+Space | Browse commands |
| Enter | Accept selected suggestion while the menu is visible; otherwise execute normally in Bash |

See the README for full behavior, configuration, privacy details, and advanced examples.
