## What changed?

Describe the user-visible or developer-facing change.

## Why?

Link the issue this addresses, when applicable:

Closes #

## Verification

- [ ] `cargo fmt --check`
- [ ] `cargo test`
- [ ] `bash scripts/check-local.sh`
- [ ] `bash scripts/release-readiness.sh` when the environment supports the full release gate
- [ ] Interactive shell behavior tested when Bash/Readline UX changed

## TermSense invariants

- [ ] Suggestions still never auto-execute
- [ ] No telemetry or command-history upload was introduced
- [ ] No user-specific or machine-specific dynamic data was hardcoded
- [ ] Dynamic identifiers are not persisted in adaptive ranking
- [ ] Existing user config is not overwritten
- [ ] User-visible behavior/docs were updated where needed

## Notes

Add screenshots, terminal examples, compatibility notes, or follow-up work here.
