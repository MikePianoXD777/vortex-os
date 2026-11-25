#!/usr/bin/env bash
# qs_manager.sh falso (lo corre «bash qs_manager.sh»): anota los argumentos en $PANELES.
printf '%s\n' "$*" >>"$PANELES"
