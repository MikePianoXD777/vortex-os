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

if ! curl -fsS --max-time 10 -o /dev/null https://archlinux.org; then
  error "No hay internet. Conéctate (nmtui o iwctl) y vuelve a intentar."
fi

libre_gb=$(df --output=avail -BG / | tail -1 | tr -dc '0-9')
((libre_gb >= 10)) || error "Hacen falta al menos 10 GB libres en / (hay ${libre_gb} GB)."

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
