# MemoryWhale per-command recorder (bash + zsh).
#
# Source this from your shell rc to record every interactive command — its
# working directory and exit code — into the local MemoryWhale database via
# mw-remember. This is a lightweight per-command INDEX; it does not capture
# output. For a full faithful transcript of a whole session, use `mw` instead.
#
#   echo '. /usr/share/memorywhale/memorywhale.sh' >> ~/.bashrc   # or ~/.zshrc
#
# Toggle off for one shell:   export MW_PERCMD_OFF=1
# Everything stays local and is never uploaded.

# Interactive shells only; never load twice.
case $- in *i*) : ;; *) return 0 2>/dev/null || true ;; esac
[ -n "${__MW_PERCMD_LOADED:-}" ] && return 0 2>/dev/null
__MW_PERCMD_LOADED=1

# Record one command. $1 = exit code, $2 = command line. Fire-and-forget in a
# detached subshell so the prompt stays snappy and a hiccup never breaks the
# shell. Word-splitting turns the line into argv for mw-remember.
__mw_record() {
  [ -n "${MW_PERCMD_OFF:-}" ] && return 0
  [ -n "${MW_RECORDING:-}" ] && return 0   # already inside a full `mw` session
  [ -z "${2:-}" ] && return 0
  case "$2" in
    __mw_*|mw-remember*) return 0 ;;        # don't record our own bookkeeping
  esac
  local bin
  bin="$(command -v mw-remember 2>/dev/null || true)"
  [ -z "$bin" ] && [ -x "$HOME/.local/bin/mw-remember" ] && bin="$HOME/.local/bin/mw-remember"
  [ -x "$bin" ] || return 0
  if [ -n "${ZSH_VERSION:-}" ]; then
    ( "$bin" --cwd "$PWD" --exit-code "$1" --notes "per-command hook" -- ${=2} >/dev/null 2>&1 & )
  else
    ( "$bin" --cwd "$PWD" --exit-code "$1" --notes "per-command hook" -- $2 >/dev/null 2>&1 & )
  fi
}

if [ -n "${ZSH_VERSION:-}" ]; then
  autoload -Uz add-zsh-hook 2>/dev/null
  __mw_preexec() { __MW_LAST_CMD="$1"; }
  __mw_precmd()  {
    local code=$?
    [ -n "${__MW_LAST_CMD:-}" ] && __mw_record "$code" "$__MW_LAST_CMD"
    __MW_LAST_CMD=""
  }
  add-zsh-hook preexec __mw_preexec
  add-zsh-hook precmd  __mw_precmd
elif [ -n "${BASH_VERSION:-}" ]; then
  __mw_preexec() {
    # Ignore completion expansions and the prompt command itself.
    [ -n "${COMP_LINE:-}" ] && return 0
    [ "$BASH_COMMAND" = "${PROMPT_COMMAND:-}" ] && return 0
    __MW_LAST_CMD="$BASH_COMMAND"
  }
  __mw_precmd() {
    local code=$?
    [ -n "${__MW_LAST_CMD:-}" ] && __mw_record "$code" "$__MW_LAST_CMD"
    __MW_LAST_CMD=""
  }
  trap '__mw_preexec' DEBUG
  case "${PROMPT_COMMAND:-}" in
    *__mw_precmd*) : ;;
    *) PROMPT_COMMAND="__mw_precmd${PROMPT_COMMAND:+; $PROMPT_COMMAND}" ;;
  esac
fi
