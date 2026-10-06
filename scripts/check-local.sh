#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

if [[ "$(uname -s)" != "Linux" ]]; then
  printf 'TermSense checks require Linux.\n' >&2
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

printf '==> Verify no GitHub Actions workflows are present\n'
if [[ -d .github/workflows ]] && find .github/workflows -type f -print -quit | grep -q .; then
  printf 'unexpected workflow file found under .github/workflows\n' >&2
  exit 1
fi

printf 'TermSense local checks: PASS\n'
