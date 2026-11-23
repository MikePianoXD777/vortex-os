#!/usr/bin/env bash
# Prueba de qué reproductor enseña la barra (shell/src/quickshell/singletons/audio/ElegirReproductor.js, 4.1):
# una vista previa de YouTube en Firefox ya no le quita la barra (ni la portada) a Vortex Player.
# Solo Node: no abre el shell ni toca tu escritorio.
#
#   pruebas/elegir-reproductor.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
command -v node >/dev/null || { echo "  --  falta node: se salta"; exit 0; }
node - "$AQUI/shell/src/quickshell/singletons/audio/ElegirReproductor.js" <<'JS'
const fs = require("fs"), vm = require("vm");
const ctx = {};
vm.runInNewContext(fs.readFileSync(process.argv[2], "utf8").replace(/^\.pragma library/m, ""), ctx);
let fallos = 0;
const bien = (c, t) => { console.log((c ? "  ok  " : "  MAL ") + t); if (!c) fallos++; };
const P = (busName, isPlaying, extra = {}) => ({ busName, isPlaying, stopped: !isPlaying, title: "x", canControl: true, ...extra });
const nombre = (l, i) => (i >= 0 ? l[i].busName : "ninguno");
const VP = "org.mpris.MediaPlayer2.vortex_player", FX = "org.mpris.MediaPlayer2.firefox.instance_1_441";

// Suena Vortex Player y empieza una vista previa en Firefox («firefox» va antes en el orden).
let desde = {};
let l = [P(FX, true), P(VP, true)];
ctx.marcar(l, desde, 1000);
bien(nombre(l, ctx.elegir(l, VP, desde, 1000)) === VP, "con Vortex Player sonando, la vista previa no se lleva la barra");
bien(nombre(l, ctx.elegir(l, VP, desde, 60000)) === VP, "ni aunque dure: el que ya sonaba sigue mientras suene");

// Vortex Player en pausa y la vista previa: se queda Vortex Player hasta que Firefox lleve 2,5 s sonando.
desde = {};
l = [P(FX, true), P(VP, false, { stopped: false })];
const falta = ctx.marcar(l, desde, 1000);
bien(falta > 0 && falta <= 2500, "pide volver a elegir cuando se asiente (" + falta + " ms)");
bien(nombre(l, ctx.elegir(l, VP, desde, 1000)) === VP, "una vista previa de un segundo no cambia la barra");
bien(nombre(l, ctx.elegir(l, VP, desde, 1000 + 2600)) === FX, "un video que sigue sonando sí se queda con la barra");

// La vista previa se acabó: su marca se borra.
l = [P(FX, false), P(VP, false, { stopped: false })];
ctx.marcar(l, desde, 5000);
bien(Object.keys(desde).length === 0, "al dejar de sonar se olvida desde cuándo sonaba");
bien(nombre(l, ctx.elegir(l, VP, desde, 5000)) === VP, "y la barra sigue con Vortex Player");

// Al arrancar el shell: lo que suene, aunque todavía no tenga marca.
bien(nombre(l = [P(FX, false), P(VP, true)], ctx.elegir(l, "", {}, 0)) === VP, "al arrancar, el que suena");
bien(nombre(l = [P(FX, false, { stopped: false }), P(VP, false, { stopped: true, title: "" })], ctx.elegir(l, "", {}, 0)) === FX,
  "sin nadie sonando, el que tiene algo cargado");
bien(ctx.elegir([], "", {}, 0) === -1, "sin reproductores, ninguno");

console.log(fallos === 0 ? "\nTodo bien" : "\n" + fallos + " fallos");
process.exit(fallos === 0 ? 0 : 1);
JS
