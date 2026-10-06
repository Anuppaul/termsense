#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
output_dir="${TERMSENSE_DIST_DIR:-$repo_root/dist}"
skip_build="${TERMSENSE_SKIP_BUILD:-0}"
arch_override=""

usage() {
  cat <<'EOF'
Usage: scripts/package-deb.sh [options]

Options:
  --output DIR    Output directory (default: ./dist)
  --arch ARCH     Debian architecture override
  --skip-build    Reuse target/release/termsense
  -h, --help      Show this help
EOF
}

while (($#)); do
  case "$1" in
    --output)
      [[ $# -ge 2 ]] || { printf 'missing value for --output\n' >&2; exit 2; }
      output_dir="$2"
      shift 2
      ;;
    --arch)
      [[ $# -ge 2 ]] || { printf 'missing value for --arch\n' >&2; exit 2; }
      arch_override="$2"
      shift 2
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
  printf 'TermSense packaging requires Linux.\n' >&2
  exit 1
}

command -v dpkg-deb >/dev/null 2>&1 || {
  printf 'dpkg-deb is required to build a Debian package.\n' >&2
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
    printf 'cargo is required to build TermSense.\n' >&2
    exit 1
  }
  cargo build --release
fi

version="$(
  awk '
    /^\[package\]$/ { in_package = 1; next }
    /^\[/ && $0 != "[package]" { in_package = 0 }
    in_package && $1 == "version" {
      gsub(/"/, "", $3)
      print $3
      exit
    }
  ' Cargo.toml
)"

[[ -n "$version" ]] || {
  printf 'could not read package version from Cargo.toml\n' >&2
  exit 1
}

if [[ -n "$arch_override" ]]; then
  deb_arch="$arch_override"
else
  case "$(uname -m)" in
    x86_64) deb_arch="amd64" ;;
    aarch64|arm64) deb_arch="arm64" ;;
    *)
      printf 'unsupported architecture %s; pass --arch explicitly.\n' "$(uname -m)" >&2
      exit 1
      ;;
  esac
fi

stage="$(mktemp -d)"
trap 'rm -rf -- "$stage"' EXIT

mkdir -p   "$stage/DEBIAN"   "$stage/usr/bin"   "$stage/usr/share/doc/termsense" \
  "$stage/usr/share/termsense"

install -m 0755 target/release/termsense "$stage/usr/bin/termsense"
install -m 0644 README.md "$stage/usr/share/doc/termsense/README.md"
install -m 0644 CHANGELOG.md "$stage/usr/share/doc/termsense/CHANGELOG.md"
install -m 0644 LICENSE "$stage/usr/share/doc/termsense/copyright"
install -m 0644 config/default.conf "$stage/usr/share/termsense/default.conf"

cat > "$stage/DEBIAN/control" <<EOF
Package: termsense
Version: $version
Section: utils
Priority: optional
Architecture: $deb_arch
Maintainer: TermSense Contributors
Depends: bash
Description: Context-aware inline command suggestions for Linux terminals
 TermSense provides local, context-aware terminal suggestions and Bash
 integration while keeping command execution under the user's control.
Homepage: https://github.com/Anuppaul/termsense
EOF

mkdir -p "$output_dir"
artifact="$output_dir/termsense_${version}_${deb_arch}.deb"
dpkg-deb --build --root-owner-group "$stage" "$artifact" >/dev/null

printf 'Debian package created: %s\n' "$artifact"
printf 'Install with: sudo apt install %q\n' "$artifact"
printf 'Then enable Bash for your user with: eval "$(termsense init bash)"\n'
