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

[[ "$(uname -s)" == "Linux" ]] || fail "TermSense release checks require Linux"
(( BASH_VERSINFO[0] >= 5 )) || fail "TermSense release checks require Bash 5.0 or newer"

for cmd in cargo rustc bash awk grep git dpkg-deb find install mktemp; do
  require "$cmd"
done

name="$(read_cargo_field name)"
version="$(read_cargo_field version)"

[[ "$name" == "termsense" ]] || fail "Cargo package name is not termsense"
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+([+-][0-9A-Za-z.-]+)?$ ]] \
  || fail "Cargo version is not release-shaped: $version"

[[ -s LICENSE ]] || fail "LICENSE is missing or empty"
[[ -s CHANGELOG.md ]] || fail "CHANGELOG.md is missing or empty"
[[ -s config/default.conf ]] || fail "default config is missing or empty"

grep -q '^MIT License$' LICENSE || fail "LICENSE is not the expected MIT license"
grep -q '^auto_suggest=1$' config/default.conf || fail "default config missing auto_suggest"
grep -q '^max_visible=20
grep -q '^ctrl_space=1$' config/default.conf || fail "default config missing ctrl_space"
grep -q 'repository = "https://github.com/Anuppaul/termsense"' Cargo.toml \
  || fail "Cargo repository identity mismatch"

if [[ -d .github/workflows ]] && find .github/workflows -type f -print -quit | grep -q .; then
  fail "GitHub Actions workflows are present; project policy currently forbids CI workflows"
fi

printf '==> source integrity\n'
grep -q "^grep -q '\\^max_visible=20\\[[ "$(grep -c '^_termsense_query() {' shell/termsense.bash)" -eq 1 ]] \
  || fail "Bash adapter must contain exactly one _termsense_query function"
[[ "$(grep -c '^_termsense_ctrl_space() {' shell/termsense.bash)" -eq 1 ]] \
  || fail "Bash adapter must contain exactly one _termsense_ctrl_space function"
[[ "$(grep -c '^_termsense_draw_overlay() {' shell/termsense.bash)" -eq 1 ]] \
  || fail "Bash adapter must contain exactly one _termsense_draw_overlay function"
[[ "$(grep -c '^pub(crate) fn active_context' src/shell_parse.rs)" -eq 1 ]] \
  || fail "shell parser must contain exactly one active_context function"
[[ "$(grep -c '^fn lex_range' src/shell_parse.rs)" -eq 1 ]] \
  || fail "shell parser must contain exactly one lex_range function"
[[ "$(grep -c '^fn inside_open_heredoc' src/shell_parse.rs)" -eq 1 ]] \
  || fail "shell parser must contain exactly one heredoc detector"

printf '==> git diff check\n'
git diff --check

printf '==> rustfmt\n'
cargo fmt --check

printf '==> tests\n'
cargo test

printf '==> release build\n'
cargo build --release

[[ -s Cargo.lock ]] || fail "Cargo.lock was not generated"
git ls-files --error-unmatch Cargo.lock >/dev/null 2>&1 \
  || fail "Cargo.lock exists but is not committed; commit the generated lockfile before release"
git diff --quiet -- Cargo.lock \
  || fail "Cargo.lock changed during build; commit the updated lockfile before release"

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
[[ "$(grep -c '^_termsense_query() {' "$tmp/termsense-init.bash")" -eq 1 ]] \
  || fail "generated Bash integration contains duplicate query functions"

printf '==> binary smoke\n'
./target/release/termsense status > "$tmp/status.txt"
grep -q '^platform: linux$' "$tmp/status.txt" || fail "status platform smoke failed"
grep -q '^network required: no$' "$tmp/status.txt" || fail "offline status smoke failed"
grep -q '^config file: ' "$tmp/status.txt" || fail "config status smoke failed"

./target/release/termsense suggest d --limit 5 > "$tmp/suggest-command.txt"
[[ -s "$tmp/suggest-command.txt" ]] || fail "command suggestion smoke returned no candidates"

./target/release/termsense suggest "" --limit 0 > "$tmp/suggest-all.txt"
[[ -s "$tmp/suggest-all.txt" ]] || fail "unbounded command discovery smoke returned no candidates"

./target/release/termsense suggest "sudo git che" --limit 20 > "$tmp/suggest-context.txt"
grep -Fq "sudo git checkout" "$tmp/suggest-context.txt" \
  || fail "sudo git contextual completion smoke failed"
awk -F '\t' '$2 == "sudo git checkout" && $9 == "Switch branches or restore files" { found = 1 } END { exit !found }' \
  "$tmp/suggest-context.txt" \
  || fail "suggestion description protocol smoke failed"

./target/release/termsense suggest "git status && docker lo" --limit 20 > "$tmp/suggest-segment.txt"
grep -Fq "git status && docker logs" "$tmp/suggest-segment.txt" \
  || fail "active command segment smoke failed"

./target/release/termsense suggest 'echo $(git che' --limit 20 > "$tmp/suggest-substitution.txt"
grep -Fq 'echo $(git checkout' "$tmp/suggest-substitution.txt" \
  || fail "command substitution smoke failed"

./target/release/termsense suggest 'diff <(git che' --limit 20 > "$tmp/suggest-process-sub.txt"
grep -Fq 'diff <(git checkout' "$tmp/suggest-process-sub.txt" \
  || fail "process substitution smoke failed"

./target/release/termsense suggest '( git che' --limit 20 > "$tmp/suggest-group.txt"
grep -Fq '( git checkout' "$tmp/suggest-group.txt" \
  || fail "command group smoke failed"

heredoc_buffer="$(printf 'cat <<EOF\nhello wor')"
./target/release/termsense suggest "$heredoc_buffer" --limit 20 > "$tmp/suggest-heredoc.txt"
[[ ! -s "$tmp/suggest-heredoc.txt" ]] || fail "heredoc body should suppress suggestions"

arithmetic_buffer='echo $((1 + 2'
./target/release/termsense suggest "$arithmetic_buffer" --limit 20 > "$tmp/suggest-arithmetic.txt"
[[ ! -s "$tmp/suggest-arithmetic.txt" ]] || fail "open arithmetic expansion should suppress suggestions"

printf '==> install/uninstall lifecycle\n'
mkdir -p "$tmp/home"

HOME="$tmp/home" \
XDG_CONFIG_HOME="$tmp/home/.config" \
XDG_CACHE_HOME="$tmp/home/.cache" \
XDG_STATE_HOME="$tmp/home/.local/state" \
TERMSENSE_SKIP_BUILD=1 \
bash scripts/install.sh \
  --prefix "$tmp/prefix" \
  --bashrc "$tmp/bashrc" >/dev/null

[[ -x "$tmp/prefix/bin/termsense" ]] || fail "installer did not install binary"
[[ -f "$tmp/home/.config/termsense/config.conf" ]] || fail "installer did not create config"
grep -q '^# >>> termsense >>>$' "$tmp/bashrc" || fail "installer did not add managed Bash block"
grep -q '^TERMSENSE_BIN=' "$tmp/bashrc" || fail "installer did not pin installed binary"

installed_lookup="$(
  HOME="$tmp/home" \
  XDG_CONFIG_HOME="$tmp/home/.config" \
  PATH="/usr/bin:/bin" \
  bash --noprofile --norc -ic "source '$tmp/bashrc'; _termsense_binary" 2>/dev/null
)"
[[ "$installed_lookup" == "$tmp/prefix/bin/termsense" ]] \
  || fail "user-local binary lookup fails when prefix/bin is outside PATH"

HOME="$tmp/home" \
XDG_CONFIG_HOME="$tmp/home/.config" \
XDG_CACHE_HOME="$tmp/home/.cache" \
XDG_STATE_HOME="$tmp/home/.local/state" \
bash scripts/uninstall.sh \
  --prefix "$tmp/prefix" \
  --bashrc "$tmp/bashrc" \
  --purge >/dev/null

[[ ! -e "$tmp/prefix/bin/termsense" ]] || fail "uninstaller did not remove binary"
! grep -q '^# >>> termsense >>>$' "$tmp/bashrc" || fail "uninstaller left managed Bash block"
[[ ! -d "$tmp/home/.config/termsense" ]] || fail "purge left config directory"

printf '==> Debian package\n'
TERMSENSE_SKIP_BUILD=1 bash scripts/package-deb.sh --output "$tmp/dist" >/dev/null

deb_file="$(find "$tmp/dist" -maxdepth 1 -type f -name "termsense_${version}_*.deb" -print -quit)"
[[ -n "$deb_file" ]] || fail "Debian package was not created"

[[ "$(dpkg-deb -f "$deb_file" Package)" == "termsense" ]] \
  || fail "Debian package name mismatch"
[[ "$(dpkg-deb -f "$deb_file" Version)" == "$version" ]] \
  || fail "Debian package version mismatch"
dpkg-deb -c "$deb_file" | grep -q './usr/share/termsense/default.conf' \
  || fail "Debian package is missing reference default config"
dpkg-deb -c "$deb_file" | grep -q './usr/share/doc/termsense/CHANGELOG.md' \
  || fail "Debian package is missing changelog"

printf 'TermSense release readiness: PASS\n'
printf 'version: %s\n' "$version"
printf 'binary: %s\n' "$repo_root/target/release/termsense"
printf 'deb: %s\n' "$deb_file"
 config/default.conf || fail "default config missing max_visible"
grep -q '^ghost=1$' config/default.conf || fail "default config missing ghost"
grep -q '^ctrl_space=1$' config/default.conf || fail "default config missing ctrl_space"
grep -q 'repository = "https://github.com/Anuppaul/termsense"' Cargo.toml \
  || fail "Cargo repository identity mismatch"

if [[ -d .github/workflows ]] && find .github/workflows -type f -print -quit | grep -q .; then
  fail "GitHub Actions workflows are present; project policy currently forbids CI workflows"
fi

printf '==> source integrity\n'
[[ "$(grep -c '^_termsense_query() {' shell/termsense.bash)" -eq 1 ]] \
  || fail "Bash adapter must contain exactly one _termsense_query function"
[[ "$(grep -c '^_termsense_ctrl_space() {' shell/termsense.bash)" -eq 1 ]] \
  || fail "Bash adapter must contain exactly one _termsense_ctrl_space function"
[[ "$(grep -c '^_termsense_draw_overlay() {' shell/termsense.bash)" -eq 1 ]] \
  || fail "Bash adapter must contain exactly one _termsense_draw_overlay function"
[[ "$(grep -c '^pub(crate) fn active_context' src/shell_parse.rs)" -eq 1 ]] \
  || fail "shell parser must contain exactly one active_context function"
[[ "$(grep -c '^fn lex_range' src/shell_parse.rs)" -eq 1 ]] \
  || fail "shell parser must contain exactly one lex_range function"
[[ "$(grep -c '^fn inside_open_heredoc' src/shell_parse.rs)" -eq 1 ]] \
  || fail "shell parser must contain exactly one heredoc detector"

printf '==> git diff check\n'
git diff --check

printf '==> rustfmt\n'
cargo fmt --check

printf '==> tests\n'
cargo test

printf '==> release build\n'
cargo build --release

[[ -s Cargo.lock ]] || fail "Cargo.lock was not generated"
git ls-files --error-unmatch Cargo.lock >/dev/null 2>&1 \
  || fail "Cargo.lock exists but is not committed; commit the generated lockfile before release"
git diff --quiet -- Cargo.lock \
  || fail "Cargo.lock changed during build; commit the updated lockfile before release"

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
[[ "$(grep -c '^_termsense_query() {' "$tmp/termsense-init.bash")" -eq 1 ]] \
  || fail "generated Bash integration contains duplicate query functions"

printf '==> binary smoke\n'
./target/release/termsense status > "$tmp/status.txt"
grep -q '^platform: linux$' "$tmp/status.txt" || fail "status platform smoke failed"
grep -q '^network required: no$' "$tmp/status.txt" || fail "offline status smoke failed"
grep -q '^config file: ' "$tmp/status.txt" || fail "config status smoke failed"

./target/release/termsense suggest d --limit 5 > "$tmp/suggest-command.txt"
[[ -s "$tmp/suggest-command.txt" ]] || fail "command suggestion smoke returned no candidates"

./target/release/termsense suggest "" --limit 0 > "$tmp/suggest-all.txt"
[[ -s "$tmp/suggest-all.txt" ]] || fail "unbounded command discovery smoke returned no candidates"

