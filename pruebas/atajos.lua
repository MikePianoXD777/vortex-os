-- Prueba de config/hypr/hyprland.lua + atajos.lua fuera de Hyprland, con un «hl» falso que
-- anota lo que se registra. La corre pruebas/atajos.sh:  lua pruebas/atajos.lua RAIZ_DEL_REPO
--   1. ningún atajo queda registrado dos veces (cada pulsación haría dos cosas);
--   2. los atajos clave hacen lo que dice el README, y los de Serpantinum que chocaban se fueron;
--   3. la lógica de los atajos que son funciones: grupos de escritorios, escritorios especiales,
--      tamaños, imagen en imagen y reinicio del shell.
local RAIZ = arg[1] or "."

---------------------------------------------------------------------------------------------
-- hl falso
---------------------------------------------------------------------------------------------

local function describir(x)
    if type(x) == "function" then return "<función>" end
    if type(x) ~= "table" then return tostring(x) end
    if x.dsp then
        local partes = {}
        for _, a in ipairs(x.args) do partes[#partes + 1] = describir(a) end
        return x.dsp .. "(" .. table.concat(partes, ", ") .. ")"
    end
    local claves = {}
    for k in pairs(x) do claves[#claves + 1] = tostring(k) end
    table.sort(claves)
    local partes = {}
    for _, k in ipairs(claves) do
        local v = x[k]
        if v == nil then v = x[tonumber(k)] end
        partes[#partes + 1] = k .. "=" .. describir(v)
    end
    return "{" .. table.concat(partes, ", ") .. "}"
end

local function proxy(ruta)
    return setmetatable({}, {
        __index = function(_, k) return proxy(ruta == "" and k or (ruta .. "." .. k)) end,
        __call = function(_, ...) return { dsp = ruta, args = { ... } } end,
    })
end

local MODS = { super = "super", win = "super", windows = "super", mod4 = "super", meta = "super",
    ctrl = "ctrl", control = "ctrl", shift = "shift", alt = "alt", mod1 = "alt" }
local function normalizar(teclas)
    local partes = {}
    for parte in tostring(teclas):gmatch("[^+]+") do partes[#partes + 1] = (parte:gsub("%s+", "")):lower() end
    local tecla = table.remove(partes) or ""
    for i, m in ipairs(partes) do partes[i] = MODS[m] or m end
    table.sort(partes)
    return table.concat(partes, "+") .. "+" .. tecla
end

-- Lo que «ve» el escritorio falso (las pruebas lo cambian).
local estado = { escritorio = { id = 1 }, especial = nil, ventanas = {}, activa = nil }
local registro = { binds = {}, despachos = {}, ejecutados = {} }

local hl = { dsp = proxy("") }
function hl.bind(teclas, accion, opciones)
    registro.binds[#registro.binds + 1] = { teclas = teclas, accion = accion, opciones = opciones or {} }
end
function hl.dispatch(x) registro.despachos[#registro.despachos + 1] = x end
function hl.get_active_workspace() return estado.escritorio end
function hl.get_active_special_workspace() return estado.especial end
function hl.get_active_monitor() return { width = 1920, height = 1080, scale = 1, x = 0, y = 0 } end
function hl.get_active_window() return estado.activa end
function hl.get_windows() return estado.ventanas end
setmetatable(hl, { __index = function() return function() end end }) -- hl.on, hl.config, hl.env...
_G.hl = hl

---------------------------------------------------------------------------------------------
-- Cargar la config del repo (con el shell del repo, no el de /usr/share)
---------------------------------------------------------------------------------------------

package.path = RAIZ .. "/config/hypr/?.lua;" .. package.path
local f = assert(io.open(RAIZ .. "/config/hypr/hyprland.lua"))
local fuente = f:read("a")
f:close()
fuente = fuente:gsub('local serp = "[^"]*"', 'local serp = "' .. RAIZ .. '/shell/compositors/hyprland"', 1)
-- Estricto: si atajos.lua falla, la prueba falla (hyprland.lua se lo tragaría y avisaría).
fuente = fuente:gsub('pcall%(require, "atajos"%)', 'true, require("atajos")', 1)
fuente = fuente:gsub('pcall%(atajos%.registrar%)', 'true, atajos.registrar()', 1)
assert(load(fuente, "@hyprland.lua"))()

local binds = {} -- clave normalizada → descripción
local veces = {}
local acciones = {}
for _, b in ipairs(registro.binds) do
    local clave = normalizar(b.teclas)
    veces[clave] = (veces[clave] or 0) + 1
    binds[clave] = describir(b.accion)
    acciones[clave] = b.accion
end

---------------------------------------------------------------------------------------------
-- Pruebas
---------------------------------------------------------------------------------------------

local fallos = 0
local function ok(nombre, cond, extra)
    if cond then
        print("  ok  " .. nombre)
    else
        print("  MAL " .. nombre .. (extra and ("  →  " .. tostring(extra)) or ""))
        fallos = fallos + 1
    end
end

print("== Atajos registrados: " .. #registro.binds)
local duplicados = {}
for clave, n in pairs(veces) do if n > 1 then duplicados[#duplicados + 1] = clave end end
table.sort(duplicados)
ok("ningún atajo duplicado", #duplicados == 0, table.concat(duplicados, ", "))

print("== Qué hace cada atajo")
-- [atajo] = texto que debe aparecer en lo que hace
local esperado = {
    ["SUPER + SUPER_L"] = "toggle launcher", ["SUPER + Space"] = "toggle launcher",
    ["CTRL + ALT + Delete"] = "toggle system", ["SUPER + B"] = "toggle system",
    ["SUPER + N"] = "toggle notifications", ["SUPER + K"] = "toggle calendar",
    ["SUPER + V"] = "toggle clipboard", ["SUPER + ALT + V"] = "toggle clipboard",
    ["CTRL + ALT + V"] = "toggle volume", ["SUPER + H"] = "toggle guide",
    ["SUPER + SHIFT + N"] = "toggle network", ["SUPER + ALT + M"] = "toggle music",
    ["SUPER + SHIFT + W"] = "toggle wallpaper", ["SUPER + SHIFT + A"] = "toggle autohide",
    ["SUPER + A"] = "vortex claude", ["SUPER + G"] = "vortex modo-juego alternar",
    ["SUPER + E"] = "thunar", ["SUPER + W"] = "firefox", ["SUPER + T"] = "kitty",
    ["SUPER + C"] = "zeditor", ["SUPER + L"] = "serpantinum lock",
    ["SUPER + Q"] = "window.close", ["SUPER + F"] = "mode=fullscreen",
    ["SUPER + ALT + F"] = "mode=maximized", ["SUPER + ALT + Space"] = "window.float",
    ["CTRL + SUPER + Backslash"] = "window.center", ["SUPER + P"] = "window.pin", ["SUPER + Z"] = "window.drag", ["SUPER + X"] = "window.resize",
    ["SUPER + SHIFT + Left"] = "direction=left", ["SUPER + Left"] = "focus({direction=left})",
    ["CTRL + SUPER + Left"] = "workspace=-1", ["CTRL + SUPER + Right"] = "workspace=+1",
    ["SUPER + Page_Up"] = "workspace=-1", ["SUPER + mouse_down"] = "workspace=+1",
    ["CTRL + SUPER + mouse_down"] = "workspace=+10",
    ["CTRL + SUPER + SHIFT + Right"] = "window.move({workspace=+1})",
    ["SUPER + ALT + S"] = "special:special", ["CTRL + SUPER + SHIFT + Down"] = "e+0",
    ["ALT + TAB"] = "cycle_next", ["CTRL + ALT + TAB"] = "group.next", ["SUPER + Comma"] = "group.toggle",
    ["SUPER + U"] = "out_of_group", ["SUPER + SHIFT + Comma"] = "lock_active",
    ["Print"] = "serpantinum screenshot", ["SUPER + Print"] = "screenshot --full",
    ["ALT + Sys_Req"] = "--geometry", ["SUPER + SHIFT + S"] = "serpantinum screenshot",
    ["CTRL + ALT + R"] = "--record --full", ["SUPER + ALT + R"] = "--record --full",
    ["SUPER + SHIFT + ALT + R"] = "--backend wf-recorder", ["SUPER + SHIFT + C"] = "hyprpicker",
    ["CTRL + SUPER + Space"] = "playerctl play-pause", ["CTRL + SUPER + Equal"] = "playerctl next",
    ["CTRL + SUPER + Minus"] = "playerctl previous", ["CTRL + SUPER + Backspace"] = "playerctl stop",
    ["XF86AudioStop"] = "playerctl stop", ["SUPER + SHIFT + M"] = "mute-toggle",
    ["XF86AudioRaiseVolume"] = "10%+", ["XF86AudioLowerVolume"] = "10%-",
    ["XF86MonBrightnessUp"] = "brightness raise", ["XF86AudioMute"] = "mute-toggle",
    ["SUPER + SHIFT + L"] = "suspend-then-hibernate", ["CTRL + ALT + C"] = "clearNotifications",
    ["CTRL + SUPER + SHIFT + R"] = "serpantinum kill", ["SUPER + RETURN"] = "kitty",
}
local orden = {}
for k in pairs(esperado) do orden[#orden + 1] = k end
table.sort(orden)
for _, atajo in ipairs(orden) do
    local hace = binds[normalizar(atajo)]
    ok(atajo .. " → " .. esperado[atajo], hace and hace:find(esperado[atajo], 1, true), hace or "no está registrado")
end
-- Funciones: solo que existan.
for _, atajo in ipairs({ "SUPER + 1", "SUPER + 0", "SUPER + ALT + 5", "CTRL + SUPER + 2", "CTRL + SUPER + ALT + 3",
    "SUPER + S", "SUPER + M", "SUPER + D", "SUPER + R", "CTRL + SHIFT + Escape", "SUPER + Equal", "SUPER + ALT + Down",
    "SUPER + ALT + Backslash", "CTRL + SUPER + ALT + Backslash",
    "CTRL + SUPER + ALT + R", "SUPER + ALT + L" }) do
    ok(atajo .. " → función", type(acciones[normalizar(atajo)]) == "function")
end

print("== Atajos de Serpantinum que ya no deben estar")
local fuera = {
    { "SUPER + F", "firefox" }, { "SUPER + R", "serpantinum reload" }, { "SUPER + SPACE", "playerctl" },
    { "SUPER + D", "toggle launcher" }, { "SUPER + C", "toggle clipboard" }, { "SUPER + S", "toggle calendar" },
    { "SUPER + M", "toggle music" }, { "SUPER + W", "toggle wallpaper" }, { "SUPER + V", "toggle volume" },
    { "SUPER + N", "toggle network" }, { "SUPER + SHIFT + Left", "resize" }, { "CTRL + SUPER + Left", "direction" },
}
for _, par in ipairs(fuera) do
    local hace = binds[normalizar(par[1])] or ""
    ok(par[1] .. " ya no es «" .. par[2] .. "»", not hace:find(par[2], 1, true), hace)
end
ok("Ctrl+Super+Arriba ya no mueve la ventana", binds[normalizar("CTRL + SUPER + Up")] == nil)
ok("Ctrl+Super+Abajo ya no mueve la ventana", binds[normalizar("CTRL + SUPER + Down")] == nil)
ok("los paneles de Serpantinum que no chocaban siguen (Super+B, Super+H)",
    binds[normalizar("SUPER + B")] and binds[normalizar("SUPER + H")])
ok("Bloq Mayús / Bloq Num siguen avisando al shell", (binds[normalizar("Caps_Lock")] or ""):find("serpantinum:locks", 1, true))
ok("Super+Shift+número (mover ventana) sigue", type(acciones[normalizar("SUPER + SHIFT + 3")]) == "function")

---------------------------------------------------------------------------------------------
-- Lógica de las funciones
---------------------------------------------------------------------------------------------

local function pulsar(teclas)
    registro.despachos = {}
    local accion = acciones[normalizar(teclas)]
    assert(type(accion) == "function", teclas .. " no es una función")
    accion()
    local salida = {}
    for _, d in ipairs(registro.despachos) do salida[#salida + 1] = describir(d) end
    return table.concat(salida, " ; ")
end

print("== Escritorios (grupos de 10)")
estado.escritorio = { id = 1 }
ok("Super+3 desde el 1 va al 3 y cierra los paneles", pulsar("SUPER + 3") == "focus({workspace=3}) ; event(serpantinum:close)")
ok("Super+0 va al 10", pulsar("SUPER + 0"):find("workspace=10", 1, true))
estado.escritorio = { id = 12 }
ok("Super+3 desde el 12 va al 13", pulsar("SUPER + 3"):find("workspace=13", 1, true))
estado.escritorio = { id = 5 }
ok("Ctrl+Super+2 desde el 5 va al 15", pulsar("CTRL + SUPER + 2"):find("workspace=15", 1, true))
ok("Super+Alt+4 manda la ventana al 4", pulsar("SUPER + ALT + 4"):find("window.move({workspace=4})", 1, true))
estado.escritorio = { id = 10 }
ok("Ctrl+Super+1 desde el 10 no manda a un escritorio 0", pulsar("CTRL + SUPER + 1") == "")
estado.escritorio = nil
ok("sin escritorio activo no falla", pulsar("SUPER + 1") == "")

print("== Escritorios especiales")
estado.especial, estado.ventanas = nil, {}
ok("Super+S abre el especial genérico", pulsar("SUPER + S") == "workspace.toggle_special(special)")
estado.especial = { name = "special:music" }
ok("Super+S cierra el que esté abierto", pulsar("SUPER + S") == "workspace.toggle_special(music)")

estado.especial, estado.ventanas = nil, {}
local s = pulsar("SUPER + M")
ok("Super+M va a special:music", s:find("focus({workspace=special:music})", 1, true), s)
ok("Super+M lanza Spotify (Flatpak) en ese escritorio", s:find("flatpak run com.spotify.Client", 1, true), s)
estado.ventanas = { { class = "Spotify", initial_title = "Spotify", title = "x", workspace = { name = "3" } } }
s = pulsar("SUPER + M")
ok("con Spotify en otro escritorio lo trae", s:find("window.move", 1, true) and s:find("workspace=special:music", 1, true), s)
ok("…y no abre otro", not s:find("exec_cmd", 1, true), s)
estado.ventanas = { { class = "Spotify", initial_title = "Spotify", title = "x", workspace = { name = "special:music" } } }
ok("con Spotify ya ahí no mueve ni lanza", pulsar("SUPER + M") == "focus({workspace=special:music})")
estado.especial = { name = "special:music" }
ok("estando abierto lo esconde", pulsar("SUPER + M") == "workspace.toggle_special(music)")
estado.especial, estado.ventanas = nil, {}
ok("Ctrl+Shift+Esc lanza btop en kitty", pulsar("CTRL + SHIFT + Escape"):find("'kitty' '--class' 'btop' '--title' 'btop' 'btop'", 1, true))
ok("Super+D lanza discord", pulsar("SUPER + D"):find("'discord'", 1, true))
ok("Super+R lanza todoist", pulsar("SUPER + R"):find("'todoist'", 1, true))
estado.ventanas = { { class = "btop", title = "btop", workspace = { name = "special:sysmon" } } }
ok("con btop ya abierto no lanza otro", not pulsar("CTRL + SHIFT + Escape"):find("exec_cmd", 1, true))

print("== Ventanas")
estado.activa = { size = { x = 1000, y = 500 }, floating = false, address = "0xabc" }
ok("Super+= agranda el ancho 10 %", pulsar("SUPER + Equal") == "window.resize({relative=true, x=100.0, y=0.0})")
ok("Super+Shift+- achica el alto 10 %", pulsar("SUPER + SHIFT + Minus") == "window.resize({relative=true, x=0.0, y=-50.0})")
s = pulsar("SUPER + ALT + Backslash")
ok("imagen en imagen: flota, redimensiona, mueve y fija",
    s:find("^window.float%(%)") and s:find("window.resize", 1, true) and s:find("window.move", 1, true) and s:find("window.pin", 1, true), s)
s = pulsar("CTRL + SUPER + ALT + Backslash")
ok("tamaño normal: 55 × 70 % de la pantalla y centrar", s:find("x=1056", 1, true) and s:find("y=756", 1, true) and s:find("window.center", 1, true), s)
estado.activa = nil
ok("sin ventana activa no fallan", pulsar("SUPER + Equal") == "no_op()" and pulsar("SUPER + ALT + Backslash") == "")

print("== Shell")
s = pulsar("CTRL + SUPER + ALT + R")
ok("Ctrl+Super+Alt+R mata el shell y lo arranca", s:find("serpantinum kill; sleep .1; ", 1, true) and s:find("serpantinumd start", 1, true), s)
ok("el reinicio lleva las reglas de logs de Qt", s:find("QT_LOGGING_RULES=", 1, true), s)
s = pulsar("SUPER + ALT + L")
ok("Super+Alt+L arranca el shell y bloquea", s:find("serpantinumd start", 1, true) and s:find("serpantinum lock", 1, true), s)

print()
if fallos > 0 then
    print(fallos .. " pruebas fallaron.")
    os.exit(1)
end
print("Todas las pruebas pasaron.")
