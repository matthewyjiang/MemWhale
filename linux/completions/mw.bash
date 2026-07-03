# bash completion for mw (MemoryWhale).
# Install:  cp mw.bash ~/.local/share/bash-completion/completions/mw
#       or: source it from ~/.bashrc

_mw_complete() {
  local cur
  cur="${COMP_WORDS[COMP_CWORD]}"

  if [ "$COMP_CWORD" -eq 1 ]; then
    COMPREPLY=( $(compgen -W "\
list show mark replay demo search \
export import push context doctor global \
--live --notes --help" -- "$cur") )
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
    import)
      # a bundle directory or an exported .sqlite3 file
      COMPREPLY=( $(compgen -f -- "$cur") )
      ;;
    push)
      # ssh hosts from ~/.ssh/config
      local hosts
      hosts="$(sed -n 's/^[Hh]ost[[:space:]]\{1,\}\(.*\)/\1/p' ~/.ssh/config 2>/dev/null | tr ' ' '\n' | grep -v '[*?]')"
      COMPREPLY=( $(compgen -W "$hosts" -- "$cur") )
      ;;
    context)
      COMPREPLY=( $(compgen -W "--last-error --limit project:" -- "$cur") )
      ;;
    export)
      COMPREPLY=( $(compgen -W "project:" -- "$cur") )
      ;;
  esac
}
complete -F _mw_complete mw
