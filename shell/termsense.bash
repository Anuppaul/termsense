# TermSense Bash integration
# Enable with: eval "$(termsense init bash)"

[[ $- == *i* ]] || return 0 2>/dev/null || true

if (( BASH_VERSINFO[0] < 5 )); then
  printf 'TermSense requires Bash 5.0 or newer.\n' >&2
  return 0 2>/dev/null || true
fi

if [[ -n "${__TERMSENSE_BASH_LOADED:-}" ]]; then
  return 0 2>/dev/null || true
fi
__TERMSENSE_BASH_LOADED=1

_termsense_load_config() {
  local config_file="${TERMSENSE_CONFIG_FILE:-${XDG_CONFIG_HOME:-$HOME/.config}/termsense/config.conf}"
  [[ -r "$config_file" ]] || return 0

  local key value
  while IFS='=' read -r key value; do
    key="${key#"${key%%[![:space:]]*}"}"
    key="${key%"${key##*[![:space:]]}"}"
    value="${value#"${value%%[![:space:]]*}"}"
    value="${value%"${value##*[![:space:]]}"}"

    [[ -n "$key" && "$key" != \#* ]] || continue

    case "$key" in
      auto_suggest)
        [[ -v TERMSENSE_AUTO_SUGGEST ]] || TERMSENSE_AUTO_SUGGEST="$value"
        ;;
      max_visible)
        [[ -v TERMSENSE_MAX_VISIBLE ]] || TERMSENSE_MAX_VISIBLE="$value"
        ;;
      ghost)
        [[ -v TERMSENSE_GHOST ]] || TERMSENSE_GHOST="$value"
        ;;
      ctrl_space)
        [[ -v TERMSENSE_CTRL_SPACE ]] || TERMSENSE_CTRL_SPACE="$value"
        ;;
    esac
  done < "$config_file"
}

_termsense_load_config

: "${TERMSENSE_AUTO_SUGGEST:=1}"
: "${TERMSENSE_MAX_VISIBLE:=20}"
: "${TERMSENSE_GHOST:=1}"
: "${TERMSENSE_CTRL_SPACE:=1}"

[[ "$TERMSENSE_AUTO_SUGGEST" =~ ^[01]$ ]] || TERMSENSE_AUTO_SUGGEST=1
[[ "$TERMSENSE_GHOST" =~ ^[01]$ ]] || TERMSENSE_GHOST=1
[[ "$TERMSENSE_CTRL_SPACE" =~ ^[01]$ ]] || TERMSENSE_CTRL_SPACE=1
[[ "$TERMSENSE_MAX_VISIBLE" =~ ^[0-9]+$ ]] || TERMSENSE_MAX_VISIBLE=20
(( TERMSENSE_MAX_VISIBLE >= 1 )) || TERMSENSE_MAX_VISIBLE=1
(( TERMSENSE_MAX_VISIBLE <= 20 )) || TERMSENSE_MAX_VISIBLE=20

__TERMSENSE_VISIBLE=0
__TERMSENSE_GHOST_LEN=0
__TERMSENSE_SELECTED=0
__TERMSENSE_REPLACE_START=0
__TERMSENSE_REPLACE_END=0
__TERMSENSE_PREFIX=""
__TERMSENSE_CANDIDATES=()
__TERMSENSE_DISPLAYS=()
__TERMSENSE_DESCRIPTIONS=()
__TERMSENSE_USAGE_KEYS=()
__TERMSENSE_PALETTE_ACTIVE=0

_termsense_binary() {
  if [[ -n "${TERMSENSE_BIN:-}" && -x "$TERMSENSE_BIN" ]]; then
    printf '%s\n' "$TERMSENSE_BIN"
    return 0
  fi

  command -v termsense 2>/dev/null
}

