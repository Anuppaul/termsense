# TermSense Bash integration
#
# Enable with:
#   eval "$(termsense init bash)"
#
# This first integration slice provides a VS Code-style Ctrl+Space command
# browser and insertion. Live per-keystroke rendering is the next renderer
# slice; the suggestion engine API used here is already shared with it.

if [[ -n "${__TERMSENSE_BASH_LOADED:-}" ]]; then
  return 0 2>/dev/null || true
fi
__TERMSENSE_BASH_LOADED=1

_termsense_binary() {
  command -v termsense 2>/dev/null
}

_termsense_current_prefix() {
  local left="${READLINE_LINE:0:READLINE_POINT}"
  if [[ "$left" =~ ^[[:space:]]*([^[:space:]]*)$ ]]; then
    printf '%s' "${BASH_REMATCH[1]}"
  else
    printf ''
  fi
}

_termsense_ctrl_space() {
  local ts
  ts="$(_termsense_binary)" || {
    printf '\nTermSense binary not found in PATH.\n' >&2
    return 1
  }

  local prefix
  prefix="$(_termsense_current_prefix)"

  local -a candidates=()
  while IFS=$'\t' read -r value _kind _source _score; do
    [[ -n "$value" ]] && candidates+=("$value")
  done < <("$ts" suggest "$prefix" --limit 200 2>/dev/null)

  if (("${#candidates[@]}" == 0)); then
    return 0
  fi

  local selected
  if command -v fzf >/dev/null 2>&1; then
    selected="$(printf '%s\n' "${candidates[@]}" | fzf       --height=40%       --reverse       --prompt='TermSense > '       --query="$prefix"       --select-1       --exit-0)"
  else
    # Dependency-free fallback: show a numbered candidate browser.
    printf '\n' >&2
    local i max=20
    (("${#candidates[@]}" < max)) && max="${#candidates[@]}"
    for ((i=0; i<max; i++)); do
      printf '%2d  %s\n' "$((i+1))" "${candidates[i]}" >&2
    done
    printf 'TermSense choice [1-%d, Enter to cancel]: ' "$max" >&2
    local choice
    IFS= read -r choice
    if [[ "$choice" =~ ^[0-9]+$ ]] && ((choice >= 1 && choice <= max)); then
      selected="${candidates[choice-1]}"
    fi
  fi

  [[ -z "$selected" ]] && return 0

  local left="${READLINE_LINE:0:READLINE_POINT}"
  local right="${READLINE_LINE:READLINE_POINT}"

  if [[ "$left" =~ ^([[:space:]]*)([^[:space:]]*)$ ]]; then
    local leading="${BASH_REMATCH[1]}"
    READLINE_LINE="${leading}${selected}${right}"
    READLINE_POINT=$(("${#leading}" + "${#selected}"))
  fi
}

bind -x '"\C- ":_termsense_ctrl_space'
