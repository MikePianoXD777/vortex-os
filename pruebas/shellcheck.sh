#!/usr/bin/env bash
# Pasa shellcheck por todos los scripts de Vortex (los de bash o sh que están en git), con avisos de nivel
# «warning» para arriba, como pruebas/miku.sh. Fuera: shell/ (Serpantinum, upstream) y .claude/.
#   pruebas/shellcheck.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$AQUI" || exit 1

if ! command -v shellcheck >/dev/null; then
  echo "Falta shellcheck: sudo pacman -S shellcheck" >&2
  exit 1
fi

scripts=()
while IFS= read -r -d '' f; do
  case "$f" in shell/* | .claude/*) continue ;; esac
  [[ -f "$f" ]] || continue
  if [[ "$f" == *.sh ]] || head -n1 "$f" | grep -qE '^#!.*\b(ba)?sh\b'; then
    scripts+=("$f")
  fi
done < <(git ls-files -z --cached --others --exclude-standard)   # también los nuevos sin commit

if (( ${#scripts[@]} == 0 )); then
  echo "No encontré scripts: ¿git no puede leer el repo? (git ls-files)" >&2
  exit 1
fi
echo "shellcheck en ${#scripts[@]} scripts"
if shellcheck -x -S warning "${scripts[@]}"; then
  echo "Todo bien"
else
  echo "Hay avisos"
  exit 1
fi
