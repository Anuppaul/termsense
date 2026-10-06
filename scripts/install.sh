#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
prefix="${TERMSENSE_PREFIX:-$HOME/.local}"
bashrc="${TERMSENSE_BASHRC:-$HOME/.bashrc}"
enable_shell=1
skip_build="${TERMSENSE_SKIP_BUILD:-0}"

usage() {
  cat <<'EOF'
Usage: scripts/install.sh [options]

Options:
  --prefix PATH   Install under PATH (default: ~/.local)
  --bashrc PATH   Bash rc file to manage (default: ~/.bashrc)
  --no-shell      Install binary only; do not edit Bash rc
  --skip-build    Reuse target/release/termsense instead of running Cargo
  -h, --help      Show this help
EOF
}

while (($#)); do
  case "$1" in
    --prefix)
      [[ $# -ge 2 ]] || { printf 'missing value for --prefix\n' >&2; exit 2; }
      prefix="$2"
      shift 2
      ;;
    --bashrc)
      [[ $# -ge 2 ]] || { printf 'missing value for --bashrc\n' >&2; exit 2; }
      bashrc="$2"
      shift 2
      ;;
    --no-shell)
      enable_shell=0
      shift
      ;;
    --skip-build)
      skip_build=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      printf 'unknown option: %s\n' "$1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

[[ "$(uname -s)" == "Linux" ]] || {
  printf 'TermSense is Linux-only.\n' >&2
  exit 1
}

cd "$repo_root"

if [[ "$skip_build" == "1" ]]; then
  [[ -x target/release/termsense ]] || {
    printf 'target/release/termsense not found; remove --skip-build or build first.\n' >&2
    exit 1
  }
else
  command -v cargo >/dev/null 2>&1 || {
    printf 'cargo is required to build TermSense from source.\n' >&2
    exit 1
  }
  cargo build --release
fi

bin_dir="$prefix/bin"
binary="$bin_dir/termsense"
mkdir -p "$bin_dir"
install -m 0755 target/release/termsense "$binary"

if (( enable_shell )); then
  mkdir -p "$(dirname -- "$bashrc")"
  touch "$bashrc"

  temp="$(mktemp)"
  awk '
    BEGIN { managed = 0 }
    /^# >>> termsense >>>$/ { managed = 1; next }
    /^# <<< termsense <<</ { managed = 0; next }
    managed == 0 { print }
  ' "$bashrc" > "$temp"

  cat >> "$temp" <<EOF

# >>> termsense >>>
if [[ -x "$binary" ]]; then
  eval "\$("$binary" init bash)"
fi
# <<< termsense <<<
EOF

  mv "$temp" "$bashrc"
fi

"$binary" index >/dev/null 2>&1 || true

printf 'TermSense installed: %s\n' "$binary"
if (( enable_shell )); then
  printf 'Bash integration managed in: %s\n' "$bashrc"
  printf 'Start a new Bash shell or run: source %q\n' "$bashrc"
fi