_termsense_byte_point() {
  local prefix="${READLINE_LINE:0:READLINE_POINT}"
  local LC_ALL=C
  printf '%d\n' "${#prefix}"
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
    [[ "$name" == _* ]] || filtered+=("$name")
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

_termsense_capture_binding() {
  local key="$1"
  local prefix="\"${key}\":"
  local line

  while IFS= read -r line; do
    if [[ "$line" == "$prefix"* ]]; then
      printf '%s\n' "$line"
      return 0
    fi
  done < <(
    bind -p 2>/dev/null
    bind -s 2>/dev/null
    bind -X 2>/dev/null || true
  )
}

_termsense_restore_binding() {
  local spec="$1"
  local key="$2"

  if [[ -n "$spec" ]]; then
    bind "$spec" 2>/dev/null || true
  else
    bind -r "$key" 2>/dev/null || true
  fi
}

_termsense_restore_navigation() {
  _termsense_restore_binding "$__TERMSENSE_BIND_UP" '\e[A'
  _termsense_restore_binding "$__TERMSENSE_BIND_DOWN" '\e[B'
  _termsense_restore_binding "$__TERMSENSE_BIND_RIGHT" '\e[C'
  _termsense_restore_binding "$__TERMSENSE_BIND_LEFT" '\e[D'
  _termsense_restore_binding "$__TERMSENSE_BIND_TAB" '\C-i'
  _termsense_restore_binding "$__TERMSENSE_BIND_ESC" '\e'
  _termsense_restore_binding "$__TERMSENSE_BIND_ENTER_CR" '\C-m'
  _termsense_restore_binding "$__TERMSENSE_BIND_ENTER_LF" '\C-j'
  _termsense_restore_binding "$__TERMSENSE_BIND_INTERNAL_CLEAN" '\C-x\C-y'
  _termsense_restore_binding "$__TERMSENSE_BIND_INTERNAL_ACCEPT" '\C-x\C-z'
}

_termsense_activate_navigation() {
  bind -x '"\e[A":_termsense_select_prev' 2>/dev/null || true
  bind -x '"\e[B":_termsense_select_next' 2>/dev/null || true
  bind -x '"\e[C":_termsense_accept_ghost' 2>/dev/null || true
  bind -x '"\e[D":_termsense_move_left' 2>/dev/null || true
  bind -x '"\C-i":_termsense_accept_selected' 2>/dev/null || true
  bind -x '"\e":_termsense_dismiss' 2>/dev/null || true
  bind -x '"\C-m":_termsense_accept_selected' 2>/dev/null || true
  bind -x '"\C-j":_termsense_accept_selected' 2>/dev/null || true
}

_termsense_reset_state() {
  __TERMSENSE_CANDIDATES=()
  __TERMSENSE_DISPLAYS=()
  __TERMSENSE_DESCRIPTIONS=()
  __TERMSENSE_USAGE_KEYS=()
  __TERMSENSE_SELECTED=0
  __TERMSENSE_REPLACE_START=$READLINE_POINT
  __TERMSENSE_REPLACE_END=$READLINE_POINT
  __TERMSENSE_PREFIX=""
}

_termsense_dismiss() {
  __TERMSENSE_PALETTE_ACTIVE=0
  _termsense_clear_overlay
  _termsense_reset_state
  _termsense_restore_navigation
}

_termsense_query() {
  local limit="${1:-24}"
  local ts
  ts="$(_termsense_binary)" || return 1

  _termsense_reset_state

  local byte_point
  byte_point="$(_termsense_byte_point)"

  local value display _kind _source _score start end usage_key description first=1
  local tab
  printf -v tab '\t'
  while IFS="$tab" read -r value display _kind _source _score start end usage_key description; do
    [[ -n "$value" ]] || continue

    __TERMSENSE_CANDIDATES+=("$value")
    __TERMSENSE_DISPLAYS+=("$display")
    __TERMSENSE_DESCRIPTIONS+=("$description")
    __TERMSENSE_USAGE_KEYS+=("$usage_key")

    if (( first )); then
      __TERMSENSE_REPLACE_START=$start
      __TERMSENSE_REPLACE_END=$end
      __TERMSENSE_PREFIX="${READLINE_LINE:start:end-start}"
      first=0
    fi
  done < <("$ts" suggest "$READLINE_LINE" --cursor "$byte_point" --limit "$limit" 2>/dev/null)

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
  if [[ "$TERMSENSE_GHOST" != "0" \
        && -n "$__TERMSENSE_PREFIX" \
        && "$READLINE_POINT" -eq "${#READLINE_LINE}" \
        && "$selected" == "$__TERMSENSE_PREFIX"* ]]; then
    suffix="${selected:${#__TERMSENSE_PREFIX}}"
  fi

  local max=$TERMSENSE_MAX_VISIBLE
  [[ "$max" =~ ^[0-9]+$ ]] || max=20
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
    if [[ -n "${__TERMSENSE_DESCRIPTIONS[index]}" ]]; then
      label+="  —  ${__TERMSENSE_DESCRIPTIONS[index]}"
    fi
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
  if [[ "$TERMSENSE_AUTO_SUGGEST" == "0" && "$__TERMSENSE_PALETTE_ACTIVE" == "0" ]]; then
    return 0
  fi

  local left="${READLINE_LINE:0:READLINE_POINT}"
  if [[ -z "${left//[[:space:]]/}" && "$__TERMSENSE_PALETTE_ACTIVE" == "0" ]]; then
    _termsense_dismiss
    return 0
  fi

  local limit=24
  [[ "$__TERMSENSE_PALETTE_ACTIVE" != "0" ]] && limit=500

  __TERMSENSE_SELECTED=0
  if _termsense_query "$limit"; then
    _termsense_draw_overlay
  else
    _termsense_dismiss
  fi
}

_termsense_before_accept() {
  __TERMSENSE_PALETTE_ACTIVE=0
  _termsense_clear_overlay
  _termsense_reset_state
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
    printf '\nTermSense binary not found.\n' >&2
    return 1
  }

  __TERMSENSE_PALETTE_ACTIVE=1
  __TERMSENSE_SELECTED=0

  local left="${READLINE_LINE:0:READLINE_POINT}"
  local limit=500
  [[ -z "${left//[[:space:]]/}" ]] && limit=0

  if _termsense_query "$limit"; then
    _termsense_draw_overlay
  else
    _termsense_dismiss
  fi
}