./target/release/termsense suggest "sudo git che" --limit 20 > "$tmp/suggest-context.txt"
grep -Fq "sudo git checkout" "$tmp/suggest-context.txt" \
  || fail "sudo git contextual completion smoke failed"

./target/release/termsense suggest "git status && docker lo" --limit 20 > "$tmp/suggest-segment.txt"
grep -Fq "git status && docker logs" "$tmp/suggest-segment.txt" \
  || fail "active command segment smoke failed"

./target/release/termsense suggest 'echo $(git che' --limit 20 > "$tmp/suggest-substitution.txt"
grep -Fq 'echo $(git checkout' "$tmp/suggest-substitution.txt" \
  || fail "command substitution smoke failed"

./target/release/termsense suggest 'diff <(git che' --limit 20 > "$tmp/suggest-process-sub.txt"
grep -Fq 'diff <(git checkout' "$tmp/suggest-process-sub.txt" \
  || fail "process substitution smoke failed"

./target/release/termsense suggest '( git che' --limit 20 > "$tmp/suggest-group.txt"
grep -Fq '( git checkout' "$tmp/suggest-group.txt" \
  || fail "command group smoke failed"

heredoc_buffer="$(printf 'cat <<EOF\nhello wor')"
./target/release/termsense suggest "$heredoc_buffer" --limit 20 > "$tmp/suggest-heredoc.txt"
[[ ! -s "$tmp/suggest-heredoc.txt" ]] || fail "heredoc body should suppress suggestions"

arithmetic_buffer='echo $((1 + 2'
./target/release/termsense suggest "$arithmetic_buffer" --limit 20 > "$tmp/suggest-arithmetic.txt"
[[ ! -s "$tmp/suggest-arithmetic.txt" ]] || fail "open arithmetic expansion should suppress suggestions"

printf '==> install/uninstall lifecycle\n'
mkdir -p "$tmp/home"

HOME="$tmp/home" \
XDG_CONFIG_HOME="$tmp/home/.config" \
XDG_CACHE_HOME="$tmp/home/.cache" \
XDG_STATE_HOME="$tmp/home/.local/state" \
TERMSENSE_SKIP_BUILD=1 \
bash scripts/install.sh \
  --prefix "$tmp/prefix" \
  --bashrc "$tmp/bashrc" >/dev/null

[[ -x "$tmp/prefix/bin/termsense" ]] || fail "installer did not install binary"
[[ -f "$tmp/home/.config/termsense/config.conf" ]] || fail "installer did not create config"
grep -q '^# >>> termsense >>>$' "$tmp/bashrc" || fail "installer did not add managed Bash block"
grep -q '^TERMSENSE_BIN=' "$tmp/bashrc" || fail "installer did not pin installed binary"

installed_lookup="$(
  HOME="$tmp/home" \
  XDG_CONFIG_HOME="$tmp/home/.config" \
  PATH="/usr/bin:/bin" \
  bash --noprofile --norc -ic "source '$tmp/bashrc'; _termsense_binary" 2>/dev/null
)"
[[ "$installed_lookup" == "$tmp/prefix/bin/termsense" ]] \
  || fail "user-local binary lookup fails when prefix/bin is outside PATH"

HOME="$tmp/home" \
XDG_CONFIG_HOME="$tmp/home/.config" \
XDG_CACHE_HOME="$tmp/home/.cache" \
XDG_STATE_HOME="$tmp/home/.local/state" \
bash scripts/uninstall.sh \
  --prefix "$tmp/prefix" \
  --bashrc "$tmp/bashrc" \
  --purge >/dev/null

[[ ! -e "$tmp/prefix/bin/termsense" ]] || fail "uninstaller did not remove binary"
! grep -q '^# >>> termsense >>>$' "$tmp/bashrc" || fail "uninstaller left managed Bash block"
[[ ! -d "$tmp/home/.config/termsense" ]] || fail "purge left config directory"

printf '==> Debian package\n'
TERMSENSE_SKIP_BUILD=1 bash scripts/package-deb.sh --output "$tmp/dist" >/dev/null

deb_file="$(find "$tmp/dist" -maxdepth 1 -type f -name "termsense_${version}_*.deb" -print -quit)"
[[ -n "$deb_file" ]] || fail "Debian package was not created"

[[ "$(dpkg-deb -f "$deb_file" Package)" == "termsense" ]] \
  || fail "Debian package name mismatch"
[[ "$(dpkg-deb -f "$deb_file" Version)" == "$version" ]] \
  || fail "Debian package version mismatch"
dpkg-deb -c "$deb_file" | grep -q './usr/share/termsense/default.conf' \
  || fail "Debian package is missing reference default config"
dpkg-deb -c "$deb_file" | grep -q './usr/share/doc/termsense/CHANGELOG.md' \
  || fail "Debian package is missing changelog"

printf 'TermSense release readiness: PASS\n'
printf 'version: %s\n' "$version"
printf 'binary: %s\n' "$repo_root/target/release/termsense"
printf 'deb: %s\n' "$deb_file"
 config/default.conf || fail "default config missing max_visible"
grep -q '^ghost=1
grep -q '^ctrl_space=1$' config/default.conf || fail "default config missing ctrl_space"
grep -q 'repository = "https://github.com/Anuppaul/termsense"' Cargo.toml \
  || fail "Cargo repository identity mismatch"

if [[ -d .github/workflows ]] && find .github/workflows -type f -print -quit | grep -q .; then
  fail "GitHub Actions workflows are present; project policy currently forbids CI workflows"
fi

printf '==> source integrity\n'
[[ "$(grep -c '^_termsense_query() {' shell/termsense.bash)" -eq 1 ]] \
  || fail "Bash adapter must contain exactly one _termsense_query function"
[[ "$(grep -c '^_termsense_ctrl_space() {' shell/termsense.bash)" -eq 1 ]] \
  || fail "Bash adapter must contain exactly one _termsense_ctrl_space function"
[[ "$(grep -c '^_termsense_draw_overlay() {' shell/termsense.bash)" -eq 1 ]] \
  || fail "Bash adapter must contain exactly one _termsense_draw_overlay function"
[[ "$(grep -c '^pub(crate) fn active_context' src/shell_parse.rs)" -eq 1 ]] \
  || fail "shell parser must contain exactly one active_context function"
[[ "$(grep -c '^fn lex_range' src/shell_parse.rs)" -eq 1 ]] \
  || fail "shell parser must contain exactly one lex_range function"
[[ "$(grep -c '^fn inside_open_heredoc' src/shell_parse.rs)" -eq 1 ]] \
  || fail "shell parser must contain exactly one heredoc detector"

printf '==> git diff check\n'
git diff --check

printf '==> rustfmt\n'
cargo fmt --check

printf '==> tests\n'
cargo test

printf '==> release build\n'
cargo build --release

[[ -s Cargo.lock ]] || fail "Cargo.lock was not generated"
git ls-files --error-unmatch Cargo.lock >/dev/null 2>&1 \
  || fail "Cargo.lock exists but is not committed; commit the generated lockfile before release"
git diff --quiet -- Cargo.lock \
  || fail "Cargo.lock changed during build; commit the updated lockfile before release"

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
[[ "$(grep -c '^_termsense_query() {' "$tmp/termsense-init.bash")" -eq 1 ]] \
  || fail "generated Bash integration contains duplicate query functions"

printf '==> binary smoke\n'
./target/release/termsense status > "$tmp/status.txt"
grep -q '^platform: linux$' "$tmp/status.txt" || fail "status platform smoke failed"
grep -q '^network required: no$' "$tmp/status.txt" || fail "offline status smoke failed"
grep -q '^config file: ' "$tmp/status.txt" || fail "config status smoke failed"

./target/release/termsense suggest d --limit 5 > "$tmp/suggest-command.txt"
[[ -s "$tmp/suggest-command.txt" ]] || fail "command suggestion smoke returned no candidates"

./target/release/termsense suggest "" --limit 0 > "$tmp/suggest-all.txt"
[[ -s "$tmp/suggest-all.txt" ]] || fail "unbounded command discovery smoke returned no candidates"

./target/release/termsense suggest "sudo git che" --limit 20 > "$tmp/suggest-context.txt"
grep -Fq "sudo git checkout" "$tmp/suggest-context.txt" \
  || fail "sudo git contextual completion smoke failed"
awk -F '\t' '$2 == "sudo git checkout" && $9 == "Switch branches or restore files" { found = 1 } END { exit !found }' \
  "$tmp/suggest-context.txt" \
  || fail "suggestion description protocol smoke failed"

./target/release/termsense suggest "git status && docker lo" --limit 20 > "$tmp/suggest-segment.txt"
grep -Fq "git status && docker logs" "$tmp/suggest-segment.txt" \
  || fail "active command segment smoke failed"

./target/release/termsense suggest 'echo $(git che' --limit 20 > "$tmp/suggest-substitution.txt"
grep -Fq 'echo $(git checkout' "$tmp/suggest-substitution.txt" \
  || fail "command substitution smoke failed"

./target/release/termsense suggest 'diff <(git che' --limit 20 > "$tmp/suggest-process-sub.txt"
grep -Fq 'diff <(git checkout' "$tmp/suggest-process-sub.txt" \
  || fail "process substitution smoke failed"

./target/release/termsense suggest '( git che' --limit 20 > "$tmp/suggest-group.txt"
grep -Fq '( git checkout' "$tmp/suggest-group.txt" \
  || fail "command group smoke failed"

heredoc_buffer="$(printf 'cat <<EOF\nhello wor')"
./target/release/termsense suggest "$heredoc_buffer" --limit 20 > "$tmp/suggest-heredoc.txt"
[[ ! -s "$tmp/suggest-heredoc.txt" ]] || fail "heredoc body should suppress suggestions"

arithmetic_buffer='echo $((1 + 2'
./target/release/termsense suggest "$arithmetic_buffer" --limit 20 > "$tmp/suggest-arithmetic.txt"
[[ ! -s "$tmp/suggest-arithmetic.txt" ]] || fail "open arithmetic expansion should suppress suggestions"

printf '==> install/uninstall lifecycle\n'
mkdir -p "$tmp/home"

HOME="$tmp/home" \
XDG_CONFIG_HOME="$tmp/home/.config" \
XDG_CACHE_HOME="$tmp/home/.cache" \
XDG_STATE_HOME="$tmp/home/.local/state" \
TERMSENSE_SKIP_BUILD=1 \
bash scripts/install.sh \
  --prefix "$tmp/prefix" \
  --bashrc "$tmp/bashrc" >/dev/null

[[ -x "$tmp/prefix/bin/termsense" ]] || fail "installer did not install binary"
[[ -f "$tmp/home/.config/termsense/config.conf" ]] || fail "installer did not create config"
grep -q '^# >>> termsense >>>$' "$tmp/bashrc" || fail "installer did not add managed Bash block"
grep -q '^TERMSENSE_BIN=' "$tmp/bashrc" || fail "installer did not pin installed binary"

installed_lookup="$(
  HOME="$tmp/home" \
  XDG_CONFIG_HOME="$tmp/home/.config" \
  PATH="/usr/bin:/bin" \
  bash --noprofile --norc -ic "source '$tmp/bashrc'; _termsense_binary" 2>/dev/null
)"
[[ "$installed_lookup" == "$tmp/prefix/bin/termsense" ]] \
  || fail "user-local binary lookup fails when prefix/bin is outside PATH"

HOME="$tmp/home" \
XDG_CONFIG_HOME="$tmp/home/.config" \
XDG_CACHE_HOME="$tmp/home/.cache" \
XDG_STATE_HOME="$tmp/home/.local/state" \
bash scripts/uninstall.sh \
  --prefix "$tmp/prefix" \
  --bashrc "$tmp/bashrc" \
  --purge >/dev/null

[[ ! -e "$tmp/prefix/bin/termsense" ]] || fail "uninstaller did not remove binary"
! grep -q '^# >>> termsense >>>$' "$tmp/bashrc" || fail "uninstaller left managed Bash block"
[[ ! -d "$tmp/home/.config/termsense" ]] || fail "purge left config directory"

printf '==> Debian package\n'
TERMSENSE_SKIP_BUILD=1 bash scripts/package-deb.sh --output "$tmp/dist" >/dev/null

