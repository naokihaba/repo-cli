# The binary selects a path; the function changes the current shell directory.
repo() {
  local argument
  for argument in "$@"; do
    case "$argument" in
      -h|--help|-V|--version) command repo "$@"; return $? ;;
    esac
  done
  case "${1:-}" in
    list|ls|root|get|init|completions|help) command repo "$@" ;;
    *)
      local selected result
      selected=$(command repo "$@")
      result=$?
      (( result == 0 )) || return $result
      [[ -n $selected ]] || return 0
      builtin cd -- "$selected"
      ;;
  esac
}
