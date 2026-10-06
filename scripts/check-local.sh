#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

if [[ "$(uname -s)" != "Linux" ]]; then
  printf 'TermSense checks require Linux.\n' >&2
  exit 1
fi

if (( BASH_VERSINFO[0] < 5 )); then
  printf 'TermSense checks require Bash 5.0 or newer.\n' >&2
  exit 1
fi

command -v cargo >/dev/null 2>&1 || {
  printf 'cargo is required for local verification.\n' >&2
  exit 1
}

printf '==> rustfmt\n'
cargo fmt --check

printf '==> Rust tests\n'
cargo test

printf '==> Bash syntax\n'
bash -n shell/termsense.bash
bash -n scripts/install.sh
bash -n scripts/uninstall.sh
[[ ! -f scripts/package-deb.sh ]] || bash -n scripts/package-deb.sh
[[ ! -f scripts/release-readiness.sh ]] || bash -n scripts/release-readiness.sh

printf '==> Verify tag-only GitHub release workflow\n'
[[ -s .github/workflows/release.yml ]] || {
  printf 'missing .github/workflows/release.yml\n' >&2
  exit 1
}

unexpected_workflow="$(
  find .github/workflows -maxdepth 1 -type f ! -name 'release.yml' -print -quit 2>/dev/null || true
)"
if [[ -n "$unexpected_workflow" ]]; then
  printf 'unexpected workflow file found: %s\n' "$unexpected_workflow" >&2
  exit 1
fi

printf 'TermSense local checks: PASS\n'
