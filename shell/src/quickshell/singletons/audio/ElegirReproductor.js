.pragma library

// Vortex 4.1: qué reproductor enseña el escritorio (la barra y el panel de música).
// Sin tipos de Qt para poder probarlo con Node (pruebas/elegir-reproductor.sh).
//
// Antes ganaba el primero que sonara en orden alfabético: si sonaba Vortex Player y en YouTube pasabas el
// mouse por un video, la vista previa de Firefox («firefox» < «vortex_player») se quedaba con la barra, y en
// el brinco de ida y vuelta la portada se perdía. Ahora:
//   1. el que ya estaba sonando sigue mientras suene;
//   2. otro que empieza a sonar gana solo cuando lleva ASENTADO ms sonando (una vista previa dura menos);
//   3. mientras, se queda el de antes (aunque esté en pausa).

var ASENTADO = 2500;

// players: [{busName, isPlaying, stopped, title, canControl}] ya ordenados; ultimo: busName del que sonaba;
// desde: {busName: ms en que empezó a sonar}; ahora: ms. Devuelve el índice elegido o -1.
function elegir(players, ultimo, desde, ahora) {
    var i;
    var previo = -1;
    for (i = 0; i < players.length; i++) if (ultimo !== "" && players[i].busName === ultimo) previo = i;
    if (previo >= 0 && players[previo].isPlaying) return previo;
    for (i = 0; i < players.length; i++) {
        var d = desde[players[i].busName];
        // Sin marca todavía: acaba de empezar (cuenta como recién llegado), salvo que no haya otro a quién volver.
        if (players[i].isPlaying && d !== undefined && ahora - d >= ASENTADO) return i;
    }
    if (previo >= 0) return previo;
    for (i = 0; i < players.length; i++) if (players[i].isPlaying) return i;
    for (i = 0; i < players.length; i++) if (!players[i].stopped && players[i].title !== "") return i;
    for (i = 0; i < players.length; i++) if (players[i].canControl) return i;
    return players.length > 0 ? 0 : -1;
}

// Actualiza las marcas de «desde cuándo suena» y dice en cuántos ms hay que volver a elegir (0: no hace falta).
function marcar(players, desde, ahora) {
    var vistos = {};
    var falta = 0;
    for (var i = 0; i < players.length; i++) {
        var b = players[i].busName;
        if (!players[i].isPlaying) continue;
        vistos[b] = true;
        if (desde[b] === undefined) desde[b] = ahora;
        var resta = ASENTADO - (ahora - desde[b]);
        if (resta > 0 && (falta === 0 || resta < falta)) falta = resta;
    }
    for (var k in desde) if (!vistos[k]) delete desde[k];
    return falta;
}
