#!/usr/bin/env bash
# Vortex 1.8: datos y acciones del panel «Tu celular» (CelularPopup.qml), con KDE Connect.
#
#   celular.sh estado            JSON: {"activo": bool, "dispositivos": [...]} (lo lee el panel)
#   celular.sh ACCION ID         vincular | desvincular | sonar | portapapeles | compartir | archivo
#   celular.sh buscar            vuelve a buscar celulares en la red
#
# Cada dispositivo: id, nombre, tipo, alcanzable, vinculado, pide (el celular pidió vincularse),
# bateria (-1 si no se sabe), cargando y auto (4.0: el portapapeles se comparte solo con ese aparato).
# Sirve igual para un celular que para una PC con Windows (KDE Connect para Windows, mismo protocolo):
# tipo es smartphone, tablet, desktop o laptop. Lee las propiedades por D-Bus (busctl) porque la salida de
# kdeconnect-cli está traducida y cambia con el idioma.
set -uo pipefail

DEST=org.kde.kdeconnect
RAIZ=/modules/kdeconnect/devices

# Valor de una propiedad: «s "Pixel 7"» → Pixel 7, «b true» → true, «i 3» → 3. Vacío si no existe.
propiedad() { # RUTA INTERFAZ NOMBRE
  busctl --user get-property "$DEST" "$1" "$2" "$3" 2>/dev/null |
    sed -E 's/^[a-z] //; s/^"(.*)"$/\1/'
}

# El portapapeles automático de KDE Connect (complemento «clipboard», por aparato). Viene encendido: solo
# está apagado si en su config dice autoShare=false.
CONFIG_KDEC="${XDG_CONFIG_HOME:-$HOME/.config}/kdeconnect"
auto_portapapeles() { # ID → true | false
  if grep -qsx 'autoShare=false' "$CONFIG_KDEC/$1/kdeconnect_clipboard/config"; then echo false; else echo true; fi
}

# Escribe CLAVE=VALOR en la config del complemento clipboard de un aparato sin pisar las demás claves, y se lo
# avisa a kdeconnectd: guarda los valores al cargar el complemento y solo los vuelve a leer con esta señal (la
# misma que manda su propia ventana de ajustes). Sin ella, el cambio no surtía efecto hasta reiniciarlo.
poner_clave() { # ID CLAVE VALOR
  local f="$CONFIG_KDEC/$1/kdeconnect_clipboard/config"
  mkdir -p "$(dirname "$f")"
  if grep -qs "^$2=" "$f"; then
    sed -i "s/^$2=.*/$2=$3/" "$f"
  elif [[ -s "$f" ]] && grep -qs '^\[General\]' "$f"; then
    sed -i "/^\[General\]/a $2=$3" "$f"
  else
    printf '[General]\n%s=%s\n' "$2" "$3" >>"$f"
  fi
  busctl --user emit "/kdeconnect/$1/kdeconnect_clipboard" org.kde.kdeconnect.config configChanged >/dev/null 2>&1 || true
}

# Enciende o apaga ese complemento para un aparato. (4.0: antes no le avisaba a kdeconnectd y el panel decía
# «apagado» mientras el portapapeles se seguía mandando.)
alternar_portapapeles() { # ID
  local nuevo=false
  [[ "$(auto_portapapeles "$1")" == false ]] && nuevo=true
  poner_clave "$1" autoShare "$nuevo"
}

# 4.0: KDE Connect manda de serie también lo que copian los gestores de contraseñas (sendPassword=true). De
# fábrica no: a cada aparato vinculado que todavía no tenga la clave se le pone sendPassword=false. Si alguien
# lo enciende en los ajustes de KDE Connect, la clave ya existe y se respeta.
cuidar_contrasenas() { # ID
  grep -qs '^sendPassword=' "$CONFIG_KDEC/$1/kdeconnect_clipboard/config" || poner_clave "$1" sendPassword false
}

