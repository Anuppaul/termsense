# TermSense Bash integration
# Enable with: eval "$(termsense init bash)"

[[ $- == *i* ]] || return 0 2>/dev/null || true

if [[ -n "${__TERMSENSE_BASH_LOADED:-}" ]]; then
  return 0 2>/dev/null || true
fi
__TERMSENSE_BASH_LOADED=1

: "${TERMSENSE_AUTO_SUGGEST:=1}"
: "${TERMSENSE_MAX_VISIBLE:=5}"
: "${TERMSENSE_GHOST:=1}"

__TERMSENSE_VISIBLE=0
__TERMSENSE_GHOST_LEN=0
__TERMSENSE_SELECTED=0
__TERMSENSE_REPLACE_START=0
__TERMSENSE_REPLACE_END=0
__TERMSENSE_PREFIX=""
__TERMSENSE_CANDIDATES=()
__TERMSENSE_DISPLAYS=()
__TERMSENSE_USAGE_KEYS=()

_termsense_binary() {
  command -v termsense 2>/dev/null
}

_termsense_refresh_shell_context() {
  local -a values=()
  local -a filtered=()
  local name

  TERMSENSE_SHELL_BUILTINS=""
  mapfile -t values < <(compgen -b 2>/dev/null)
  if (("${#values[@]}" > 0)); then
    printf -v TERMSENSE_SHELL_BUILTINS '%s\n' "${values[@]}"
  fi
  export TERMSENSE_SHELL_BUILTINS

  TERMSENSE_SHELL_ALIASES=""
  mapfile -t values < <(compgen -A alias 2>/dev/null)
  if (("${#values[@]}" > 0)); then
    printf -v TERMSENSE_SHELL_ALIASES '%s\n' "${values[@]}"
  fi
  export TERMSENSE_SHELL_ALIASES

  TERMSENSE_SHELL_FUNCTIONS=""
  filtered=()
  mapfile -t values < <(compgen -A function 2>/dev/null)
  for name in "${values[@]}"; do
    [[ "$name" == _termsense_* ]] || filtered+=("$name")
  done
  if (("${#filtered[@]}" > 0)); then
    printf -v TERMSENSE_SHELL_FUNCTIONS '%s\n' "${filtered[@]}"
  fi
  export TERMSENSE_SHELL_FUNCTIONS
}

_termsense_record_usage() {
  local usage_key="$1"
  [[ -n "$usage_key" ]] || return 0

  local ts
  ts="$(_termsense_binary)" || return 0
  "$ts" record "$usage_key" >/dev/null 2>&1 || true
}

_termsense_clear_overlay() {
  local old_visible=${__TERMSENSE_VISIBLE:-0}
  local old_ghost=${__TERMSENSE_GHOST_LEN:-0}
  (( old_visible == 0 && old_ghost == 0 )) && return 0

  printf '\033[s' >&2
  if (( old_ghost > 0 )); then
    printf '%*s' "$old_ghost" '' >&2
  fi

  local i
  for ((i=0; i<old_visible; i++)); do
    printf '\033[1B\r\033[2K' >&2
  done
  printf '\033[u' >&2

  __TERMSENSE_VISIBLE=0
  __TERMSENSE_GHOST_LEN=0
}

_termsense_restore_navigation() {
  bind '"\e[A": previous-history' 2>/dev/null || true
  bind '"\e[B": next-history' 2>/dev/null || true
  bind '"\e[C": forward-char' 2>/dev/null || true
  bind '"\e[D": backward-char' 2>/dev/null || true
  bind '"\C-i": complete' 2>/dev/null || true
  bind '"\e": prefix-meta' 2>/dev/null || true
}

_termsense_activate_navigation() {
  bind -x '"\e[A":_termsense_select_prev' 2>/dev/null || true
  bind -x '"\e[B":_termsense_select_next' 2>/dev/null || true
  bind -x '"\e[C":_termsense_accept_ghost' 2>/dev/null || true
  bind -x '"\e[D":_termsense_move_left' 2>/dev/null || true
  bind -x '"\C-i":_termsense_accept_selected' 2>/dev/null || true
  bind -x '"\e":_termsense_dismiss' 2>/dev/null || true
}

_termsense_reset_state() {
  __TERMSENSE_CANDIDATES=()
  __TERMSENSE_DISPLAYS=()
  __TERMSENSE_USAGE_KEYS=()
  __TERMSENSE_SELECTED=0
  __TERMSENSE_REPLACE_START=$READLINE_POINT
  __TERMSENSE_REPLACE_END=$READLINE_POINT
  __TERMSENSE_PREFIX=""
}

_termsense_dismiss() {
  _termsense_clear_overlay
  _termsense_reset_state
  _termsense_restore_navigation
}

