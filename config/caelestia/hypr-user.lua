-- Vortex OS: añadidos a la config de Hyprland de Caelestia.
-- Para tus propios cambios usa ~/.config/vortex/hypr-mio.lua (se carga al final).
local home = os.getenv("HOME")
local ok, perfil = pcall(dofile, home .. "/.config/vortex/perfil.lua")
if not ok or type(perfil) ~= "table" then perfil = {} end

hl.config({
    animations = { enabled = perfil.animaciones ~= false },
    -- Caelestia ya marca los juegos con la regla «immediate»; esto la deja funcionar
    -- (menos latencia de entrada a cambio de posible tearing, solo en juegos).
    general = { allow_tearing = true },
})

-- Claude: abre una terminal conectada a Claude, que puede ver tu pantalla si lo permites.
hl.bind("SUPER + A", hl.dsp.exec_cmd("vortex claude"))
-- Modo juego a mano (gamemode lo activa solo al abrir un juego).
hl.bind("SUPER + G", hl.dsp.exec_cmd("vortex modo-juego alternar"))

pcall(dofile, home .. "/.config/vortex/hypr-mio.lua")
