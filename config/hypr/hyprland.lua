-- Vortex OS: config de Hyprland (~/.config/hypr/hyprland.lua).
-- Carga los módulos de Serpantinum desde el sistema (/usr/share/serpantinum), así sus
-- actualizaciones llegan solas, y encima pone lo de Vortex. Tus cambios van en
-- ~/.config/vortex/hypr-mio.lua (se carga al final y gana sobre todo lo demás).
local home = os.getenv("HOME")
local serp = "/usr/share/serpantinum/compositors/hyprland"
package.path = serp .. "/?.lua;" .. package.path

-- Atajos de Serpantinum que Vortex usa para otra cosa (se dejan de registrar).
local reemplazados = {
    ["SUPER + A"] = true, -- autohide de la barra → Claude (va a SUPER + SHIFT + A)
    ["SUPER + E"] = true, -- nautilus
}
local bind_original = hl.bind
hl.bind = function(teclas, accion, opciones)
    if reemplazados[teclas] then return end
    return bind_original(teclas, accion, opciones)
end

require("config/variables")
require("config/env")
-- No se carga config/autostart: activa EasyEffects en cada inicio de sesión.
hl.on("hyprland.start", function()
    hl.exec_cmd("wl-paste --type text --watch cliphist store")
    hl.exec_cmd("wl-paste --type image --watch cliphist store")
    hl.exec_cmd("serpantinumd start")
end)
require("config/monitors")
require("config/settings")
require("config/keybinds")
hl.bind = bind_original

-- Iconos de las apps Qt del shell sin forzar un tema Qt a todo el sistema.
hl.env("QS_ICON_THEME", "Adwaita")

hl.config({
    ecosystem = { no_update_news = true, no_donation_nag = true },
    -- Los juegos con la regla «immediate» pueden ir sin vsync (menos latencia).
    general = { allow_tearing = true },
    misc = { font_family = "Sans" },
})

-- Teclado del sistema (lo guarda el instalador; Serpantinum fija «us»).
local ok, teclado = pcall(dofile, home .. "/.config/vortex/teclado.lua")
if ok and type(teclado) == "string" then
    hl.config({ input = { kb_layout = teclado } })
elseif ok and type(teclado) == "table" then
    hl.config({ input = { kb_layout = teclado.layout, kb_variant = teclado.variant or "" } })
end

-- Juegos de Steam, Lutris y gamescope: sin vsync y sin que se bloquee la pantalla.
hl.window_rule({ match = { class = "^(steam_app_.*|gamescope)$" }, immediate = true, idle_inhibit = "always" })

-- Atajos de Vortex.
hl.bind("SUPER + A", hl.dsp.exec_cmd("vortex claude"))
hl.bind("SUPER + SHIFT + A", hl.dsp.exec_cmd("serpantinum msg toggle autohide"))
hl.bind("SUPER + G", hl.dsp.exec_cmd("vortex modo-juego alternar"))
hl.bind("SUPER + E", hl.dsp.exec_cmd("thunar"))

pcall(dofile, home .. "/.config/vortex/hypr-mio.lua")