deb_file="$(find "$tmp/dist" -maxdepth 1 -type f -name "termsense_${version}_*.deb" -print -quit)"
[[ -n "$deb_file" ]] || fail "Debian package was not created"

[[ "$(dpkg-deb -f "$deb_file" Package)" == "termsense" ]] \
  || fail "Debian package name mismatch"
[[ "$(dpkg-deb -f "$deb_file" Version)" == "$version" ]] \
  || fail "Debian package version mismatch"
dpkg-deb -c "$deb_file" | grep -q './usr/share/termsense/default.conf' \
  || fail "Debian package is missing reference default config"
dpkg-deb -c "$deb_file" | grep -q './usr/share/doc/termsense/CHANGELOG.md' \
  || fail "Debian package is missing changelog"

printf 'TermSense release readiness: PASS\n'
printf 'version: %s\n' "$version"
printf 'binary: %s\n' "$repo_root/target/release/termsense"
printf 'deb: %s\n' "$deb_file"
 config/default.conf || fail "default config missing max_visible"
grep -q '^ghost=1$' config/default.conf || fail "default config missing ghost"
grep -q '^ctrl_space=1$' config/default.conf || fail "default config missing ctrl_space"
grep -q 'repository = "https://github.com/Anuppaul/termsense"' Cargo.toml \
  || fail "Cargo repository identity mismatch"

if [[ -d .github/workflows ]] && find .github/workflows -type f -print -quit | grep -q .; then
  fail "GitHub Actions workflows are present; project policy currently forbids CI workflows"
fi

printf '==> source integrity\n'
[[ "$(grep -c '^_termsense_query() {' shell/termsense.bash)" -eq 1 ]] \
  || fail "Bash adapter must contain exactly one _termsense_query function"
[[ "$(grep -c '^_termsense_ctrl_space() {' shell/termsense.bash)" -eq 1 ]] \
  || fail "Bash adapter must contain exactly one _termsense_ctrl_space function"
[[ "$(grep -c '^_termsense_draw_overlay() {' shell/termsense.bash)" -eq 1 ]] \
  || fail "Bash adapter must contain exactly one _termsense_draw_overlay function"
[[ "$(grep -c '^pub(crate) fn active_context' src/shell_parse.rs)" -eq 1 ]] \
  || fail "shell parser must contain exactly one active_context function"
[[ "$(grep -c '^fn lex_range' src/shell_parse.rs)" -eq 1 ]] \
  || fail "shell parser must contain exactly one lex_range function"
[[ "$(grep -c '^fn inside_open_heredoc' src/shell_parse.rs)" -eq 1 ]] \
  || fail "shell parser must contain exactly one heredoc detector"

printf '==> git diff check\n'
git diff --check

printf '==> rustfmt\n'
cargo fmt --check

printf '==> tests\n'
cargo test

printf '==> release build\n'
cargo build --release

[[ -s Cargo.lock ]] || fail "Cargo.lock was not generated"
git ls-files --error-unmatch Cargo.lock >/dev/null 2>&1 \
  || fail "Cargo.lock exists but is not committed; commit the generated lockfile before release"
git diff --quiet -- Cargo.lock \
  || fail "Cargo.lock changed during build; commit the updated lockfile before release"

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
[[ "$(grep -c '^_termsense_query() {' "$tmp/termsense-init.bash")" -eq 1 ]] \
  || fail "generated Bash integration contains duplicate query functions"

printf '==> binary smoke\n'
./target/release/termsense status > "$tmp/status.txt"
grep -q '^platform: linux$' "$tmp/status.txt" || fail "status platform smoke failed"
grep -q '^network required: no$' "$tmp/status.txt" || fail "offline status smoke failed"
grep -q '^config file: ' "$tmp/status.txt" || fail "config status smoke failed"

./target/release/termsense suggest d --limit 5 > "$tmp/suggest-command.txt"
[[ -s "$tmp/suggest-command.txt" ]] || fail "command suggestion smoke returned no candidates"

./target/release/termsense suggest "" --limit 0 > "$tmp/suggest-all.txt"
[[ -s "$tmp/suggest-all.txt" ]] || fail "unbounded command discovery smoke returned no candidates"

./target/release/termsense suggest "sudo git che" --limit 20 > "$tmp/suggest-context.txt"
grep -Fq "sudo git checkout" "$tmp/suggest-context.txt" \
  || fail "sudo git contextual completion smoke failed"

./target/release/termsense suggest "git status && docker lo" --limit 20 > "$tmp/suggest-segment.txt"
grep -Fq "git status && docker logs" "$tmp/suggest-segment.txt" \
  || fail "active command segment smoke failed"

./target/release/termsense suggest 'echo $(git che' --limit 20 > "$tmp/suggest-substitution.txt"
grep -Fq 'echo $(git checkout' "$tmp/suggest-substitution.txt" \
  || fail "command substitution smoke failed"

./target/release/termsense suggest 'diff <(git che' --limit 20 > "$tmp/suggest-process-sub.txt"
grep -Fq 'diff <(git checkout' "$tmp/suggest-process-sub.txt" \
  || fail "process substitution smoke failed"

./target/release/termsense suggest '( git che' --limit 20 > "$tmp/suggest-group.txt"
grep -Fq '( git checkout' "$tmp/suggest-group.txt" \
  || fail "command group smoke failed"

heredoc_buffer="$(printf 'cat <<EOF\nhello wor')"
./target/release/termsense suggest "$heredoc_buffer" --limit 20 > "$tmp/suggest-heredoc.txt"
[[ ! -s "$tmp/suggest-heredoc.txt" ]] || fail "heredoc body should suppress suggestions"

arithmetic_buffer='echo $((1 + 2'
./target/release/termsense suggest "$arithmetic_buffer" --limit 20 > "$tmp/suggest-arithmetic.txt"
[[ ! -s "$tmp/suggest-arithmetic.txt" ]] || fail "open arithmetic expansion should suppress suggestions"

printf '==> install/uninstall lifecycle\n'
mkdir -p "$tmp/home"

HOME="$tmp/home" \
XDG_CONFIG_HOME="$tmp/home/.config" \
XDG_CACHE_HOME="$tmp/home/.cache" \
XDG_STATE_HOME="$tmp/home/.local/state" \
TERMSENSE_SKIP_BUILD=1 \
bash scripts/install.sh \
  --prefix "$tmp/prefix" \
  --bashrc "$tmp/bashrc" >/dev/null

[[ -x "$tmp/prefix/bin/termsense" ]] || fail "installer did not install binary"
[[ -f "$tmp/home/.config/termsense/config.conf" ]] || fail "installer did not create config"
grep -q '^# >>> termsense >>>$' "$tmp/bashrc" || fail "installer did not add managed Bash block"
grep -q '^TERMSENSE_BIN=' "$tmp/bashrc" || fail "installer did not pin installed binary"

installed_lookup="$(
  HOME="$tmp/home" \
  XDG_CONFIG_HOME="$tmp/home/.config" \
  PATH="/usr/bin:/bin" \
  bash --noprofile --norc -ic "source '$tmp/bashrc'; _termsense_binary" 2>/dev/null
)"
[[ "$installed_lookup" == "$tmp/prefix/bin/termsense" ]] \
  || fail "user-local binary lookup fails when prefix/bin is outside PATH"

HOME="$tmp/home" \
XDG_CONFIG_HOME="$tmp/home/.config" \
XDG_CACHE_HOME="$tmp/home/.cache" \
XDG_STATE_HOME="$tmp/home/.local/state" \
bash scripts/uninstall.sh \
  --prefix "$tmp/prefix" \
  --bashrc "$tmp/bashrc" \
  --purge >/dev/null

[[ ! -e "$tmp/prefix/bin/termsense" ]] || fail "uninstaller did not remove binary"
! grep -q '^# >>> termsense >>>$' "$tmp/bashrc" || fail "uninstaller left managed Bash block"
[[ ! -d "$tmp/home/.config/termsense" ]] || fail "purge left config directory"

printf '==> Debian package\n'
TERMSENSE_SKIP_BUILD=1 bash scripts/package-deb.sh --output "$tmp/dist" >/dev/null

deb_file="$(find "$tmp/dist" -maxdepth 1 -type f -name "termsense_${version}_*.deb" -print -quit)"
[[ -n "$deb_file" ]] || fail "Debian package was not created"

[[ "$(dpkg-deb -f "$deb_file" Package)" == "termsense" ]] \
  || fail "Debian package name mismatch"
[[ "$(dpkg-deb -f "$deb_file" Version)" == "$version" ]] \
  || fail "Debian package version mismatch"
dpkg-deb -c "$deb_file" | grep -q './usr/share/termsense/default.conf' \
  || fail "Debian package is missing reference default config"
dpkg-deb -c "$deb_file" | grep -q './usr/share/doc/termsense/CHANGELOG.md' \
  || fail "Debian package is missing changelog"

printf 'TermSense release readiness: PASS\n'
printf 'version: %s\n' "$version"
printf 'binary: %s\n' "$repo_root/target/release/termsense"
printf 'deb: %s\n' "$deb_file"
 config/default.conf || fail "default config missing ghost"
grep -q '^ctrl_space=1$' config/default.conf || fail "default config missing ctrl_space"
grep -q 'repository = "https://github.com/Anuppaul/termsense"' Cargo.toml \
  || fail "Cargo repository identity mismatch"

if [[ -d .github/workflows ]] && find .github/workflows -type f -print -quit | grep -q .; then
  fail "GitHub Actions workflows are present; project policy currently forbids CI workflows"
fi

printf '==> source integrity\n'
[[ "$(grep -c '^_termsense_query() {' shell/termsense.bash)" -eq 1 ]] \
  || fail "Bash adapter must contain exactly one _termsense_query function"
[[ "$(grep -c '^_termsense_ctrl_space() {' shell/termsense.bash)" -eq 1 ]] \
  || fail "Bash adapter must contain exactly one _termsense_ctrl_space function"
[[ "$(grep -c '^_termsense_draw_overlay() {' shell/termsense.bash)" -eq 1 ]] \
  || fail "Bash adapter must contain exactly one _termsense_draw_overlay function"
[[ "$(grep -c '^pub(crate) fn active_context' src/shell_parse.rs)" -eq 1 ]] \
  || fail "shell parser must contain exactly one active_context function"
[[ "$(grep -c '^fn lex_range' src/shell_parse.rs)" -eq 1 ]] \
  || fail "shell parser must contain exactly one lex_range function"
[[ "$(grep -c '^fn inside_open_heredoc' src/shell_parse.rs)" -eq 1 ]] \
  || fail "shell parser must contain exactly one heredoc detector"

printf '==> git diff check\n'
git diff --check

printf '==> rustfmt\n'
cargo fmt --check

printf '==> tests\n'
cargo test

printf '==> release build\n'
cargo build --release

[[ -s Cargo.lock ]] || fail "Cargo.lock was not generated"
git ls-files --error-unmatch Cargo.lock >/dev/null 2>&1 \
  || fail "Cargo.lock exists but is not committed; commit the generated lockfile before release"
git diff --quiet -- Cargo.lock \
  || fail "Cargo.lock changed during build; commit the updated lockfile before release"

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
[[ "$(grep -c '^_termsense_query() {' "$tmp/termsense-init.bash")" -eq 1 ]] \
  || fail "generated Bash integration contains duplicate query functions"

printf '==> binary smoke\n'
./target/release/termsense status > "$tmp/status.txt"
grep -q '^platform: linux$' "$tmp/status.txt" || fail "status platform smoke failed"
grep -q '^network required: no$' "$tmp/status.txt" || fail "offline status smoke failed"
grep -q '^config file: ' "$tmp/status.txt" || fail "config status smoke failed"

./target/release/termsense suggest d --limit 5 > "$tmp/suggest-command.txt"
[[ -s "$tmp/suggest-command.txt" ]] || fail "command suggestion smoke returned no candidates"

./target/release/termsense suggest "" --limit 0 > "$tmp/suggest-all.txt"
[[ -s "$tmp/suggest-all.txt" ]] || fail "unbounded command discovery smoke returned no candidates"

./target/release/termsense suggest "sudo git che" --limit 20 > "$tmp/suggest-context.txt"
grep -Fq "sudo git checkout" "$tmp/suggest-context.txt" \
  || fail "sudo git contextual completion smoke failed"
