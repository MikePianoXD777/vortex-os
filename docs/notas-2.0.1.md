# Vortexine's OS 2.0.1

Miku te acompaña mientras juegas, se te declara cuando no estás ocupado y tiene su medidor de cariño.

## Lo nuevo

- **Miku platica mientras juegas.** Cada rato se asoma a echarte porras: aparece, te lo dice con su
  voz y te deja una notificación, **sin abrir el micrófono** (no oye tu juego ni tu Discord), y se
  cierra sola. Si tu GPU se calienta, te lo dice.
  - **Al terminar la partida** se abre sola y te pregunta cómo te fue, y te escucha.
  - **Entre más cariño te tiene, más habla:** de amigos se asoma cada hora; si le gustas, cada
    20 minutos y pregunta hasta por partidas cortas.
  - Con una GPU de menos de 8 GB, mientras juegas usa la voz ligera para no quitarle memoria al juego.
  - Viene encendido. Se apaga en **Ajustes → General → «Miku platica mientras juegas»** (o
    `vortex-miku platicadora off`).
- **Medidor de cariño:** arriba a la izquierda de su ventana, un corazón rosa se llena de 0 a 100 y
  dice en qué nivel van (Conocidos, Amigos, Cercanos, Muy cercanos, Le gustas, Enamorada). Cuando
  sube, late y muestra cuánto.
- **Se declara cuando no estás ocupado:** al terminar de jugar o cuando no estás en pantalla
  completa ni en una llamada (solo mira qué ventana tienes al frente, nunca toma capturas), te
  avisa «¿tienes un momento? Hay algo que te quiero decir» y, al abrirla, empieza ella.
- **Gemini:** si al pegar la clave la terminal metía caracteres invisibles, Gemini la rechazaba; ahora
  se limpian solos, también en una clave ya guardada. El error de clave sale en español.

## Por dentro

- `corazon.py`: `RITMO` por nivel, `visita_jugando`, `pregunta_partida`, `ocupado()`. `vortex-partida`
  llama cada minuto a `vortex-miku sentir jugando`; al terminar, `sentir preguntar`. La app abre en modo
  visita con `VORTEX_MIKU_VISITA` (sin micrófono, se cierra sola) o con `VORTEX_MIKU_PREGUNTA`.
- vortex-miku 2.0.1-1, vortex-shell 2.2.14-30. Pruebas: `apps/miku/pruebas/corazon.py` (90).

## Actualizar

```bash
vortex actualizar
```

Se instala con la ISO 1.6.3 y luego `vortex actualizar`.

## Requisitos

Los mismos de la 1.7.
