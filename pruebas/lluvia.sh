#!/usr/bin/env bash
# shellcheck disable=SC2015
# Prueba de la lluvia de dígitos de la terminal (4.3, rust/apps/vortex-lluvia): en una terminal falsa
# (pty de «script», sin ventanas) dura menos de 2 s, usa la pantalla alterna y la deja como estaba;
# «vortex saludo lluvia off» la apaga, y sin terminal (a un tubo) no manda nada.
#
#   pruebas/lluvia.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-lluvia.XXXXXX")"
trap 'rm -rf "$T"' EXIT
FALLOS=0
bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }

B="$(command -v vortex-lluvia 2>/dev/null || true)"
if (cd "$AQUI/rust" && cargo build -q -p vortex-lluvia 2>/dev/null); then
  B="$AQUI/rust/target/debug/vortex-lluvia"
fi
[[ -x "$B" ]] || { echo "  --  no hay vortex-lluvia (ni cargo): se salta"; exit 0; }
cp "$B" "$T/vortex-lluvia"
command -v script >/dev/null || { echo "  --  falta «script» (util-linux): se salta"; exit 0; }

cat >"$T/pty.sh" <<EOS
stty cols 100 rows 30
inicio=\$(date +%s%N)
"$T/vortex-lluvia"
echo "FIN rc=\$? ms=\$(( (\$(date +%s%N) - inicio) / 1000000 ))"
EOS
correr() { (sleep 4 | XDG_CONFIG_HOME="$T/config" timeout 10 script -qec "bash $T/pty.sh" /dev/null >"$T/out" 2>&1); }

echo "== en una terminal"
mkdir -p "$T/config"; correr
ms="$(grep -a -o 'ms=[0-9]*' "$T/out" | cut -d= -f2)"
[[ -n "$ms" && "$ms" -ge 600 && "$ms" -le 2100 ]] && bien "llovió ${ms} ms" || mal "duró «${ms}» ms"
grep -aq 'rc=0' "$T/out" && bien "sale bien" || mal "salida: $(tail -c 200 "$T/out" | cat -v)"
python3 - "$T/out" <<'PY' && bien "pantalla alterna, cursor y colores de regreso" || mal "no deja la terminal como estaba"
import sys
s = open(sys.argv[1], 'rb').read()
fin = s.rfind(b'\x1b[?1049l')
ok = b'\x1b[?1049h' in s and fin > 0 and b'\x1b[?25h' in s[:fin + 8] and b'\x1b[0m' in s[fin - 20:fin + 8]
sys.exit(0 if ok else 1)
PY
LC_ALL=C grep -aq '[0-9]' <(tr -d '\033' <"$T/out" | head -c 20000) && bien "dibuja dígitos" || mal "no dibujó"

echo "== apagada («vortex saludo lluvia off»)"
mkdir -p "$T/config/vortex" && : >"$T/config/vortex/sin-lluvia"; correr
ms="$(grep -a -o 'ms=[0-9]*' "$T/out" | cut -d= -f2)"
[[ -n "$ms" && "$ms" -le 200 ]] && ! grep -aq $'\e\\[?1049h' "$T/out" && bien "no llueve (${ms} ms)" || mal "llovió apagada (${ms} ms)"
rm -f "$T/config/vortex/sin-lluvia"

echo "== sin terminal"
n="$("$T/vortex-lluvia" </dev/null | wc -c)"
[[ "$n" == 0 ]] && bien "a un tubo no manda nada" || mal "mandó $n bytes"

echo
((FALLOS == 0)) && echo "Todo bien" || { echo "$FALLOS fallos"; exit 1; }