awk -F '\t' '$2 == "sudo git checkout" && $9 == "Switch branches or restore files" { found = 1 } END { exit !found }' \
  "$tmp/suggest-context.txt" \
  || fail "suggestion description protocol smoke failed"

./target/release/termsense suggest "git status && docker lo" --limit 20 > "$tmp/suggest-segment.txt"
grep -Fq "git status && docker logs" "$tmp/suggest-segment.txt" \
  || fail "active command segment smoke failed"

./target/release/termsense suggest 'echo $(git che' --limit 20 > "$tmp/suggest-substitution.txt"
grep -Fq 'echo $(git checkout' "$tmp/suggest-substitution.txt" \
  || fail "command substitution smoke failed"

./target/release/termsense suggest 'diff <(git che' --limit 20 > "$tmp/suggest-process-sub.txt"
grep -Fq 'diff <(git checkout' "$tmp/suggest-process-sub.txt" \
  || fail "process substitution smoke failed"

./target/release/termsense suggest '( git che' --limit 20 > "$tmp/suggest-group.txt"
grep -Fq '( git checkout' "$tmp/suggest-group.txt" \
  || fail "command group smoke failed"

heredoc_buffer="$(printf 'cat <<EOF\nhello wor')"
./target/release/termsense suggest "$heredoc_buffer" --limit 20 > "$tmp/suggest-heredoc.txt"
[[ ! -s "$tmp/suggest-heredoc.txt" ]] || fail "heredoc body should suppress suggestions"

arithmetic_buffer='echo $((1 + 2'
./target/release/termsense suggest "$arithmetic_buffer" --limit 20 > "$tmp/suggest-arithmetic.txt"
[[ ! -s "$tmp/suggest-arithmetic.txt" ]] || fail "open arithmetic expansion should suppress suggestions"

printf '==> install/uninstall lifecycle\n'
mkdir -p "$tmp/home"

HOME="$tmp/home" \
XDG_CONFIG_HOME="$tmp/home/.config" \
XDG_CACHE_HOME="$tmp/home/.cache" \
XDG_STATE_HOME="$tmp/home/.local/state" \
TERMSENSE_SKIP_BUILD=1 \
bash scripts/install.sh \
  --prefix "$tmp/prefix" \
  --bashrc "$tmp/bashrc" >/dev/null

[[ -x "$tmp/prefix/bin/termsense" ]] || fail "installer did not install binary"
[[ -f "$tmp/home/.config/termsense/config.conf" ]] || fail "installer did not create config"
grep -q '^# >>> termsense >>>$' "$tmp/bashrc" || fail "installer did not add managed Bash block"
grep -q '^TERMSENSE_BIN=' "$tmp/bashrc" || fail "installer did not pin installed binary"

installed_lookup="$(
  HOME="$tmp/home" \
  XDG_CONFIG_HOME="$tmp/home/.config" \
  PATH="/usr/bin:/bin" \
  bash --noprofile --norc -ic "source '$tmp/bashrc'; _termsense_binary" 2>/dev/null
)"
[[ "$installed_lookup" == "$tmp/prefix/bin/termsense" ]] \
  || fail "user-local binary lookup fails when prefix/bin is outside PATH"

HOME="$tmp/home" \
XDG_CONFIG_HOME="$tmp/home/.config" \
XDG_CACHE_HOME="$tmp/home/.cache" \
XDG_STATE_HOME="$tmp/home/.local/state" \
bash scripts/uninstall.sh \
  --prefix "$tmp/prefix" \
  --bashrc "$tmp/bashrc" \
  --purge >/dev/null

[[ ! -e "$tmp/prefix/bin/termsense" ]] || fail "uninstaller did not remove binary"
! grep -q '^# >>> termsense >>>$' "$tmp/bashrc" || fail "uninstaller left managed Bash block"
[[ ! -d "$tmp/home/.config/termsense" ]] || fail "purge left config directory"

printf '==> Debian package\n'
TERMSENSE_SKIP_BUILD=1 bash scripts/package-deb.sh --output "$tmp/dist" >/dev/null

deb_file="$(find "$tmp/dist" -maxdepth 1 -type f -name "termsense_${version}_*.deb" -print -quit)"
[[ -n "$deb_file" ]] || fail "Debian package was not created"

[[ "$(dpkg-deb -f "$deb_file" Package)" == "termsense" ]] \
  || fail "Debian package name mismatch"
[[ "$(dpkg-deb -f "$deb_file" Version)" == "$version" ]] \
  || fail "Debian package version mismatch"
dpkg-deb -c "$deb_file" | grep -q './usr/share/termsense/default.conf' \
  || fail "Debian package is missing reference default config"
dpkg-deb -c "$deb_file" | grep -q './usr/share/doc/termsense/CHANGELOG.md' \
  || fail "Debian package is missing changelog"

printf 'TermSense release readiness: PASS\n'
printf 'version: %s\n' "$version"
printf 'binary: %s\n' "$repo_root/target/release/termsense"
printf 'deb: %s\n' "$deb_file"
 config/default.conf || fail "default config missing max_visible"
grep -q '^ghost=1$' config/default.conf || fail "default config missing ghost"
grep -q '^ctrl_space=1$' config/default.conf || fail "default config missing ctrl_space"
grep -q 'repository = "https://github.com/Anuppaul/termsense"' Cargo.toml \
  || fail "Cargo repository identity mismatch"

if [[ -d .github/workflows ]] && find .github/workflows -type f -print -quit | grep -q .; then
  fail "GitHub Actions workflows are present; project policy currently forbids CI workflows"
fi

printf '==> source integrity\n'
[[ "$(grep -c '^_termsense_query() {' shell/termsense.bash)" -eq 1 ]] \
  || fail "Bash adapter must contain exactly one _termsense_query function"
[[ "$(grep -c '^_termsense_ctrl_space() {' shell/termsense.bash)" -eq 1 ]] \
  || fail "Bash adapter must contain exactly one _termsense_ctrl_space function"
[[ "$(grep -c '^_termsense_draw_overlay() {' shell/termsense.bash)" -eq 1 ]] \
  || fail "Bash adapter must contain exactly one _termsense_draw_overlay function"
[[ "$(grep -c '^pub(crate) fn active_context' src/shell_parse.rs)" -eq 1 ]] \
  || fail "shell parser must contain exactly one active_context function"
[[ "$(grep -c '^fn lex_range' src/shell_parse.rs)" -eq 1 ]] \
  || fail "shell parser must contain exactly one lex_range function"
[[ "$(grep -c '^fn inside_open_heredoc' src/shell_parse.rs)" -eq 1 ]] \
  || fail "shell parser must contain exactly one heredoc detector"

printf '==> git diff check\n'
git diff --check

printf '==> rustfmt\n'
cargo fmt --check

printf '==> tests\n'
cargo test

printf '==> release build\n'
cargo build --release

[[ -s Cargo.lock ]] || fail "Cargo.lock was not generated"
git ls-files --error-unmatch Cargo.lock >/dev/null 2>&1 \
  || fail "Cargo.lock exists but is not committed; commit the generated lockfile before release"
git diff --quiet -- Cargo.lock \
  || fail "Cargo.lock changed during build; commit the updated lockfile before release"

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
[[ "$(grep -c '^_termsense_query() {' "$tmp/termsense-init.bash")" -eq 1 ]] \
  || fail "generated Bash integration contains duplicate query functions"

printf '==> binary smoke\n'
./target/release/termsense status > "$tmp/status.txt"
grep -q '^platform: linux$' "$tmp/status.txt" || fail "status platform smoke failed"
grep -q '^network required: no$' "$tmp/status.txt" || fail "offline status smoke failed"
grep -q '^config file: ' "$tmp/status.txt" || fail "config status smoke failed"

./target/release/termsense suggest d --limit 5 > "$tmp/suggest-command.txt"
[[ -s "$tmp/suggest-command.txt" ]] || fail "command suggestion smoke returned no candidates"

./target/release/termsense suggest "" --limit 0 > "$tmp/suggest-all.txt"
[[ -s "$tmp/suggest-all.txt" ]] || fail "unbounded command discovery smoke returned no candidates"

./target/release/termsense suggest "sudo git che" --limit 20 > "$tmp/suggest-context.txt"
grep -Fq "sudo git checkout" "$tmp/suggest-context.txt" \
  || fail "sudo git contextual completion smoke failed"

./target/release/termsense suggest "git status && docker lo" --limit 20 > "$tmp/suggest-segment.txt"
grep -Fq "git status && docker logs" "$tmp/suggest-segment.txt" \
  || fail "active command segment smoke failed"

./target/release/termsense suggest 'echo $(git che' --limit 20 > "$tmp/suggest-substitution.txt"
grep -Fq 'echo $(git checkout' "$tmp/suggest-substitution.txt" \
  || fail "command substitution smoke failed"

./target/release/termsense suggest 'diff <(git che' --limit 20 > "$tmp/suggest-process-sub.txt"
grep -Fq 'diff <(git checkout' "$tmp/suggest-process-sub.txt" \
  || fail "process substitution smoke failed"

./target/release/termsense suggest '( git che' --limit 20 > "$tmp/suggest-group.txt"
grep -Fq '( git checkout' "$tmp/suggest-group.txt" \
  || fail "command group smoke failed"

heredoc_buffer="$(printf 'cat <<EOF\nhello wor')"
./target/release/termsense suggest "$heredoc_buffer" --limit 20 > "$tmp/suggest-heredoc.txt"
[[ ! -s "$tmp/suggest-heredoc.txt" ]] || fail "heredoc body should suppress suggestions"

arithmetic_buffer='echo $((1 + 2'
./target/release/termsense suggest "$arithmetic_buffer" --limit 20 > "$tmp/suggest-arithmetic.txt"
[[ ! -s "$tmp/suggest-arithmetic.txt" ]] || fail "open arithmetic expansion should suppress suggestions"

printf '==> install/uninstall lifecycle\n'
mkdir -p "$tmp/home"

HOME="$tmp/home" \
XDG_CONFIG_HOME="$tmp/home/.config" \
XDG_CACHE_HOME="$tmp/home/.cache" \
XDG_STATE_HOME="$tmp/home/.local/state" \
TERMSENSE_SKIP_BUILD=1 \
bash scripts/install.sh \
  --prefix "$tmp/prefix" \
  --bashrc "$tmp/bashrc" >/dev/null

[[ -x "$tmp/prefix/bin/termsense" ]] || fail "installer did not install binary"
[[ -f "$tmp/home/.config/termsense/config.conf" ]] || fail "installer did not create config"
grep -q '^# >>> termsense >>>$' "$tmp/bashrc" || fail "installer did not add managed Bash block"
grep -q '^TERMSENSE_BIN=' "$tmp/bashrc" || fail "installer did not pin installed binary"

installed_lookup="$(
  HOME="$tmp/home" \
  XDG_CONFIG_HOME="$tmp/home/.config" \
  PATH="/usr/bin:/bin" \
  bash --noprofile --norc -ic "source '$tmp/bashrc'; _termsense_binary" 2>/dev/null
)"
[[ "$installed_lookup" == "$tmp/prefix/bin/termsense" ]] \
  || fail "user-local binary lookup fails when prefix/bin is outside PATH"

HOME="$tmp/home" \
XDG_CONFIG_HOME="$tmp/home/.config" \
XDG_CACHE_HOME="$tmp/home/.cache" \
XDG_STATE_HOME="$tmp/home/.local/state" \
bash scripts/uninstall.sh \
  --prefix "$tmp/prefix" \
  --bashrc "$tmp/bashrc" \
  --purge >/dev/null

[[ ! -e "$tmp/prefix/bin/termsense" ]] || fail "uninstaller did not remove binary"
! grep -q '^# >>> termsense >>>$' "$tmp/bashrc" || fail "uninstaller left managed Bash block"
[[ ! -d "$tmp/home/.config/termsense" ]] || fail "purge left config directory"

printf '==> Debian package\n'
TERMSENSE_SKIP_BUILD=1 bash scripts/package-deb.sh --output "$tmp/dist" >/dev/null

deb_file="$(find "$tmp/dist" -maxdepth 1 -type f -name "termsense_${version}_*.deb" -print -quit)"
[[ -n "$deb_file" ]] || fail "Debian package was not created"

