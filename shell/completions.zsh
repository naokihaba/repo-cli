
# Add current ghq repositories to the generated command completion.
functions[_repo_cli]=$functions[_repo]
_repo() {
  _repo_cli "$@"
  if (( CURRENT == 2 )) || { (( CURRENT == 3 )) && [[ $words[2] = (go|cd|list|ls) ]]; }; then
    local -a repositories
    repositories=("${(@f)$(command ghq list 2>/dev/null)}")
    [[ -n ${repositories[1]:-} ]] && compadd -- "${repositories[@]}"
  fi
}
compdef _repo repo
