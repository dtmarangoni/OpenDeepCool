# Shared by the git hooks. Prints the owner's sensitive values (one per line)
# from the git-ignored docs/local/machine.md, between the markers
# <!-- guard:sensitive-begin --> and <!-- guard:sensitive-end -->.
# Prints nothing when the file is absent (e.g. a fresh clone elsewhere).
sensitive_values() {
  local facts
  facts="$(git rev-parse --show-toplevel)/docs/local/machine.md"
  [[ -f ${facts} ]] || return 0
  sed -n '/<!-- guard:sensitive-begin -->/,/<!-- guard:sensitive-end -->/p' "${facts}" \
    | grep -v 'guard:sensitive' \
    | sed -E 's/^[[:space:]]*[-*][[:space:]]+//; s/`//g; s/[[:space:]]+$//' \
    | grep -v '^$' || true
}
