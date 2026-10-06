#!/usr/bin/env bash
set -euo pipefail

prefix="${TERMSENSE_PREFIX:-$HOME/.local}"
bashrc="${TERMSENSE_BASHRC:-$HOME/.bashrc}"
purge=0

usage() {
  cat <<'EOF'
Usage: scripts/uninstall.sh [options]

Options:
  --prefix PATH   Installed prefix (default: ~/.local)
  --bashrc PATH   Bash rc file to clean (default: ~/.bashrc)
  --purge         Also remove TermSense cache/state/config directories
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
    --purge)
      purge=1
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

binary="$prefix/bin/termsense"
rm -f -- "$binary"

if [[ -f "$bashrc" ]]; then
  temp="$(mktemp)"
  awk '
    BEGIN { managed = 0 }
    /^# >>> termsense >>>$/ { managed = 1; next }
    /^# <<< termsense <<</ { managed = 0; next }
    managed == 0 { print }
  ' "$bashrc" > "$temp"
  mv "$temp" "$bashrc"
fi

if (( purge )); then
  cache_root="${XDG_CACHE_HOME:-$HOME/.cache}"
  state_root="${XDG_STATE_HOME:-$HOME/.local/state}"
  config_root="${XDG_CONFIG_HOME:-$HOME/.config}"
  rm -rf -- "$cache_root/termsense" "$state_root/termsense" "$config_root/termsense"
fi

printf 'TermSense binary removed: %s\n' "$binary"
printf 'TermSense Bash managed block removed from: %s\n' "$bashrc"
if (( purge )); then
  printf 'TermSense cache/state/config purged.\n'
fi
