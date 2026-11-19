-- Vortexine's OS 4.0: el Hyprland mínimo de la pantalla de inicio (lo arranca vortex-greeter como el
-- usuario «greeter»). Solo dibuja el login de Quickshell y se va en cuanto la persona entra.
local dir = "/usr/share/vortex/greeter"

hl.config({
    ecosystem = { no_update_news = true, no_donation_nag = true },
    misc = {
        disable_hyprland_logo = true,
        disable_splash_rendering = true,
        background_color = 0x000000,
        font_family = "Sans",
    },
    general = { gaps_in = 0, gaps_out = 0, border_size = 0 },
    decoration = { rounding = 0, shadow = { enabled = false }, blur = { enabled = false } },
    animations = { enabled = false },
    -- Por software en todas: con algunas gráficas (AMD, laptops con dos) el cursor por hardware no se ve.
    cursor = { no_hardware_cursors = 1, inactive_timeout = 3 },
})

-- El teclado del sistema (el que se eligió al instalar): con «us» una contraseña con ñ o símbolos no entra.
local function leer(ruta)
    local f = io.open(ruta)
    if not f then return nil end
    local t = f:read("a")
    f:close()
    return t
end
local function clave(texto, nombre)
    if not texto then return nil end
    local v = texto:match("\n%s*" .. nombre .. "=\"?([^\"\n]*)") or texto:match("^%s*" .. nombre .. "=\"?([^\"\n]*)")
    if v and v ~= "" then return v end
    return nil
end
local vc = leer("/etc/vconsole.conf")
local x11 = leer("/etc/X11/xorg.conf.d/00-keyboard.conf")
local layout = clave(vc, "XKBLAYOUT") or (x11 and x11:match('"XkbLayout"%s*"([^"]+)"'))
local variant = clave(vc, "XKBVARIANT") or (x11 and x11:match('"XkbVariant"%s*"([^"]+)"'))
if layout then
    hl.config({ input = { kb_layout = layout, kb_variant = variant or "", numlock_by_default = true } })
else
    hl.config({ input = { numlock_by_default = true } })
end

-- El login, ya con Hyprland arriba (antes de «hyprland.start» todavía no hay pantalla a la que conectarse).
-- Al terminar (entró alguien, o falló) se guarda cómo salió y se cierra este Hyprland.
hl.on("hyprland.start", function()
    hl.exec_cmd("sh -c 'quickshell -p " .. dir .. "/shell.qml; echo $? > \"$XDG_RUNTIME_DIR/vortex-greeter.rc\"; "
        .. "pkill -TERM -u \"$(id -u)\" -x Hyprland'")
end)