_termsense_query() {
  local ts
  ts="$(_termsense_binary)" || return 1

  _termsense_reset_state
  local value display _kind _source _score start end usage_key first=1

  while IFS=$'\t' read -r value display _kind _source _score start end usage_key; do
    [[ -n "$value" ]] || continue
    __TERMSENSE_CANDIDATES+=("$value")
    __TERMSENSE_DISPLAYS+=("$display")
    __TERMSENSE_USAGE_KEYS+=("$usage_key")

    if (( first )); then
      __TERMSENSE_REPLACE_START=$start
      __TERMSENSE_REPLACE_END=$end
      __TERMSENSE_PREFIX="${READLINE_LINE:start:end-start}"
      first=0
    fi
  done < <("$ts" suggest "$READLINE_LINE" --cursor "$READLINE_POINT" --limit 24 2>/dev/null)

  (("${#__TERMSENSE_CANDIDATES[@]}" > 0))
}

_termsense_draw_overlay() {
  _termsense_clear_overlay

  local total=${#__TERMSENSE_CANDIDATES[@]}
  (( total > 0 )) || {
    _termsense_restore_navigation
    return 0
  }

  (( __TERMSENSE_SELECTED >= total )) && __TERMSENSE_SELECTED=$((total - 1))
  (( __TERMSENSE_SELECTED < 0 )) && __TERMSENSE_SELECTED=0

  local selected="${__TERMSENSE_CANDIDATES[__TERMSENSE_SELECTED]}"
  local suffix=""
  if [[ "$TERMSENSE_GHOST" != "0"         && "$READLINE_POINT" -eq "${#READLINE_LINE}"         && "$selected" == "$__TERMSENSE_PREFIX"* ]]; then
    suffix="${selected:${#__TERMSENSE_PREFIX}}"
  fi

  local max=$TERMSENSE_MAX_VISIBLE
  [[ "$max" =~ ^[0-9]+$ ]] || max=5
  (( max < 1 )) && max=1
  (( max > total )) && max=$total

  local columns=${COLUMNS:-80}
  (( columns < 20 )) && columns=20
  local width=$((columns - 6))

  printf '\033[s' >&2
  if [[ -n "$suffix" ]]; then
    printf '\033[2m%s\033[0m' "$suffix" >&2
    __TERMSENSE_GHOST_LEN=${#suffix}
  fi

  local window_start=0
  if (( __TERMSENSE_SELECTED >= max )); then
    window_start=$((__TERMSENSE_SELECTED - max + 1))
  fi

  local i index label
  for ((i=0; i<max; i++)); do
    index=$((window_start + i))
    (( index < total )) || break
    label="${__TERMSENSE_DISPLAYS[index]}"
    (("${#label}" > width)) && label="${label:0:width}"

    printf '\033[1B\r\033[2K' >&2
    if (( index == __TERMSENSE_SELECTED )); then
      printf '  \033[7m> %-*s\033[0m' "$width" "$label" >&2
    else
      printf '    \033[2m%-*s\033[0m' "$width" "$label" >&2
    fi
  done
  printf '\033[u' >&2

  __TERMSENSE_VISIBLE=$max
  _termsense_activate_navigation
}

_termsense_refresh() {
  [[ "$TERMSENSE_AUTO_SUGGEST" != "0" ]] || return 0

  local left="${READLINE_LINE:0:READLINE_POINT}"
  if [[ -z "${left//[[:space:]]/}" ]]; then
    _termsense_dismiss
    return 0
  fi

  __TERMSENSE_SELECTED=0
  if _termsense_query; then
    _termsense_draw_overlay
  else
    _termsense_dismiss
  fi
}

_termsense_accept_selected() {
  local total=${#__TERMSENSE_CANDIDATES[@]}
  (( total > 0 )) || return 0

  local selected="${__TERMSENSE_CANDIDATES[__TERMSENSE_SELECTED]}"
  local usage_key="${__TERMSENSE_USAGE_KEYS[__TERMSENSE_SELECTED]}"
  local left="${READLINE_LINE:0:__TERMSENSE_REPLACE_START}"
  local right="${READLINE_LINE:__TERMSENSE_REPLACE_END}"

  READLINE_LINE="${left}${selected}${right}"
  READLINE_POINT=$((__TERMSENSE_REPLACE_START + ${#selected}))
  _termsense_record_usage "$usage_key"
  _termsense_dismiss
}

_termsense_accept_ghost() {
  if (("${#__TERMSENSE_CANDIDATES[@]}" > 0)); then
    _termsense_accept_selected
  else
    (( READLINE_POINT < ${#READLINE_LINE} )) && READLINE_POINT=$((READLINE_POINT + 1))
  fi
}

_termsense_select_prev() {
  local total=${#__TERMSENSE_CANDIDATES[@]}
  (( total > 0 )) || return 0

  if (( __TERMSENSE_SELECTED == 0 )); then
    __TERMSENSE_SELECTED=$((total - 1))
  else
    __TERMSENSE_SELECTED=$((__TERMSENSE_SELECTED - 1))
  fi
  _termsense_draw_overlay
}

_termsense_select_next() {
  local total=${#__TERMSENSE_CANDIDATES[@]}
  (( total > 0 )) || return 0

  __TERMSENSE_SELECTED=$(((__TERMSENSE_SELECTED + 1) % total))
  _termsense_draw_overlay
}

_termsense_move_left() {
  (( READLINE_POINT > 0 )) && READLINE_POINT=$((READLINE_POINT - 1))
  _termsense_refresh
}

_termsense_ctrl_space() {
  local ts
  ts="$(_termsense_binary)" || {
    _termsense_dismiss
    printf '\nTermSense binary not found in PATH.\n' >&2
    return 1
  }

  _termsense_clear_overlay

  local -a candidates=()
  local -a displays=()
  local -a usage_keys=()
  local value display _kind _source _score start end usage_key
  local replace_start=$READLINE_POINT
  local replace_end=$READLINE_POINT
  local prefix=""
  local first=1

  while IFS=$'\t' read -r value display _kind _source _score start end usage_key; do
    [[ -n "$value" ]] || continue
    candidates+=("$value")
    displays+=("$display")
    usage_keys+=("$usage_key")

    if (( first )); then
      replace_start=$start
      replace_end=$end
      prefix="${READLINE_LINE:start:end-start}"
      first=0
    fi
  done < <("$ts" suggest "$READLINE_LINE" --cursor "$READLINE_POINT" --limit 500 2>/dev/null)

  (("${#candidates[@]}" > 0)) || {
    _termsense_reset_state
    _termsense_restore_navigation
    return 0
  }

  local selected=""
  local selected_usage_key=""
  local i

  if command -v fzf >/dev/null 2>&1; then
    local selected_row selected_index
    selected_row="$(
      for ((i=0; i<${#candidates[@]}; i++)); do
        printf '%d\t%s\n' "$i" "${displays[i]}"
      done | fzf         --height=40%         --reverse         --delimiter=$'\t'         --with-nth=2..         --prompt='TermSense > '         --query="$prefix"         --select-1         --exit-0
    )"

    selected_index="${selected_row%%$'\t'*}"
    if [[ "$selected_index" =~ ^[0-9]+$ ]] && ((selected_index < ${#candidates[@]})); then
      selected="${candidates[selected_index]}"
      selected_usage_key="${usage_keys[selected_index]}"
    fi
  else
    printf '\n' >&2
    local max=20
    (("${#candidates[@]}" < max)) && max=${#candidates[@]}

    for ((i=0; i<max; i++)); do
      printf '%2d  %s\n' "$((i + 1))" "${displays[i]}" >&2
    done
    printf 'TermSense choice [1-%d, Enter to cancel]: ' "$max" >&2

    local choice
    IFS= read -r choice
    if [[ "$choice" =~ ^[0-9]+$ ]] && ((choice >= 1 && choice <= max)); then
      selected="${candidates[choice-1]}"
      selected_usage_key="${usage_keys[choice-1]}"
    fi
  fi

  if [[ -n "$selected" ]]; then
    local left="${READLINE_LINE:0:replace_start}"
    local right="${READLINE_LINE:replace_end}"
    READLINE_LINE="${left}${selected}${right}"
    READLINE_POINT=$((replace_start + ${#selected}))
    _termsense_record_usage "$selected_usage_key"
  fi

  _termsense_reset_state
  _termsense_restore_navigation
}

_termsense_prompt_cleanup() {
  _termsense_clear_overlay
  _termsense_reset_state
  _termsense_restore_navigation
  _termsense_refresh_shell_context
}

_termsense_refresh_shell_context

bind -x '"\C-x\C-t":_termsense_refresh'

for __termsense_code in $(seq 32 126); do
  printf -v __termsense_hex '%02x' "$__termsense_code"
  bind "\"\\x${__termsense_hex}\": \"\\C-v\\x${__termsense_hex}\\C-x\\C-t\"" 2>/dev/null || true
done
unset __termsense_code __termsense_hex

bind '"\C-x\C-b": backward-delete-char'
bind '"\C-h": "\C-x\C-b\C-x\C-t"'
bind '"\C-?": "\C-x\C-b\C-x\C-t"'

bind -x '"\C- ":_termsense_ctrl_space'
bind -x '"\C-@":_termsense_ctrl_space'

if declare -p PROMPT_COMMAND 2>/dev/null | grep -q '^declare -a'; then
  __termsense_prompt_found=0
  for __termsense_prompt_item in "${PROMPT_COMMAND[@]}"; do
    [[ "$__termsense_prompt_item" == "_termsense_prompt_cleanup" ]] && __termsense_prompt_found=1
  done
  if (( __termsense_prompt_found == 0 )); then
    PROMPT_COMMAND=("_termsense_prompt_cleanup" "${PROMPT_COMMAND[@]}")
  fi
  unset __termsense_prompt_found __termsense_prompt_item
else
  if [[ ";${PROMPT_COMMAND:-};" != *";_termsense_prompt_cleanup;"* ]]; then
    PROMPT_COMMAND="_termsense_prompt_cleanup${PROMPT_COMMAND:+;$PROMPT_COMMAND}"
  fi
fi
