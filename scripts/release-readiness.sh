#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

fail() {
  printf 'release readiness: FAIL: %s\n' "$*" >&2
  exit 1
}

require() {
  command -v "$1" >/dev/null 2>&1 || fail "missing required command: $1"
}

[[ "$(uname -s)" == "Linux" ]] || fail "TermSense release checks require Linux"

for cmd in cargo rustc bash awk grep git dpkg-deb; do
  require "$cmd"
done

read_cargo_field() {
  local field="$1"
  awk -v field="$field" '
    /^\[package\]$/ { in_package = 1; next }
    /^\[/ && $0 != "[package]" { in_package = 0 }
    in_package && $1 == field {
      gsub(/"/, "", $3)
      print $3
      exit
    }
  ' Cargo.toml
}

name="$(read_cargo_field name)"
version="$(read_cargo_field version)"

[[ "$name" == "termsense" ]] || fail "Cargo package name is not termsense"
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+([+-][0-9A-Za-z.-]+)?$ ]]   || fail "Cargo version is not release-shaped: $version"

[[ -s LICENSE ]] || fail "LICENSE is missing or empty"
grep -q '^MIT License$' LICENSE || fail "LICENSE is not the expected MIT license"
grep -q 'repository = "https://github.com/Anuppaul/termsense"' Cargo.toml   || fail "Cargo repository identity mismatch"

if [[ -d .github/workflows ]] && find .github/workflows -type f -print -quit | grep -q .; then
  fail "GitHub Actions workflows are present; project policy currently forbids CI workflows"
fi

printf '==> git diff check\n'
git diff --check

printf '==> rustfmt\n'
cargo fmt --check

printf '==> tests\n'
cargo test

printf '==> release build\n'
cargo build --release

printf '==> shell syntax\n'
bash -n shell/termsense.bash
bash -n scripts/check-local.sh
bash -n scripts/install.sh
bash -n scripts/uninstall.sh
bash -n scripts/package-deb.sh
bash -n scripts/release-readiness.sh

tmp="$(mktemp -d)"
trap 'rm -rf -- "$tmp"' EXIT

printf '==> generated Bash integration syntax\n'
./target/release/termsense init bash > "$tmp/termsense-init.bash"
bash -n "$tmp/termsense-init.bash"

printf '==> binary smoke\n'
./target/release/termsense status > "$tmp/status.txt"
grep -q '^platform: linux$' "$tmp/status.txt" || fail "status platform smoke failed"
grep -q '^network required: no$' "$tmp/status.txt" || fail "offline status smoke failed"

./target/release/termsense suggest d --limit 5 > "$tmp/suggest-command.txt"
[[ -s "$tmp/suggest-command.txt" ]] || fail "command suggestion smoke returned no candidates"

./target/release/termsense suggest "sudo git che" --limit 20 > "$tmp/suggest-context.txt"
grep -Fq $'checkout\t' "$tmp/suggest-context.txt"   || fail "sudo git contextual completion smoke failed"

./target/release/termsense suggest "git status && docker lo" --limit 20   > "$tmp/suggest-segment.txt"
grep -Fq $'logs\tgit status && docker logs\t' "$tmp/suggest-segment.txt"   || fail "active command segment smoke failed"

./target/release/termsense suggest 'echo $(git che' --limit 20   > "$tmp/suggest-substitution.txt"
grep -Fq $'checkout\techo $(git checkout\t' "$tmp/suggest-substitution.txt"   || fail "command substitution smoke failed"

printf '==> Debian package\n'
TERMSENSE_SKIP_BUILD=1 bash scripts/package-deb.sh --output "$tmp/dist" >/dev/null

deb="$tmp/dist/termsense_${version}_"
deb_file="$(find "$tmp/dist" -maxdepth 1 -type f -name "termsense_${version}_*.deb" -print -quit)"
[[ -n "$deb_file" ]] || fail "Debian package was not created"

[[ "$(dpkg-deb -f "$deb_file" Package)" == "termsense" ]]   || fail "Debian package name mismatch"
[[ "$(dpkg-deb -f "$deb_file" Version)" == "$version" ]]   || fail "Debian package version mismatch"

printf 'TermSense release readiness: PASS\n'
printf 'version: %s\n' "$version"
printf 'binary: %s\n' "$repo_root/target/release/termsense"
printf 'deb: %s\n' "$deb_file"
