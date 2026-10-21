-- Vortex OS: variables de Hyprland/Caelestia.
-- Los valores base salen del perfil activo (cámbialo con «vortex perfil <nombre>»).
local home = os.getenv("HOME")
local ok, perfil = pcall(dofile, home .. "/.config/vortex/perfil.lua")
local vars = (ok and type(perfil) == "table" and perfil.vars) or {}

-- Tus cambios: lo que pongas aquí gana sobre el perfil.
-- Ejemplo: mios.terminal = "kitty"
local mios = {}
local ok_mios, extra = pcall(dofile, home .. "/.config/vortex/hypr-vars-mios.lua")
if ok_mios and type(extra) == "table" then mios = extra end

for k, v in pairs(mios) do
    vars[k] = v
end

return vars