[[ "$(dpkg-deb -f "$deb_file" Package)" == "termsense" ]] \
  || fail "Debian package name mismatch"
[[ "$(dpkg-deb -f "$deb_file" Version)" == "$version" ]] \
  || fail "Debian package version mismatch"
dpkg-deb -c "$deb_file" | grep -q './usr/share/termsense/default.conf' \
  || fail "Debian package is missing reference default config"
dpkg-deb -c "$deb_file" | grep -q './usr/share/doc/termsense/CHANGELOG.md' \
  || fail "Debian package is missing changelog"

printf 'TermSense release readiness: PASS\n'
printf 'version: %s\n' "$version"
printf 'binary: %s\n' "$repo_root/target/release/termsense"
printf 'deb: %s\n' "$deb_file"
 config/default.conf || fail \"default config missing max_visible\"$" scripts/release-readiness.sh \
  || fail "release readiness max_visible guard is malformed"
[[ "$(grep -c '^_termsense_query() {' shell/termsense.bash)" -eq 1 ]] \
  || fail "Bash adapter must contain exactly one _termsense_query function"
[[ "$(grep -c '^_termsense_ctrl_space() {' shell/termsense.bash)" -eq 1 ]] \
  || fail "Bash adapter must contain exactly one _termsense_ctrl_space function"
[[ "$(grep -c '^_termsense_draw_overlay() {' shell/termsense.bash)" -eq 1 ]] \
  || fail "Bash adapter must contain exactly one _termsense_draw_overlay function"
[[ "$(grep -c '^pub(crate) fn active_context' src/shell_parse.rs)" -eq 1 ]] \
  || fail "shell parser must contain exactly one active_context function"
[[ "$(grep -c '^fn lex_range' src/shell_parse.rs)" -eq 1 ]] \
  || fail "shell parser must contain exactly one lex_range function"
[[ "$(grep -c '^fn inside_open_heredoc' src/shell_parse.rs)" -eq 1 ]] \
  || fail "shell parser must contain exactly one heredoc detector"

printf '==> git diff check\n'
git diff --check

printf '==> rustfmt\n'
cargo fmt --check

printf '==> tests\n'
cargo test

printf '==> release build\n'
cargo build --release

[[ -s Cargo.lock ]] || fail "Cargo.lock was not generated"
git ls-files --error-unmatch Cargo.lock >/dev/null 2>&1 \
  || fail "Cargo.lock exists but is not committed; commit the generated lockfile before release"
git diff --quiet -- Cargo.lock \
  || fail "Cargo.lock changed during build; commit the updated lockfile before release"

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
[[ "$(grep -c '^_termsense_query() {' "$tmp/termsense-init.bash")" -eq 1 ]] \
  || fail "generated Bash integration contains duplicate query functions"

printf '==> binary smoke\n'
./target/release/termsense status > "$tmp/status.txt"
grep -q '^platform: linux$' "$tmp/status.txt" || fail "status platform smoke failed"
grep -q '^network required: no$' "$tmp/status.txt" || fail "offline status smoke failed"
grep -q '^config file: ' "$tmp/status.txt" || fail "config status smoke failed"

./target/release/termsense suggest d --limit 5 > "$tmp/suggest-command.txt"
[[ -s "$tmp/suggest-command.txt" ]] || fail "command suggestion smoke returned no candidates"

./target/release/termsense suggest "" --limit 0 > "$tmp/suggest-all.txt"
[[ -s "$tmp/suggest-all.txt" ]] || fail "unbounded command discovery smoke returned no candidates"

./target/release/termsense suggest "sudo git che" --limit 20 > "$tmp/suggest-context.txt"
grep -Fq "sudo git checkout" "$tmp/suggest-context.txt" \
  || fail "sudo git contextual completion smoke failed"
awk -F '\t' '$2 == "sudo git checkout" && $9 == "Switch branches or restore files" { found = 1 } END { exit !found }' \
  "$tmp/suggest-context.txt" \
  || fail "suggestion description protocol smoke failed"

./target/release/termsense suggest "git status && docker lo" --limit 20 > "$tmp/suggest-segment.txt"
grep -Fq "git status && docker logs" "$tmp/suggest-segment.txt" \
  || fail "active command segment smoke failed"

./target/release/termsense suggest 'echo $(git che' --limit 20 > "$tmp/suggest-substitution.txt"
grep -Fq 'echo $(git checkout' "$tmp/suggest-substitution.txt" \
  || fail "command substitution smoke failed"

./target/release/termsense suggest 'diff <(git che' --limit 20 > "$tmp/suggest-process-sub.txt"
grep -Fq 'diff <(git checkout' "$tmp/suggest-process-sub.txt" \
  || fail "process substitution smoke failed"

./target/release/termsense suggest '( git che' --limit 20 > "$tmp/suggest-group.txt"
grep -Fq '( git checkout' "$tmp/suggest-group.txt" \
  || fail "command group smoke failed"

heredoc_buffer="$(printf 'cat <<EOF\nhello wor')"
./target/release/termsense suggest "$heredoc_buffer" --limit 20 > "$tmp/suggest-heredoc.txt"
[[ ! -s "$tmp/suggest-heredoc.txt" ]] || fail "heredoc body should suppress suggestions"

arithmetic_buffer='echo $((1 + 2'
./target/release/termsense suggest "$arithmetic_buffer" --limit 20 > "$tmp/suggest-arithmetic.txt"
[[ ! -s "$tmp/suggest-arithmetic.txt" ]] || fail "open arithmetic expansion should suppress suggestions"

printf '==> install/uninstall lifecycle\n'
mkdir -p "$tmp/home"

HOME="$tmp/home" \
XDG_CONFIG_HOME="$tmp/home/.config" \
XDG_CACHE_HOME="$tmp/home/.cache" \
XDG_STATE_HOME="$tmp/home/.local/state" \
TERMSENSE_SKIP_BUILD=1 \
bash scripts/install.sh \
  --prefix "$tmp/prefix" \
  --bashrc "$tmp/bashrc" >/dev/null

[[ -x "$tmp/prefix/bin/termsense" ]] || fail "installer did not install binary"
[[ -f "$tmp/home/.config/termsense/config.conf" ]] || fail "installer did not create config"
grep -q '^# >>> termsense >>>$' "$tmp/bashrc" || fail "installer did not add managed Bash block"
grep -q '^TERMSENSE_BIN=' "$tmp/bashrc" || fail "installer did not pin installed binary"

installed_lookup="$(
  HOME="$tmp/home" \
  XDG_CONFIG_HOME="$tmp/home/.config" \
  PATH="/usr/bin:/bin" \
  bash --noprofile --norc -ic "source '$tmp/bashrc'; _termsense_binary" 2>/dev/null
)"
[[ "$installed_lookup" == "$tmp/prefix/bin/termsense" ]] \
  || fail "user-local binary lookup fails when prefix/bin is outside PATH"

HOME="$tmp/home" \
XDG_CONFIG_HOME="$tmp/home/.config" \
XDG_CACHE_HOME="$tmp/home/.cache" \
XDG_STATE_HOME="$tmp/home/.local/state" \
bash scripts/uninstall.sh \
  --prefix "$tmp/prefix" \
  --bashrc "$tmp/bashrc" \
  --purge >/dev/null

[[ ! -e "$tmp/prefix/bin/termsense" ]] || fail "uninstaller did not remove binary"
! grep -q '^# >>> termsense >>>$' "$tmp/bashrc" || fail "uninstaller left managed Bash block"
[[ ! -d "$tmp/home/.config/termsense" ]] || fail "purge left config directory"

printf '==> Debian package\n'
TERMSENSE_SKIP_BUILD=1 bash scripts/package-deb.sh --output "$tmp/dist" >/dev/null

deb_file="$(find "$tmp/dist" -maxdepth 1 -type f -name "termsense_${version}_*.deb" -print -quit)"
[[ -n "$deb_file" ]] || fail "Debian package was not created"

[[ "$(dpkg-deb -f "$deb_file" Package)" == "termsense" ]] \
  || fail "Debian package name mismatch"
[[ "$(dpkg-deb -f "$deb_file" Version)" == "$version" ]] \
  || fail "Debian package version mismatch"
dpkg-deb -c "$deb_file" | grep -q './usr/share/termsense/default.conf' \
  || fail "Debian package is missing reference default config"
dpkg-deb -c "$deb_file" | grep -q './usr/share/doc/termsense/CHANGELOG.md' \
  || fail "Debian package is missing changelog"

printf 'TermSense release readiness: PASS\n'
printf 'version: %s\n' "$version"
printf 'binary: %s\n' "$repo_root/target/release/termsense"
printf 'deb: %s\n' "$deb_file"
 config/default.conf || fail "default config missing max_visible"
grep -q '^ghost=1$' config/default.conf || fail "default config missing ghost"
grep -q '^ctrl_space=1$' config/default.conf || fail "default config missing ctrl_space"
grep -q 'repository = "https://github.com/Anuppaul/termsense"' Cargo.toml \
  || fail "Cargo repository identity mismatch"

if [[ -d .github/workflows ]] && find .github/workflows -type f -print -quit | grep -q .; then
  fail "GitHub Actions workflows are present; project policy currently forbids CI workflows"
fi

printf '==> source integrity\n'
[[ "$(grep -c '^_termsense_query() {' shell/termsense.bash)" -eq 1 ]] \
  || fail "Bash adapter must contain exactly one _termsense_query function"
[[ "$(grep -c '^_termsense_ctrl_space() {' shell/termsense.bash)" -eq 1 ]] \
  || fail "Bash adapter must contain exactly one _termsense_ctrl_space function"
[[ "$(grep -c '^_termsense_draw_overlay() {' shell/termsense.bash)" -eq 1 ]] \
  || fail "Bash adapter must contain exactly one _termsense_draw_overlay function"
[[ "$(grep -c '^pub(crate) fn active_context' src/shell_parse.rs)" -eq 1 ]] \
  || fail "shell parser must contain exactly one active_context function"
[[ "$(grep -c '^fn lex_range' src/shell_parse.rs)" -eq 1 ]] \
  || fail "shell parser must contain exactly one lex_range function"
[[ "$(grep -c '^fn inside_open_heredoc' src/shell_parse.rs)" -eq 1 ]] \
  || fail "shell parser must contain exactly one heredoc detector"

printf '==> git diff check\n'
git diff --check

printf '==> rustfmt\n'
cargo fmt --check

printf '==> tests\n'
cargo test

printf '==> release build\n'
cargo build --release

[[ -s Cargo.lock ]] || fail "Cargo.lock was not generated"
git ls-files --error-unmatch Cargo.lock >/dev/null 2>&1 \
  || fail "Cargo.lock exists but is not committed; commit the generated lockfile before release"
git diff --quiet -- Cargo.lock \
  || fail "Cargo.lock changed during build; commit the updated lockfile before release"

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
[[ "$(grep -c '^_termsense_query() {' "$tmp/termsense-init.bash")" -eq 1 ]] \
  || fail "generated Bash integration contains duplicate query functions"

printf '==> binary smoke\n'
./target/release/termsense status > "$tmp/status.txt"
grep -q '^platform: linux$' "$tmp/status.txt" || fail "status platform smoke failed"
grep -q '^network required: no$' "$tmp/status.txt" || fail "offline status smoke failed"
grep -q '^config file: ' "$tmp/status.txt" || fail "config status smoke failed"

./target/release/termsense suggest d --limit 5 > "$tmp/suggest-command.txt"
[[ -s "$tmp/suggest-command.txt" ]] || fail "command suggestion smoke returned no candidates"

./target/release/termsense suggest "" --limit 0 > "$tmp/suggest-all.txt"
[[ -s "$tmp/suggest-all.txt" ]] || fail "unbounded command discovery smoke returned no candidates"

./target/release/termsense suggest "sudo git che" --limit 20 > "$tmp/suggest-context.txt"
grep -Fq "sudo git checkout" "$tmp/suggest-context.txt" \
  || fail "sudo git contextual completion smoke failed"

./target/release/termsense suggest "git status && docker lo" --limit 20 > "$tmp/suggest-segment.txt"
grep -Fq "git status && docker logs" "$tmp/suggest-segment.txt" \
  || fail "active command segment smoke failed"

./target/release/termsense suggest 'echo $(git che' --limit 20 > "$tmp/suggest-substitution.txt"
grep -Fq 'echo $(git checkout' "$tmp/suggest-substitution.txt" \
  || fail "command substitution smoke failed"