estado() {
  if ! pgrep -u "$(id -u)" -x kdeconnectd >/dev/null 2>&1; then
    echo '{"activo": false, "dispositivos": []}'
    return 0
  fi
  local ids id ruta nombre tipo alcanzable par vinculado pide bateria cargando lista=()
  ids="$(kdeconnect-cli -l --id-only 2>/dev/null; kdeconnect-cli -a --id-only 2>/dev/null)"
  while IFS= read -r id; do
    [[ "$id" =~ ^[A-Za-z0-9_-]+$ ]] || continue
    ruta="$RAIZ/$id"
    nombre="$(propiedad "$ruta" org.kde.kdeconnect.device name)"
    tipo="$(propiedad "$ruta" org.kde.kdeconnect.device type)"
    alcanzable="$(propiedad "$ruta" org.kde.kdeconnect.device isReachable)"
    # pairState (KDE Connect 24+): 0 sin vincular, 1 lo pedimos, 2 lo pide el celular, 3 vinculado.
    par="$(propiedad "$ruta" org.kde.kdeconnect.device pairState)"
    if [[ "$par" =~ ^[0-9]+$ ]]; then
      vinculado=$([[ "$par" == 3 ]] && echo true || echo false)
      pide=$([[ "$par" == 2 ]] && echo true || echo false)
    else
      vinculado="$(propiedad "$ruta" org.kde.kdeconnect.device isPaired)"
      pide="$(propiedad "$ruta" org.kde.kdeconnect.device isPairRequestedByPeer)"
    fi
    bateria="$(propiedad "$ruta/battery" org.kde.kdeconnect.device.battery charge)"
    cargando="$(propiedad "$ruta/battery" org.kde.kdeconnect.device.battery isCharging)"
    [[ "$vinculado" == true ]] && cuidar_contrasenas "$id"
    lista+=("$(jq -n --arg id "$id" --arg nombre "${nombre:-$id}" --arg tipo "${tipo:-smartphone}" \
      --argjson alcanzable "$([[ "$alcanzable" == true ]] && echo true || echo false)" \
      --argjson vinculado "$([[ "$vinculado" == true ]] && echo true || echo false)" \
      --argjson pide "$([[ "$pide" == true ]] && echo true || echo false)" \
      --argjson bateria "$([[ "$bateria" =~ ^-?[0-9]+$ ]] && echo "$bateria" || echo -1)" \
      --argjson cargando "$([[ "$cargando" == true ]] && echo true || echo false)" \
      --argjson auto "$(auto_portapapeles "$id")" \
      '{id: $id, nombre: $nombre, tipo: $tipo, alcanzable: $alcanzable, vinculado: $vinculado, pide: $pide,
        bateria: $bateria, cargando: $cargando, auto: $auto}')")
  done < <(sort -u <<<"$ids")
  printf '%s\n' "${lista[@]}" | jq -s '{activo: true, dispositivos: (map(select(. != null)) | sort_by((.vinculado | not), .nombre))}'
}

accion="${1:-estado}"
id="${2:-}"
case "$accion" in
  estado) estado ;;
  buscar) kdeconnect-cli --refresh >/dev/null 2>&1 ;;
  vincular | desvincular | sonar | portapapeles | compartir | archivo)
    [[ "$id" =~ ^[A-Za-z0-9_-]+$ ]] || { echo "celular.sh: falta el id del dispositivo" >&2; exit 2; }
    case "$accion" in
      # Si el celular ya lo pidió, --pair lo acepta; si no, se lo pide al celular.
      vincular) kdeconnect-cli -d "$id" --pair; cuidar_contrasenas "$id" ;;
      desvincular) kdeconnect-cli -d "$id" --unpair ;;
      sonar) kdeconnect-cli -d "$id" --ring ;;
      portapapeles) kdeconnect-cli -d "$id" --send-clipboard ;;
      compartir) alternar_portapapeles "$id" ;;
      archivo)
        command -v zenity >/dev/null || { notify-send -a "Vortexine's OS" "Tu celular" "Falta zenity para elegir archivos (sudo pacman -S zenity)." 2>/dev/null; exit 1; }
        enviados=0
        while IFS= read -r f; do
          [[ -f "$f" ]] || continue
          kdeconnect-cli -d "$id" --share "$f" >/dev/null 2>&1 && enviados=$((enviados + 1))
        done < <(zenity --file-selection --multiple --separator=$'\n' --title="Enviar al celular" 2>/dev/null)
        ((enviados)) && notify-send -a "Vortexine's OS" -i smartphone "Tu celular" \
          "$([[ $enviados == 1 ]] && echo "Enviando 1 archivo" || echo "Enviando $enviados archivos")" 2>/dev/null
        ;;
    esac >/dev/null 2>&1
    ;;
  *) echo "uso: celular.sh estado|buscar|vincular|desvincular|sonar|portapapeles|compartir|archivo [ID]" >&2; exit 2 ;;
esac
exit 0
