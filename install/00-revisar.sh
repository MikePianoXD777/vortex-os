# shellcheck shell=bash
paso "Revisando el equipo"

[[ $EUID -ne 0 ]] || error "Corre el instalador con tu usuario normal, no como root."
[[ -f /etc/arch-release ]] || error "Vortex OS se instala sobre Arch Linux (o CachyOS)."

case "$(uname -m)" in
  x86_64) ;;
  *) error "Por ahora solo PCs x86_64 (Intel/AMD). ARM no está soportado: los repos de CachyOS no lo tienen." ;;
esac

tiene sudo || error "Falta sudo. Instálalo y agrega tu usuario al grupo wheel."
if [[ "$VORTEX_DRY_RUN" != 1 ]]; then
  sudo -v || error "Necesito permisos de sudo."
  # Mantiene vivo sudo mientras dura la instalación.
  while true; do sudo -n true; sleep 50; kill -0 "$$" || exit; done 2>/dev/null &
fi

# Se prueban varios destinos: basta con que responda uno. Así una caída de
# archlinux.org (que no es de donde se descarga todo) no da un falso «sin internet».
hay_internet=0
for destino in https://geo.mirror.pkgbuild.com https://mirror.cachyos.org \
  https://aur.archlinux.org https://archlinux.org; do
  if curl -fsS --max-time 8 -o /dev/null "$destino" 2>/dev/null; then
    hay_internet=1
    break
  fi
done
((hay_internet)) || error "No hay internet. Conéctate (nmtui o iwctl) y vuelve a intentar."

libre_gb=$(df --output=avail -BG / | tail -1 | tr -dc '0-9')
# La instalación completa ocupa ~11,5 GB y pacman llega a descargar ~3,2 GB de
# paquetes antes de limpiar la caché; con margen para el uso diario pedimos 20 GB
# (menos si se omiten juegos o dev) y avisamos si queda justo.
minimo_gb=20
((VORTEX_JUEGOS)) || minimo_gb=$((minimo_gb - 4))
((VORTEX_DEV)) || minimo_gb=$((minimo_gb - 3))
((libre_gb >= minimo_gb)) || error "Hacen falta al menos ${minimo_gb} GB libres en / (hay ${libre_gb} GB)."
((libre_gb >= minimo_gb + 5)) || aviso "Poco espacio libre (${libre_gb} GB): la instalación cabe, pero quedará muy justo."

VORTEX_ES_CACHYOS=0
grep -qi '^ID=cachyos' /etc/os-release && VORTEX_ES_CACHYOS=1
export VORTEX_ES_CACHYOS

info "RAM: $(ram_mb) MB · núcleos: $(nproc) · libre en /: ${libre_gb} GB"
gpus | sed 's/^/    GPU: /' || true
es_laptop && info "Equipo portátil (batería detectada)"
((VORTEX_ES_CACHYOS)) && info "Ya estás en CachyOS: se reutilizan sus repos y su kernel."

[[ -n "$VORTEX_PERFIL" ]] || VORTEX_PERFIL="$(perfil_sugerido)"
case "$VORTEX_PERFIL" in
  ligero | equilibrado | bonito) ;;
  *) error "Perfil desconocido: $VORTEX_PERFIL (usa ligero, equilibrado o bonito)" ;;
esac
bien "Perfil gráfico: $VORTEX_PERFIL"