./target/release/termsense suggest 'diff <(git che' --limit 20 > "$tmp/suggest-process-sub.txt"
grep -Fq 'diff <(git checkout' "$tmp/suggest-process-sub.txt" \
  || fail "process substitution smoke failed"

./target/release/termsense suggest '( git che' --limit 20 > "$tmp/suggest-group.txt"
grep -Fq '( git checkout' "$tmp/suggest-group.txt" \
  || fail "command group smoke failed"

heredoc_buffer="$(printf 'cat <<EOF\nhello wor')"
./target/release/termsense suggest "$heredoc_buffer" --limit 20 > "$tmp/suggest-heredoc.txt"
[[ ! -s "$tmp/suggest-heredoc.txt" ]] || fail "heredoc body should suppress suggestions"

arithmetic_buffer='echo $((1 + 2'
./target/release/termsense suggest "$arithmetic_buffer" --limit 20 > "$tmp/suggest-arithmetic.txt"
[[ ! -s "$tmp/suggest-arithmetic.txt" ]] || fail "open arithmetic expansion should suppress suggestions"

printf '==> install/uninstall lifecycle\n'
mkdir -p "$tmp/home"

HOME="$tmp/home" \
XDG_CONFIG_HOME="$tmp/home/.config" \
XDG_CACHE_HOME="$tmp/home/.cache" \
XDG_STATE_HOME="$tmp/home/.local/state" \
TERMSENSE_SKIP_BUILD=1 \
bash scripts/install.sh \
  --prefix "$tmp/prefix" \
  --bashrc "$tmp/bashrc" >/dev/null

[[ -x "$tmp/prefix/bin/termsense" ]] || fail "installer did not install binary"
[[ -f "$tmp/home/.config/termsense/config.conf" ]] || fail "installer did not create config"
grep -q '^# >>> termsense >>>$' "$tmp/bashrc" || fail "installer did not add managed Bash block"
grep -q '^TERMSENSE_BIN=' "$tmp/bashrc" || fail "installer did not pin installed binary"

installed_lookup="$(
  HOME="$tmp/home" \
  XDG_CONFIG_HOME="$tmp/home/.config" \
  PATH="/usr/bin:/bin" \
  bash --noprofile --norc -ic "source '$tmp/bashrc'; _termsense_binary" 2>/dev/null
)"
[[ "$installed_lookup" == "$tmp/prefix/bin/termsense" ]] \
  || fail "user-local binary lookup fails when prefix/bin is outside PATH"

HOME="$tmp/home" \
XDG_CONFIG_HOME="$tmp/home/.config" \
XDG_CACHE_HOME="$tmp/home/.cache" \
XDG_STATE_HOME="$tmp/home/.local/state" \
bash scripts/uninstall.sh \
  --prefix "$tmp/prefix" \
  --bashrc "$tmp/bashrc" \
  --purge >/dev/null

[[ ! -e "$tmp/prefix/bin/termsense" ]] || fail "uninstaller did not remove binary"
! grep -q '^# >>> termsense >>>$' "$tmp/bashrc" || fail "uninstaller left managed Bash block"
[[ ! -d "$tmp/home/.config/termsense" ]] || fail "purge left config directory"

printf '==> Debian package\n'
TERMSENSE_SKIP_BUILD=1 bash scripts/package-deb.sh --output "$tmp/dist" >/dev/null

deb_file="$(find "$tmp/dist" -maxdepth 1 -type f -name "termsense_${version}_*.deb" -print -quit)"
[[ -n "$deb_file" ]] || fail "Debian package was not created"

[[ "$(dpkg-deb -f "$deb_file" Package)" == "termsense" ]] \
  || fail "Debian package name mismatch"
[[ "$(dpkg-deb -f "$deb_file" Version)" == "$version" ]] \
  || fail "Debian package version mismatch"
dpkg-deb -c "$deb_file" | grep -q './usr/share/termsense/default.conf' \
  || fail "Debian package is missing reference default config"
dpkg-deb -c "$deb_file" | grep -q './usr/share/doc/termsense/CHANGELOG.md' \
  || fail "Debian package is missing changelog"

printf 'TermSense release readiness: PASS\n'
printf 'version: %s\n' "$version"
printf 'binary: %s\n' "$repo_root/target/release/termsense"
printf 'deb: %s\n' "$deb_file"
 config/default.conf || fail "default config missing max_visible"
grep -q '^ghost=1
grep -q '^ctrl_space=1$' config/default.conf || fail "default config missing ctrl_space"
grep -q 'repository = "https://github.com/Anuppaul/termsense"' Cargo.toml \
  || fail "Cargo repository identity mismatch"

if [[ -d .github/workflows ]] && find .github/workflows -type f -print -quit | grep -q .; then
  fail "GitHub Actions workflows are present; project policy currently forbids CI workflows"
fi

printf '==> source integrity\n'
[[ "$(grep -c '^_termsense_query() {' shell/termsense.bash)" -eq 1 ]] \
  || fail "Bash adapter must contain exactly one _termsense_query function"
[[ "$(grep -c '^_termsense_ctrl_space() {' shell/termsense.bash)" -eq 1 ]] \
  || fail "Bash adapter must contain exactly one _termsense_ctrl_space function"
[[ "$(grep -c '^_termsense_draw_overlay() {' shell/termsense.bash)" -eq 1 ]] \
  || fail "Bash adapter must contain exactly one _termsense_draw_overlay function"
[[ "$(grep -c '^pub(crate) fn active_context' src/shell_parse.rs)" -eq 1 ]] \
  || fail "shell parser must contain exactly one active_context function"
[[ "$(grep -c '^fn lex_range' src/shell_parse.rs)" -eq 1 ]] \
  || fail "shell parser must contain exactly one lex_range function"
[[ "$(grep -c '^fn inside_open_heredoc' src/shell_parse.rs)" -eq 1 ]] \
  || fail "shell parser must contain exactly one heredoc detector"

printf '==> git diff check\n'
git diff --check

printf '==> rustfmt\n'
cargo fmt --check

printf '==> tests\n'
cargo test

printf '==> release build\n'
cargo build --release

[[ -s Cargo.lock ]] || fail "Cargo.lock was not generated"
git ls-files --error-unmatch Cargo.lock >/dev/null 2>&1 \
  || fail "Cargo.lock exists but is not committed; commit the generated lockfile before release"
git diff --quiet -- Cargo.lock \
  || fail "Cargo.lock changed during build; commit the updated lockfile before release"

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
[[ "$(grep -c '^_termsense_query() {' "$tmp/termsense-init.bash")" -eq 1 ]] \
  || fail "generated Bash integration contains duplicate query functions"

printf '==> binary smoke\n'
./target/release/termsense status > "$tmp/status.txt"
grep -q '^platform: linux$' "$tmp/status.txt" || fail "status platform smoke failed"
grep -q '^network required: no$' "$tmp/status.txt" || fail "offline status smoke failed"
grep -q '^config file: ' "$tmp/status.txt" || fail "config status smoke failed"

./target/release/termsense suggest d --limit 5 > "$tmp/suggest-command.txt"
[[ -s "$tmp/suggest-command.txt" ]] || fail "command suggestion smoke returned no candidates"

./target/release/termsense suggest "" --limit 0 > "$tmp/suggest-all.txt"
[[ -s "$tmp/suggest-all.txt" ]] || fail "unbounded command discovery smoke returned no candidates"

./target/release/termsense suggest "sudo git che" --limit 20 > "$tmp/suggest-context.txt"
grep -Fq "sudo git checkout" "$tmp/suggest-context.txt" \
  || fail "sudo git contextual completion smoke failed"
awk -F '\t' '$2 == "sudo git checkout" && $9 == "Switch branches or restore files" { found = 1 } END { exit !found }' \
  "$tmp/suggest-context.txt" \
  || fail "suggestion description protocol smoke failed"

./target/release/termsense suggest "git status && docker lo" --limit 20 > "$tmp/suggest-segment.txt"
grep -Fq "git status && docker logs" "$tmp/suggest-segment.txt" \
  || fail "active command segment smoke failed"

./target/release/termsense suggest 'echo $(git che' --limit 20 > "$tmp/suggest-substitution.txt"
grep -Fq 'echo $(git checkout' "$tmp/suggest-substitution.txt" \
  || fail "command substitution smoke failed"

./target/release/termsense suggest 'diff <(git che' --limit 20 > "$tmp/suggest-process-sub.txt"
grep -Fq 'diff <(git checkout' "$tmp/suggest-process-sub.txt" \
  || fail "process substitution smoke failed"

./target/release/termsense suggest '( git che' --limit 20 > "$tmp/suggest-group.txt"
grep -Fq '( git checkout' "$tmp/suggest-group.txt" \
  || fail "command group smoke failed"

heredoc_buffer="$(printf 'cat <<EOF\nhello wor')"
./target/release/termsense suggest "$heredoc_buffer" --limit 20 > "$tmp/suggest-heredoc.txt"
[[ ! -s "$tmp/suggest-heredoc.txt" ]] || fail "heredoc body should suppress suggestions"

arithmetic_buffer='echo $((1 + 2'
./target/release/termsense suggest "$arithmetic_buffer" --limit 20 > "$tmp/suggest-arithmetic.txt"
[[ ! -s "$tmp/suggest-arithmetic.txt" ]] || fail "open arithmetic expansion should suppress suggestions"

printf '==> install/uninstall lifecycle\n'
mkdir -p "$tmp/home"

HOME="$tmp/home" \
XDG_CONFIG_HOME="$tmp/home/.config" \
XDG_CACHE_HOME="$tmp/home/.cache" \
XDG_STATE_HOME="$tmp/home/.local/state" \
TERMSENSE_SKIP_BUILD=1 \
bash scripts/install.sh \
  --prefix "$tmp/prefix" \
  --bashrc "$tmp/bashrc" >/dev/null

[[ -x "$tmp/prefix/bin/termsense" ]] || fail "installer did not install binary"
[[ -f "$tmp/home/.config/termsense/config.conf" ]] || fail "installer did not create config"
grep -q '^# >>> termsense >>>$' "$tmp/bashrc" || fail "installer did not add managed Bash block"
grep -q '^TERMSENSE_BIN=' "$tmp/bashrc" || fail "installer did not pin installed binary"

installed_lookup="$(
  HOME="$tmp/home" \
  XDG_CONFIG_HOME="$tmp/home/.config" \
  PATH="/usr/bin:/bin" \
  bash --noprofile --norc -ic "source '$tmp/bashrc'; _termsense_binary" 2>/dev/null
)"
[[ "$installed_lookup" == "$tmp/prefix/bin/termsense" ]] \
  || fail "user-local binary lookup fails when prefix/bin is outside PATH"

HOME="$tmp/home" \
XDG_CONFIG_HOME="$tmp/home/.config" \
XDG_CACHE_HOME="$tmp/home/.cache" \
XDG_STATE_HOME="$tmp/home/.local/state" \
bash scripts/uninstall.sh \
  --prefix "$tmp/prefix" \
  --bashrc "$tmp/bashrc" \
  --purge >/dev/null

[[ ! -e "$tmp/prefix/bin/termsense" ]] || fail "uninstaller did not remove binary"
! grep -q '^# >>> termsense >>>$' "$tmp/bashrc" || fail "uninstaller left managed Bash block"
[[ ! -d "$tmp/home/.config/termsense" ]] || fail "purge left config directory"

printf '==> Debian package\n'
TERMSENSE_SKIP_BUILD=1 bash scripts/package-deb.sh --output "$tmp/dist" >/dev/null

deb_file="$(find "$tmp/dist" -maxdepth 1 -type f -name "termsense_${version}_*.deb" -print -quit)"
[[ -n "$deb_file" ]] || fail "Debian package was not created"

[[ "$(dpkg-deb -f "$deb_file" Package)" == "termsense" ]] \
  || fail "Debian package name mismatch"
[[ "$(dpkg-deb -f "$deb_file" Version)" == "$version" ]] \
  || fail "Debian package version mismatch"
dpkg-deb -c "$deb_file" | grep -q './usr/share/termsense/default.conf' \
  || fail "Debian package is missing reference default config"
dpkg-deb -c "$deb_file" | grep -q './usr/share/doc/termsense/CHANGELOG.md' \
  || fail "Debian package is missing changelog"

