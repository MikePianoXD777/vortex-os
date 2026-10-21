-- Vortex OS: variables de Hyprland/Caelestia.
-- Los valores base salen del perfil activo (cámbialo con «vortex perfil <nombre>»).
local home = os.getenv("HOME")
local ok, perfil = pcall(dofile, home .. "/.config/vortex/perfil.lua")
local vars = (ok and type(perfil) == "table" and perfil.vars) or {}

-- ¿Existe el comando en el PATH? (sin lanzar procesos)
local function existe(cmd)
    for dir in (os.getenv("PATH") or ""):gmatch("[^:]+") do
        local f = io.open(dir .. "/" .. cmd, "r")
        if f then f:close() return true end
    end
    return false
end

-- Valores por defecto de Vortex: el perfil y tus cambios los pueden pisar.
-- Editor (Super+C): Caelestia trae «codium», que Vortex no instala; el editor es Zed.
if vars.editor == nil then
    if existe("zeditor") then
        vars.editor = "zeditor"
    elseif existe("nvim") then
        vars.editor = "foot nvim"
    end
end

-- Suspender (Super+Shift+L y gesto de 4 dedos): Caelestia usa «suspend-then-hibernate»,
-- que logind rechaza («na») si no hay swap en disco (Vortex solo usa zram).
-- Solo se deja si logind lo permite; si no, se suspende normal.
if vars.sleepGestureCmd == nil then
    local p = io.popen("timeout 2 busctl call org.freedesktop.login1 /org/freedesktop/login1 "
        .. "org.freedesktop.login1.Manager CanSuspendThenHibernate 2>/dev/null")
    local r = p and p:read("*a") or ""
    if p then p:close() end
    if not r:find('"yes"', 1, true) then
        vars.sleepGestureCmd = "systemctl suspend"
    end
end

-- Tus cambios: lo que pongas aquí gana sobre el perfil.
-- Ejemplo: mios.terminal = "kitty"
local mios = {}
local ok_mios, extra = pcall(dofile, home .. "/.config/vortex/hypr-vars-mios.lua")
if ok_mios and type(extra) == "table" then mios = extra end

for k, v in pairs(mios) do
    vars[k] = v
end

return vars
