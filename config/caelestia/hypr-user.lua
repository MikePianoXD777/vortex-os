-- Vortex OS: añadidos a la config de Hyprland de Caelestia.
-- Para tus propios cambios usa ~/.config/vortex/hypr-mio.lua (se carga al final).
local home = os.getenv("HOME")

-- Avisa de un error en un archivo del usuario (pcall lo capturaría en silencio):
-- lo escribe en stderr y lo muestra como notificación.
local function avisar_error(ruta, err)
    local texto = tostring(err)
    if not texto:find(ruta, 1, true) then texto = ruta .. ": " .. texto end
    texto = "Error en " .. texto
    io.stderr:write("Vortex: " .. texto .. "\n")
    -- notify-send interpreta el cuerpo como markup: se escapan &, < y > (p. ej. «near <eof>»).
    local cuerpo = texto:gsub("&", "&amp;"):gsub("<", "&lt;"):gsub(">", "&gt;")
    local comilla = "'" .. cuerpo:gsub("'", "'\\''") .. "'"
    -- Con espera (hasta ~20 s): al iniciar la sesión el demonio de notificaciones aún no está.
    pcall(hl.exec_cmd, "sh -c 'for i in $(seq 20); do busctl --user status org.freedesktop.Notifications >/dev/null 2>&1 && break; sleep 1; done; notify-send -u critical -a Vortex \"Vortex: error en tu config\" \"$1\"' _ " .. comilla)
end

-- Carga un archivo opcional: si no existe, devuelve nil sin quejarse;
-- si existe y falla, avisa y devuelve nil.
local function cargar_opcional(ruta)
    local f = io.open(ruta, "r")
    if not f then return nil end
    f:close()
    local ok, res = pcall(dofile, ruta)
    if not ok then
        avisar_error(ruta, res)
        return nil
    end
    return res
end

local perfil = cargar_opcional(home .. "/.config/vortex/perfil.lua")
if type(perfil) ~= "table" then perfil = {} end

hl.config({
    animations = { enabled = perfil.animaciones ~= false },
    -- Caelestia ya marca los juegos con la regla «immediate»; esto la deja funcionar
    -- (menos latencia de entrada a cambio de posible tearing, solo en juegos).
    general = { allow_tearing = true },
    -- Sin la ventana «Hyprland Updated» (un proceso Qt de ~74 MB) ni el aviso de donación.
    ecosystem = { no_update_news = true, no_donation_nag = true },
    -- Opcional, para ponerlo en hypr-mio.lua: con escala fraccionaria las apps X11
    -- (Steam, Proton) salen un poco borrosas pero del tamaño correcto. Con
    -- xwayland = { force_zero_scaling = true } salen nítidas, pero a escala 1, o sea
    -- más pequeñas (a 1.5 se ven al 67 %) salvo que compenses con GDK_SCALE o
    -- STEAM_FORCE_DESKTOPUI_SCALING. Por eso no se activa por defecto.
})

-- Teclado: el instalador guarda la distribución del sistema (formato XKB) en
-- ~/.config/vortex/teclado.lua, p. ej. «return "latam"» o
-- «return { layout = "latam", variant = "deadtilde" }». Caelestia fija «us».
local teclado = cargar_opcional(home .. "/.config/vortex/teclado.lua")
if type(teclado) == "string" then teclado = { layout = teclado } end
if type(teclado) == "table" and type(teclado.layout) == "string" and teclado.layout ~= "" then
    hl.config({
        input = {
            kb_layout = teclado.layout,
            kb_variant = type(teclado.variant) == "string" and teclado.variant or "",
        },
    })
end

-- Pantalla: Caelestia fija escala 1; con «auto» Hyprland la calcula según los PPI del
-- monitor: más de 140 PPI da 1.5 y más de 200 da 2. Así las laptops HiDPI no quedan
-- diminutas, pero también las de 13-15,6" a 1080p (~140-165 PPI) quedan a 1.5, y un
-- monitor de escritorio normal sigue a 1. Para fijar la tuya, usa hl.monitor en hypr-mio.lua.
hl.monitor({ output = "", mode = "preferred", position = "auto", scale = "auto" })

-- Claude: abre una terminal conectada a Claude, que puede ver tu pantalla si lo permites.
hl.bind("SUPER + A", hl.dsp.exec_cmd("vortex claude"))
-- Modo juego a mano (gamemode lo activa solo al abrir un juego).
hl.bind("SUPER + G", hl.dsp.exec_cmd("vortex modo-juego alternar"))

cargar_opcional(home .. "/.config/vortex/hypr-mio.lua")
