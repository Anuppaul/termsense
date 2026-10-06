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

if (( enable_shell && BASH_VERSINFO[0] < 5 )); then
  printf 'TermSense Bash integration requires Bash 5.0 or newer.\n' >&2
  exit 1
fi

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

config_root="${XDG_CONFIG_HOME:-$HOME/.config}/termsense"
config_file="$config_root/config.conf"
mkdir -p "$config_root"
if [[ ! -e "$config_file" ]]; then
  install -m 0644 config/default.conf "$config_file"
fi

if (( enable_shell )); then
  mkdir -p "$(dirname -- "$bashrc")"
  touch "$bashrc"

  temp="$(mktemp)"
  trap 'rm -f -- "$temp"' EXIT
  awk '
    BEGIN { managed = 0 }
    /^# >>> termsense >>>$/ { managed = 1; next }
    /^# <<< termsense <<</ { managed = 0; next }
    managed == 0 { print }
  ' "$bashrc" > "$temp"

  printf -v binary_q '%q' "$binary"

  cat >> "$temp" <<EOF

# >>> termsense >>>
TERMSENSE_BIN=$binary_q
if [[ -x "\$TERMSENSE_BIN" ]]; then
  eval "\$("\$TERMSENSE_BIN" init bash)"
fi
# <<< termsense <<<
EOF

  chmod --reference="$bashrc" "$temp" 2>/dev/null || true
  mv "$temp" "$bashrc"
  trap - EXIT
fi

"$binary" index >/dev/null 2>&1 || true

_termsense_install_welcome() {
  local version command_count
  version="$("$binary" --version 2>/dev/null | awk '{print $2}')"
  command_count="$("$binary" status 2>/dev/null | awk -F': ' '/^commands indexed:/ {print $2; exit}')"

  local bold="" dim="" cyan="" green="" reset=""
  if [[ -t 1 && -z "${NO_COLOR:-}" ]]; then
    printf -v bold '\033[1m'
    printf -v dim '\033[2m'
    printf -v cyan '\033[36m'
    printf -v green '\033[32m'
    printf -v reset '\033[0m'
  fi

  printf '\n'
  printf '%s' "$cyan$bold"
  cat <<'EOF'
 _____                   _____
|_   _|__ _ __ _ __ ___/ ___/  ___ _ __  ___  ___
  | |/ _ \ '__| '_ ` _ \___ \ / _ \ '_ \/ __|/ _ \
  | |  __/ |  | | | | | |__) |  __/ | | \__ \  __/
  |_|\___|_|  |_| |_| |_|____/ \___|_| |_|___/\___|
EOF
  printf '%s' "$reset"
  printf '\n'
  printf '  %sTerminal intelligence for Bash%s\n' "$bold" "$reset"
  printf '  %s✓%s TermSense %s installed\n' "$green" "$reset" "${version:-0.1.0}"
  if [[ -n "$command_count" ]]; then
    printf '  %s✓%s %s commands indexed\n' "$green" "$reset" "$command_count"
  fi
  if (( enable_shell )); then
    printf '  %s✓%s Bash integration enabled\n' "$green" "$reset"
  fi
  printf '\n'
  printf '  %sStart typing%s      live suggestions\n' "$bold" "$reset"
  printf '  %sCtrl+Space%s        browse all commands\n' "$bold" "$reset"
  printf '  %sTab / Enter / →%s   accept suggestion\n' "$bold" "$reset"
  printf '\n'
  if (( enable_shell )); then
    printf '  %sOpen a new Bash shell or run:%s source %q\n' "$dim" "$reset" "$bashrc"
  fi
  printf '\n'
}

_termsense_install_welcome

printf '%s\n' "Installed binary: $binary"
printf '%s\n' "Config: $config_file"