printf 'TermSense release readiness: PASS\n'
printf 'version: %s\n' "$version"
printf 'binary: %s\n' "$repo_root/target/release/termsense"
printf 'deb: %s\n' "$deb_file"
 config/default.conf || fail "default config missing max_visible"
grep -q '^ghost=1$' config/default.conf || fail "default config missing ghost"
grep -q '^ctrl_space=1$' config/default.conf || fail "default config missing ctrl_space"
grep -q 'repository = "https://github.com/Anuppaul/termsense"' Cargo.toml \
  || fail "Cargo repository identity mismatch"

if [[ -d .github/workflows ]] && find .github/workflows -type f -print -quit | grep -q .; then
  fail "GitHub Actions workflows are present; project policy currently forbids CI workflows"
fi

printf '==> source integrity\n'
[[ "$(grep -c '^_termsense_query() {' shell/termsense.bash)" -eq 1 ]] \
  || fail "Bash adapter must contain exactly one _termsense_query function"
[[ "$(grep -c '^_termsense_ctrl_space() {' shell/termsense.bash)" -eq 1 ]] \
  || fail "Bash adapter must contain exactly one _termsense_ctrl_space function"
[[ "$(grep -c '^_termsense_draw_overlay() {' shell/termsense.bash)" -eq 1 ]] \
  || fail "Bash adapter must contain exactly one _termsense_draw_overlay function"
[[ "$(grep -c '^pub(crate) fn active_context' src/shell_parse.rs)" -eq 1 ]] \
  || fail "shell parser must contain exactly one active_context function"
[[ "$(grep -c '^fn lex_range' src/shell_parse.rs)" -eq 1 ]] \
  || fail "shell parser must contain exactly one lex_range function"
[[ "$(grep -c '^fn inside_open_heredoc' src/shell_parse.rs)" -eq 1 ]] \
  || fail "shell parser must contain exactly one heredoc detector"

printf '==> git diff check\n'
git diff --check

printf '==> rustfmt\n'
cargo fmt --check

printf '==> tests\n'
cargo test

printf '==> release build\n'
cargo build --release

[[ -s Cargo.lock ]] || fail "Cargo.lock was not generated"
git ls-files --error-unmatch Cargo.lock >/dev/null 2>&1 \
  || fail "Cargo.lock exists but is not committed; commit the generated lockfile before release"
git diff --quiet -- Cargo.lock \
  || fail "Cargo.lock changed during build; commit the updated lockfile before release"

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
[[ "$(grep -c '^_termsense_query() {' "$tmp/termsense-init.bash")" -eq 1 ]] \
  || fail "generated Bash integration contains duplicate query functions"

printf '==> binary smoke\n'
./target/release/termsense status > "$tmp/status.txt"
grep -q '^platform: linux$' "$tmp/status.txt" || fail "status platform smoke failed"
grep -q '^network required: no$' "$tmp/status.txt" || fail "offline status smoke failed"
grep -q '^config file: ' "$tmp/status.txt" || fail "config status smoke failed"

./target/release/termsense suggest d --limit 5 > "$tmp/suggest-command.txt"
[[ -s "$tmp/suggest-command.txt" ]] || fail "command suggestion smoke returned no candidates"

./target/release/termsense suggest "" --limit 0 > "$tmp/suggest-all.txt"
[[ -s "$tmp/suggest-all.txt" ]] || fail "unbounded command discovery smoke returned no candidates"

./target/release/termsense suggest "sudo git che" --limit 20 > "$tmp/suggest-context.txt"
grep -Fq "sudo git checkout" "$tmp/suggest-context.txt" \
  || fail "sudo git contextual completion smoke failed"

./target/release/termsense suggest "git status && docker lo" --limit 20 > "$tmp/suggest-segment.txt"
grep -Fq "git status && docker logs" "$tmp/suggest-segment.txt" \
  || fail "active command segment smoke failed"

./target/release/termsense suggest 'echo $(git che' --limit 20 > "$tmp/suggest-substitution.txt"
grep -Fq 'echo $(git checkout' "$tmp/suggest-substitution.txt" \
  || fail "command substitution smoke failed"

./target/release/termsense suggest 'diff <(git che' --limit 20 > "$tmp/suggest-process-sub.txt"
grep -Fq 'diff <(git checkout' "$tmp/suggest-process-sub.txt" \
  || fail "process substitution smoke failed"

./target/release/termsense suggest '( git che' --limit 20 > "$tmp/suggest-group.txt"
grep -Fq '( git checkout' "$tmp/suggest-group.txt" \
  || fail "command group smoke failed"

heredoc_buffer="$(printf 'cat <<EOF\nhello wor')"
./target/release/termsense suggest "$heredoc_buffer" --limit 20 > "$tmp/suggest-heredoc.txt"
[[ ! -s "$tmp/suggest-heredoc.txt" ]] || fail "heredoc body should suppress suggestions"

arithmetic_buffer='echo $((1 + 2'
./target/release/termsense suggest "$arithmetic_buffer" --limit 20 > "$tmp/suggest-arithmetic.txt"
[[ ! -s "$tmp/suggest-arithmetic.txt" ]] || fail "open arithmetic expansion should suppress suggestions"

printf '==> install/uninstall lifecycle\n'
mkdir -p "$tmp/home"

HOME="$tmp/home" \
XDG_CONFIG_HOME="$tmp/home/.config" \
XDG_CACHE_HOME="$tmp/home/.cache" \
XDG_STATE_HOME="$tmp/home/.local/state" \
TERMSENSE_SKIP_BUILD=1 \
bash scripts/install.sh \
  --prefix "$tmp/prefix" \
  --bashrc "$tmp/bashrc" >/dev/null

[[ -x "$tmp/prefix/bin/termsense" ]] || fail "installer did not install binary"
[[ -f "$tmp/home/.config/termsense/config.conf" ]] || fail "installer did not create config"
grep -q '^# >>> termsense >>>$' "$tmp/bashrc" || fail "installer did not add managed Bash block"
grep -q '^TERMSENSE_BIN=' "$tmp/bashrc" || fail "installer did not pin installed binary"

installed_lookup="$(
  HOME="$tmp/home" \
  XDG_CONFIG_HOME="$tmp/home/.config" \
  PATH="/usr/bin:/bin" \
  bash --noprofile --norc -ic "source '$tmp/bashrc'; _termsense_binary" 2>/dev/null
)"
[[ "$installed_lookup" == "$tmp/prefix/bin/termsense" ]] \
  || fail "user-local binary lookup fails when prefix/bin is outside PATH"

HOME="$tmp/home" \
XDG_CONFIG_HOME="$tmp/home/.config" \
XDG_CACHE_HOME="$tmp/home/.cache" \
XDG_STATE_HOME="$tmp/home/.local/state" \
bash scripts/uninstall.sh \
  --prefix "$tmp/prefix" \
  --bashrc "$tmp/bashrc" \
  --purge >/dev/null

[[ ! -e "$tmp/prefix/bin/termsense" ]] || fail "uninstaller did not remove binary"
! grep -q '^# >>> termsense >>>$' "$tmp/bashrc" || fail "uninstaller left managed Bash block"
[[ ! -d "$tmp/home/.config/termsense" ]] || fail "purge left config directory"

printf '==> Debian package\n'
TERMSENSE_SKIP_BUILD=1 bash scripts/package-deb.sh --output "$tmp/dist" >/dev/null

deb_file="$(find "$tmp/dist" -maxdepth 1 -type f -name "termsense_${version}_*.deb" -print -quit)"
[[ -n "$deb_file" ]] || fail "Debian package was not created"

[[ "$(dpkg-deb -f "$deb_file" Package)" == "termsense" ]] \
  || fail "Debian package name mismatch"
[[ "$(dpkg-deb -f "$deb_file" Version)" == "$version" ]] \
  || fail "Debian package version mismatch"
dpkg-deb -c "$deb_file" | grep -q './usr/share/termsense/default.conf' \
  || fail "Debian package is missing reference default config"
dpkg-deb -c "$deb_file" | grep -q './usr/share/doc/termsense/CHANGELOG.md' \
  || fail "Debian package is missing changelog"

printf 'TermSense release readiness: PASS\n'
printf 'version: %s\n' "$version"
printf 'binary: %s\n' "$repo_root/target/release/termsense"
printf 'deb: %s\n' "$deb_file"
 config/default.conf || fail "default config missing ghost"
grep -q '^ctrl_space=1$' config/default.conf || fail "default config missing ctrl_space"
grep -q 'repository = "https://github.com/Anuppaul/termsense"' Cargo.toml \
  || fail "Cargo repository identity mismatch"

if [[ -d .github/workflows ]] && find .github/workflows -type f -print -quit | grep -q .; then
  fail "GitHub Actions workflows are present; project policy currently forbids CI workflows"
fi

printf '==> source integrity\n'
[[ "$(grep -c '^_termsense_query() {' shell/termsense.bash)" -eq 1 ]] \
  || fail "Bash adapter must contain exactly one _termsense_query function"
[[ "$(grep -c '^_termsense_ctrl_space() {' shell/termsense.bash)" -eq 1 ]] \
  || fail "Bash adapter must contain exactly one _termsense_ctrl_space function"
[[ "$(grep -c '^_termsense_draw_overlay() {' shell/termsense.bash)" -eq 1 ]] \
  || fail "Bash adapter must contain exactly one _termsense_draw_overlay function"
[[ "$(grep -c '^pub(crate) fn active_context' src/shell_parse.rs)" -eq 1 ]] \
  || fail "shell parser must contain exactly one active_context function"
[[ "$(grep -c '^fn lex_range' src/shell_parse.rs)" -eq 1 ]] \
  || fail "shell parser must contain exactly one lex_range function"
[[ "$(grep -c '^fn inside_open_heredoc' src/shell_parse.rs)" -eq 1 ]] \
  || fail "shell parser must contain exactly one heredoc detector"

printf '==> git diff check\n'
git diff --check

printf '==> rustfmt\n'
cargo fmt --check

printf '==> tests\n'
cargo test

printf '==> release build\n'
cargo build --release

[[ -s Cargo.lock ]] || fail "Cargo.lock was not generated"
git ls-files --error-unmatch Cargo.lock >/dev/null 2>&1 \
  || fail "Cargo.lock exists but is not committed; commit the generated lockfile before release"
git diff --quiet -- Cargo.lock \
  || fail "Cargo.lock changed during build; commit the updated lockfile before release"

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
[[ "$(grep -c '^_termsense_query() {' "$tmp/termsense-init.bash")" -eq 1 ]] \
  || fail "generated Bash integration contains duplicate query functions"

printf '==> binary smoke\n'
./target/release/termsense status > "$tmp/status.txt"
grep -q '^platform: linux$' "$tmp/status.txt" || fail "status platform smoke failed"
grep -q '^network required: no$' "$tmp/status.txt" || fail "offline status smoke failed"
grep -q '^config file: ' "$tmp/status.txt" || fail "config status smoke failed"

./target/release/termsense suggest d --limit 5 > "$tmp/suggest-command.txt"
[[ -s "$tmp/suggest-command.txt" ]] || fail "command suggestion smoke returned no candidates"

./target/release/termsense suggest "" --limit 0 > "$tmp/suggest-all.txt"
[[ -s "$tmp/suggest-all.txt" ]] || fail "unbounded command discovery smoke returned no candidates"

./target/release/termsense suggest "sudo git che" --limit 20 > "$tmp/suggest-context.txt"
grep -Fq "sudo git checkout" "$tmp/suggest-context.txt" \
  || fail "sudo git contextual completion smoke failed"
awk -F '\t' '$2 == "sudo git checkout" && $9 == "Switch branches or restore files" { found = 1 } END { exit !found }' \
  "$tmp/suggest-context.txt" \
  || fail "suggestion description protocol smoke failed"

./target/release/termsense suggest "git status && docker lo" --limit 20 > "$tmp/suggest-segment.txt"
grep -Fq "git status && docker logs" "$tmp/suggest-segment.txt" \
  || fail "active command segment smoke failed"

./target/release/termsense suggest 'echo $(git che' --limit 20 > "$tmp/suggest-substitution.txt"
grep -Fq 'echo $(git checkout' "$tmp/suggest-substitution.txt" \
  || fail "command substitution smoke failed"