_termsense_prompt_cleanup() {
  __TERMSENSE_PALETTE_ACTIVE=0
  _termsense_clear_overlay
  _termsense_reset_state
  _termsense_restore_navigation
  _termsense_refresh_shell_context
}

_termsense_refresh_shell_context

__TERMSENSE_BIND_UP="$(_termsense_capture_binding '\e[A')"
__TERMSENSE_BIND_DOWN="$(_termsense_capture_binding '\e[B')"
__TERMSENSE_BIND_RIGHT="$(_termsense_capture_binding '\e[C')"
__TERMSENSE_BIND_LEFT="$(_termsense_capture_binding '\e[D')"
__TERMSENSE_BIND_TAB="$(_termsense_capture_binding '\C-i')"
__TERMSENSE_BIND_ESC="$(_termsense_capture_binding '\e')"
__TERMSENSE_BIND_ENTER_CR="$(_termsense_capture_binding '\C-m')"
__TERMSENSE_BIND_ENTER_LF="$(_termsense_capture_binding '\C-j')"
__TERMSENSE_BIND_INTERNAL_CLEAN="$(_termsense_capture_binding '\C-x\C-y')"
__TERMSENSE_BIND_INTERNAL_ACCEPT="$(_termsense_capture_binding '\C-x\C-z')"

bind -x '"\C-x\C-t":_termsense_refresh'

for __termsense_code in $(seq 32 126); do
  printf -v __termsense_hex '%02x' "$__termsense_code"
  bind "\"\\x${__termsense_hex}\": \"\\C-v\\x${__termsense_hex}\\C-x\\C-t\"" 2>/dev/null || true
done
unset __termsense_code __termsense_hex

bind '"\C-x\C-b": backward-delete-char'
bind '"\C-h": "\C-x\C-b\C-x\C-t"'
bind '"\C-?": "\C-x\C-b\C-x\C-t"'

if [[ "$TERMSENSE_CTRL_SPACE" != "0" ]]; then
  bind -x '"\C- ":_termsense_ctrl_space'
  bind -x '"\C-@":_termsense_ctrl_space'
fi

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
