# bash completion for mw (MemoryWhale).
# Install:  cp mw.bash ~/.local/share/bash-completion/completions/mw
#       or: source it from ~/.bashrc

_mw_complete() {
  local cur prev
  cur="${COMP_WORDS[COMP_CWORD]}"
  prev="${COMP_WORDS[COMP_CWORD-1]}"

  if [ "$COMP_CWORD" -eq 1 ]; then
    COMPREPLY=( $(compgen -W "list show global --notes --help" -- "$cur") )
    return
  fi

  case "${COMP_WORDS[1]}" in
    global)
      [ "$COMP_CWORD" -eq 2 ] && COMPREPLY=( $(compgen -W "on off status" -- "$cur") )
      ;;
    show)
      # session ids come from `mw list`; offer them when available.
      if [ "$COMP_CWORD" -eq 2 ] && command -v mw >/dev/null 2>&1; then
        local ids
        ids="$(mw list 2>/dev/null | sed -n 's/^#\([0-9][0-9]*\).*/\1/p')"
        COMPREPLY=( $(compgen -W "$ids" -- "$cur") )
      fi
      ;;
  esac
}
complete -F _mw_complete mw