./target/release/termsense suggest 'diff <(git che' --limit 20 > "$tmp/suggest-process-sub.txt"
grep -Fq 'diff <(git checkout' "$tmp/suggest-process-sub.txt" \
  || fail "process substitution smoke failed"

./target/release/termsense suggest '( git che' --limit 20 > "$tmp/suggest-group.txt"
grep -Fq '( git checkout' "$tmp/suggest-group.txt" \
  || fail "command group smoke failed"

heredoc_buffer="$(printf 'cat <<EOF\nhello wor')"
./target/release/termsense suggest "$heredoc_buffer" --limit 20 > "$tmp/suggest-heredoc.txt"
[[ ! -s "$tmp/suggest-heredoc.txt" ]] || fail "heredoc body should suppress suggestions"

arithmetic_buffer='echo $((1 + 2'
./target/release/termsense suggest "$arithmetic_buffer" --limit 20 > "$tmp/suggest-arithmetic.txt"
[[ ! -s "$tmp/suggest-arithmetic.txt" ]] || fail "open arithmetic expansion should suppress suggestions"

printf '==> install/uninstall lifecycle\n'
mkdir -p "$tmp/home"

HOME="$tmp/home" \
XDG_CONFIG_HOME="$tmp/home/.config" \
XDG_CACHE_HOME="$tmp/home/.cache" \
XDG_STATE_HOME="$tmp/home/.local/state" \
TERMSENSE_SKIP_BUILD=1 \
bash scripts/install.sh \
  --prefix "$tmp/prefix" \
  --bashrc "$tmp/bashrc" >/dev/null

[[ -x "$tmp/prefix/bin/termsense" ]] || fail "installer did not install binary"
[[ -f "$tmp/home/.config/termsense/config.conf" ]] || fail "installer did not create config"
grep -q '^# >>> termsense >>>$' "$tmp/bashrc" || fail "installer did not add managed Bash block"
grep -q '^TERMSENSE_BIN=' "$tmp/bashrc" || fail "installer did not pin installed binary"

installed_lookup="$(
  HOME="$tmp/home" \
  XDG_CONFIG_HOME="$tmp/home/.config" \
  PATH="/usr/bin:/bin" \
  bash --noprofile --norc -ic "source '$tmp/bashrc'; _termsense_binary" 2>/dev/null
)"
[[ "$installed_lookup" == "$tmp/prefix/bin/termsense" ]] \
  || fail "user-local binary lookup fails when prefix/bin is outside PATH"

HOME="$tmp/home" \
XDG_CONFIG_HOME="$tmp/home/.config" \
XDG_CACHE_HOME="$tmp/home/.cache" \
XDG_STATE_HOME="$tmp/home/.local/state" \
bash scripts/uninstall.sh \
  --prefix "$tmp/prefix" \
  --bashrc "$tmp/bashrc" \
  --purge >/dev/null

[[ ! -e "$tmp/prefix/bin/termsense" ]] || fail "uninstaller did not remove binary"
! grep -q '^# >>> termsense >>>$' "$tmp/bashrc" || fail "uninstaller left managed Bash block"
[[ ! -d "$tmp/home/.config/termsense" ]] || fail "purge left config directory"

printf '==> Debian package\n'
TERMSENSE_SKIP_BUILD=1 bash scripts/package-deb.sh --output "$tmp/dist" >/dev/null

deb_file="$(find "$tmp/dist" -maxdepth 1 -type f -name "termsense_${version}_*.deb" -print -quit)"
[[ -n "$deb_file" ]] || fail "Debian package was not created"

[[ "$(dpkg-deb -f "$deb_file" Package)" == "termsense" ]] \
  || fail "Debian package name mismatch"
[[ "$(dpkg-deb -f "$deb_file" Version)" == "$version" ]] \
  || fail "Debian package version mismatch"
dpkg-deb -c "$deb_file" | grep -q './usr/share/termsense/default.conf' \
  || fail "Debian package is missing reference default config"
dpkg-deb -c "$deb_file" | grep -q './usr/share/doc/termsense/CHANGELOG.md' \
  || fail "Debian package is missing changelog"

printf 'TermSense release readiness: PASS\n'
printf 'version: %s\n' "$version"
printf 'binary: %s\n' "$repo_root/target/release/termsense"
printf 'deb: %s\n' "$deb_file"
 config/default.conf || fail "default config missing max_visible"
grep -q '^ghost=1$' config/default.conf || fail "default config missing ghost"
grep -q '^ctrl_space=1$' config/default.conf || fail "default config missing ctrl_space"
grep -q 'repository = "https://github.com/Anuppaul/termsense"' Cargo.toml \
  || fail "Cargo repository identity mismatch"

if [[ -d .github/workflows ]] && find .github/workflows -type f -print -quit | grep -q .; then
  fail "GitHub Actions workflows are present; project policy currently forbids CI workflows"
fi

printf '==> source integrity\n'
[[ "$(grep -c '^_termsense_query() {' shell/termsense.bash)" -eq 1 ]] \
  || fail "Bash adapter must contain exactly one _termsense_query function"
[[ "$(grep -c '^_termsense_ctrl_space() {' shell/termsense.bash)" -eq 1 ]] \
  || fail "Bash adapter must contain exactly one _termsense_ctrl_space function"
[[ "$(grep -c '^_termsense_draw_overlay() {' shell/termsense.bash)" -eq 1 ]] \
  || fail "Bash adapter must contain exactly one _termsense_draw_overlay function"
[[ "$(grep -c '^pub(crate) fn active_context' src/shell_parse.rs)" -eq 1 ]] \
  || fail "shell parser must contain exactly one active_context function"
[[ "$(grep -c '^fn lex_range' src/shell_parse.rs)" -eq 1 ]] \
  || fail "shell parser must contain exactly one lex_range function"
[[ "$(grep -c '^fn inside_open_heredoc' src/shell_parse.rs)" -eq 1 ]] \
  || fail "shell parser must contain exactly one heredoc detector"

printf '==> git diff check\n'
git diff --check

printf '==> rustfmt\n'
cargo fmt --check

printf '==> tests\n'
cargo test

printf '==> release build\n'
cargo build --release

[[ -s Cargo.lock ]] || fail "Cargo.lock was not generated"
git ls-files --error-unmatch Cargo.lock >/dev/null 2>&1 \
  || fail "Cargo.lock exists but is not committed; commit the generated lockfile before release"
git diff --quiet -- Cargo.lock \
  || fail "Cargo.lock changed during build; commit the updated lockfile before release"

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
[[ "$(grep -c '^_termsense_query() {' "$tmp/termsense-init.bash")" -eq 1 ]] \
  || fail "generated Bash integration contains duplicate query functions"

printf '==> binary smoke\n'
./target/release/termsense status > "$tmp/status.txt"
grep -q '^platform: linux$' "$tmp/status.txt" || fail "status platform smoke failed"
grep -q '^network required: no$' "$tmp/status.txt" || fail "offline status smoke failed"
grep -q '^config file: ' "$tmp/status.txt" || fail "config status smoke failed"

./target/release/termsense suggest d --limit 5 > "$tmp/suggest-command.txt"
[[ -s "$tmp/suggest-command.txt" ]] || fail "command suggestion smoke returned no candidates"

./target/release/termsense suggest "" --limit 0 > "$tmp/suggest-all.txt"
[[ -s "$tmp/suggest-all.txt" ]] || fail "unbounded command discovery smoke returned no candidates"

./target/release/termsense suggest "sudo git che" --limit 20 > "$tmp/suggest-context.txt"
grep -Fq "sudo git checkout" "$tmp/suggest-context.txt" \
  || fail "sudo git contextual completion smoke failed"

./target/release/termsense suggest "git status && docker lo" --limit 20 > "$tmp/suggest-segment.txt"
grep -Fq "git status && docker logs" "$tmp/suggest-segment.txt" \
  || fail "active command segment smoke failed"

./target/release/termsense suggest 'echo $(git che' --limit 20 > "$tmp/suggest-substitution.txt"
grep -Fq 'echo $(git checkout' "$tmp/suggest-substitution.txt" \
  || fail "command substitution smoke failed"

./target/release/termsense suggest 'diff <(git che' --limit 20 > "$tmp/suggest-process-sub.txt"
grep -Fq 'diff <(git checkout' "$tmp/suggest-process-sub.txt" \
  || fail "process substitution smoke failed"

./target/release/termsense suggest '( git che' --limit 20 > "$tmp/suggest-group.txt"
grep -Fq '( git checkout' "$tmp/suggest-group.txt" \
  || fail "command group smoke failed"

heredoc_buffer="$(printf 'cat <<EOF\nhello wor')"
./target/release/termsense suggest "$heredoc_buffer" --limit 20 > "$tmp/suggest-heredoc.txt"
[[ ! -s "$tmp/suggest-heredoc.txt" ]] || fail "heredoc body should suppress suggestions"

arithmetic_buffer='echo $((1 + 2'
./target/release/termsense suggest "$arithmetic_buffer" --limit 20 > "$tmp/suggest-arithmetic.txt"
[[ ! -s "$tmp/suggest-arithmetic.txt" ]] || fail "open arithmetic expansion should suppress suggestions"

printf '==> install/uninstall lifecycle\n'
mkdir -p "$tmp/home"

HOME="$tmp/home" \
XDG_CONFIG_HOME="$tmp/home/.config" \
XDG_CACHE_HOME="$tmp/home/.cache" \
XDG_STATE_HOME="$tmp/home/.local/state" \
TERMSENSE_SKIP_BUILD=1 \
bash scripts/install.sh \
  --prefix "$tmp/prefix" \
  --bashrc "$tmp/bashrc" >/dev/null

[[ -x "$tmp/prefix/bin/termsense" ]] || fail "installer did not install binary"
[[ -f "$tmp/home/.config/termsense/config.conf" ]] || fail "installer did not create config"
grep -q '^# >>> termsense >>>$' "$tmp/bashrc" || fail "installer did not add managed Bash block"
grep -q '^TERMSENSE_BIN=' "$tmp/bashrc" || fail "installer did not pin installed binary"

installed_lookup="$(
  HOME="$tmp/home" \
  XDG_CONFIG_HOME="$tmp/home/.config" \
  PATH="/usr/bin:/bin" \
  bash --noprofile --norc -ic "source '$tmp/bashrc'; _termsense_binary" 2>/dev/null
)"
[[ "$installed_lookup" == "$tmp/prefix/bin/termsense" ]] \
  || fail "user-local binary lookup fails when prefix/bin is outside PATH"

HOME="$tmp/home" \
XDG_CONFIG_HOME="$tmp/home/.config" \
XDG_CACHE_HOME="$tmp/home/.cache" \
XDG_STATE_HOME="$tmp/home/.local/state" \
bash scripts/uninstall.sh \
  --prefix "$tmp/prefix" \
  --bashrc "$tmp/bashrc" \
  --purge >/dev/null

[[ ! -e "$tmp/prefix/bin/termsense" ]] || fail "uninstaller did not remove binary"
! grep -q '^# >>> termsense >>>$' "$tmp/bashrc" || fail "uninstaller left managed Bash block"
[[ ! -d "$tmp/home/.config/termsense" ]] || fail "purge left config directory"

printf '==> Debian package\n'
TERMSENSE_SKIP_BUILD=1 bash scripts/package-deb.sh --output "$tmp/dist" >/dev/null

deb_file="$(find "$tmp/dist" -maxdepth 1 -type f -name "termsense_${version}_*.deb" -print -quit)"
[[ -n "$deb_file" ]] || fail "Debian package was not created"

[[ "$(dpkg-deb -f "$deb_file" Package)" == "termsense" ]] \
  || fail "Debian package name mismatch"
[[ "$(dpkg-deb -f "$deb_file" Version)" == "$version" ]] \
  || fail "Debian package version mismatch"
dpkg-deb -c "$deb_file" | grep -q './usr/share/termsense/default.conf' \
  || fail "Debian package is missing reference default config"
dpkg-deb -c "$deb_file" | grep -q './usr/share/doc/termsense/CHANGELOG.md' \
  || fail "Debian package is missing changelog"

printf 'TermSense release readiness: PASS\n'
printf 'version: %s\n' "$version"
printf 'binary: %s\n' "$repo_root/target/release/termsense"
printf 'deb: %s\n' "$deb_file"
