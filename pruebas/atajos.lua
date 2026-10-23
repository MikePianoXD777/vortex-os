-- Prueba de config/hypr/hyprland.lua + atajos.lua fuera de Hyprland, con un «hl» falso que
-- anota lo que se registra. La corre pruebas/atajos.sh:  lua pruebas/atajos.lua RAIZ_DEL_REPO
--   1. ningún atajo queda registrado dos veces (cada pulsación haría dos cosas);
--   2. los atajos clave hacen lo que dice el README, y los de Serpantinum que chocaban se fueron;
--   3. la lógica de los atajos que son funciones: grupos de escritorios, escritorios especiales,
--      tamaños, imagen en imagen y reinicio del shell;
--   4. los comandos de shell de algunos atajos (Alt+Impr, Super+.), ejecutados de verdad con
--      programas falsos y un HOME temporal;
--   5. el respaldo de hyprland.lua cuando atajos.lua falta o falla: siguen los atajos de
--      Serpantinum y sale el aviso.
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
local estado = {
    escritorio = { id = 1 }, especial = nil, ventanas = {}, activa = nil,
    monitor = { width = 1920, height = 1080, scale = 1, x = 0, y = 0 },
}
local registro = { binds = {}, despachos = {}, eventos = {}, ejecutados = {}, avisos = {}, config = {}, reglas = {} }

local hl = { dsp = proxy("") }
function hl.bind(teclas, accion, opciones)
    registro.binds[#registro.binds + 1] = { teclas = teclas, accion = accion, opciones = opciones or {} }
end
function hl.dispatch(x) registro.despachos[#registro.despachos + 1] = x end
function hl.get_active_workspace() return estado.escritorio end
function hl.get_active_special_workspace() return estado.especial end
function hl.get_active_monitor() return estado.monitor end
function hl.get_active_window() return estado.activa end
function hl.get_windows() return estado.ventanas end
function hl.on(evento, cb)
    registro.eventos[evento] = registro.eventos[evento] or {}
    table.insert(registro.eventos[evento], cb)
end
function hl.config(valores) registro.config[#registro.config + 1] = valores end
function hl.window_rule(regla) registro.reglas[#registro.reglas + 1] = regla end
function hl.exec_cmd(comando) registro.ejecutados[#registro.ejecutados + 1] = comando end
hl.notification = { create = function(opciones) registro.avisos[#registro.avisos + 1] = opciones end }
setmetatable(hl, { __index = function() return function() end end }) -- hl.env, hl.window_rule...
_G.hl = hl

---------------------------------------------------------------------------------------------
-- Cargar la config del repo (con el shell del repo, no el de /usr/share)
---------------------------------------------------------------------------------------------

local PATH_ORIGINAL = package.path

-- Carga hyprland.lua del repo (con el shell del repo, no el de /usr/share). Estricto: si
-- atajos.lua falla, la prueba falla (hyprland.lua se lo tragaría y avisaría); con
-- estricto = false se carga tal cual, como en Hyprland. carpeta_atajos: de dónde sale atajos.lua.
-- carpeta_serp: de dónde sale Serpantinum (por defecto el del repo).
local function cargar_config(estricto, carpeta_atajos, carpeta_serp)
    for nombre in pairs(package.loaded) do
        if nombre == "atajos" or nombre:match("^config/") then package.loaded[nombre] = nil end
    end
    registro.binds, registro.despachos, registro.eventos, registro.ejecutados, registro.avisos = {}, {}, {}, {}, {}
    registro.config = {}
    registro.reglas = {}
    package.path = (carpeta_atajos and (carpeta_atajos .. "/?.lua;") or "") .. PATH_ORIGINAL
    local f = assert(io.open(RAIZ .. "/config/hypr/hyprland.lua"))
    local fuente = f:read("a")
    f:close()
    fuente = fuente:gsub('local serp = "[^"]*"', 'local serp = "' .. (carpeta_serp or (RAIZ .. "/shell/compositors/hyprland")) .. '"', 1)
    if estricto then
        fuente = fuente:gsub('pcall%(require, "atajos"%)', 'true, require("atajos")', 1)
        fuente = fuente:gsub('pcall%(atajos%.registrar%)', 'true, atajos.registrar()', 1)
    end
    assert(load(fuente, "@hyprland.lua"))()
end

cargar_config(true, RAIZ .. "/config/hypr")

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
local restaurar_bloqueo = false
for _, valores in ipairs(registro.config) do
    if valores.misc and valores.misc.allow_session_lock_restore == true then restaurar_bloqueo = true end
end
ok("misc.allow_session_lock_restore activo (sin él Super+Alt+L no puede recuperar el bloqueo)", restaurar_bloqueo)

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
    ["SUPER + O"] = "exec_cmd(vortex-miku)",
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
    ["Print"] = "vortex-captura", ["SUPER + Print"] = "vortex-captura pantalla",
    ["ALT + Sys_Req"] = "vortex-captura ventana", ["SUPER + SHIFT + S"] = "vortex-captura",
    ["CTRL + ALT + R"] = "--record --full", ["SUPER + ALT + R"] = "--record --full",
    ["SUPER + SHIFT + ALT + R"] = "--backend wf-recorder", ["SUPER + SHIFT + C"] = "hyprpicker",
    ["CTRL + SUPER + Space"] = "playerctl play-pause", ["CTRL + SUPER + Equal"] = "playerctl next",
    ["CTRL + SUPER + Minus"] = "playerctl previous", ["CTRL + SUPER + Backspace"] = "playerctl stop",
    ["XF86AudioStop"] = "playerctl stop", ["SUPER + SHIFT + M"] = "mute-toggle",
    ["XF86AudioRaiseVolume"] = "10%+", ["XF86AudioLowerVolume"] = "10%-",
    ["XF86MonBrightnessUp"] = "brightness raise", ["XF86AudioMute"] = "mute-toggle",
    ["SUPER + SHIFT + L"] = "suspend-then-hibernate", ["CTRL + ALT + C"] = "clearNotifications",
    ["CTRL + SUPER + SHIFT + R"] = "serpantinum kill", ["SUPER + RETURN"] = "kitty",
    -- De Serpantinum, sin cambios (no chocan con nada de Caelestia).
    ["CTRL + SUPER + Up"] = "direction=u", ["CTRL + SUPER + Down"] = "direction=d",
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
    "CTRL + SUPER + ALT + R", "SUPER + ALT + L", "SUPER + Period", "SUPER + SHIFT + 3", "SUPER + SHIFT + 0" }) do
    ok(atajo .. " → función", type(acciones[normalizar(atajo)]) == "function")
end

-- Miku, el asistente de voz: el atajo y las reglas de su ventana (clase «vortex-miku»).
print("== Miku (Super+O y su ventana)")
do
    local miku
    for _, regla in ipairs(registro.reglas) do
        if regla.match and regla.match.class == "^vortex-miku$" then miku = regla end
    end
    ok("hay una regla de ventana para la clase vortex-miku", miku ~= nil)
    if miku then
        ok("flotante y fija en todos los escritorios", miku.float == true and miku.pin == true)
        ok("sin borde, sombra, blur ni esquinas redondeadas",
            miku.border_size == 0 and miku.no_shadow == true and miku.no_blur == true and miku.rounding == 0)
        ok("opaca al 100 % (gana a la regla de todas las ventanas al 95 %)", miku.opacity == "1.0 override")
        ok("no roba el teclado al abrirse", miku.no_initial_focus == true and miku.no_focus == true)
        ok("pegada al borde derecho, debajo de la barra (move = «monitor_w-window_w-N Y»; 7 de gaps_out y 41 = barra de 34 + 7)",
            type(miku.move) == "string" and miku.move:match("^monitor_w%-window_w%-%d+ %d+$") ~= nil, miku.move)
        ok("sin «size»: la app fija su tamaño (460x760 con el 3D, 420x240 sin él)", miku.size == nil)
    end
    -- Super+O estaba libre: si atajos.lua lo usara, saldría registrado dos veces y «ningún atajo
    -- duplicado» (arriba) fallaría. Si una actualización de Serpantinum lo estrenara, el filtro de
    -- «reemplazados» de hyprland.lua lo deja fuera: eso se prueba más abajo, con un Serpantinum que sí lo registra.
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
ok("los paneles de Serpantinum que no chocaban siguen (Super+B, Super+H)",
    binds[normalizar("SUPER + B")] and binds[normalizar("SUPER + H")])
ok("Bloq Mayús / Bloq Num siguen avisando al shell", (binds[normalizar("Caps_Lock")] or ""):find("serpantinum:locks", 1, true))

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
ok("Super+3 desde el 10 (último del grupo 1) va al 3", pulsar("SUPER + 3"):find("workspace=3", 1, true))
ok("Ctrl+Super+1 desde el 10 se queda en el 10 (posición 10 del grupo 1)", pulsar("CTRL + SUPER + 1"):find("workspace=10", 1, true))
ok("Ctrl+Super+2 desde el 10 va al 20", pulsar("CTRL + SUPER + 2"):find("workspace=20", 1, true))
estado.escritorio = { id = 20 }
ok("Super+3 desde el 20 (último del grupo 2) va al 13", pulsar("SUPER + 3"):find("workspace=13", 1, true))
ok("Ctrl+Super+1 desde el 20 va al 10", pulsar("CTRL + SUPER + 1"):find("workspace=10", 1, true))
ok("Ctrl+Super+3 desde el 20 va al 30", pulsar("CTRL + SUPER + 3"):find("workspace=30", 1, true))
ok("Ctrl+Super+Alt+1 desde el 20 manda la ventana al 10", pulsar("CTRL + SUPER + ALT + 1"):find("window.move({workspace=10})", 1, true))
estado.escritorio = { id = 12 }
ok("Super+Shift+3 manda la ventana al 13 (por grupo, como Super+Alt+3)",
    pulsar("SUPER + SHIFT + 3") == "window.move({workspace=13}) ; event(serpantinum:close)")
estado.escritorio = { id = 1 }
ok("Super+Shift+0 desde el 1 manda la ventana al 10", pulsar("SUPER + SHIFT + 0"):find("window.move({workspace=10})", 1, true))
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
estado.monitor = { width = 1920, height = 1080, scale = 2, x = 0, y = 0 }
s = pulsar("CTRL + SUPER + ALT + Backslash")
ok("tamaño normal con escala 2: 55 × 70 % de la pantalla lógica (528 × 378)", s:find("x=528", 1, true) and s:find("y=378", 1, true), s)
estado.monitor = { width = 1920, height = 1080, scale = 1.5, x = 0, y = 0 }
s = pulsar("CTRL + SUPER + ALT + Backslash")
ok("tamaño normal con escala 1.5 (704 × 504)", s:find("x=704", 1, true) and s:find("y=504", 1, true), s)
estado.monitor = { width = 1920, height = 1080, x = 0, y = 0 }
s = pulsar("CTRL + SUPER + ALT + Backslash")
ok("tamaño normal sin escala informada se toma como 1", s:find("x=1056", 1, true) and s:find("y=756", 1, true), s)
estado.monitor = { width = 1920, height = 1080, scale = 1, x = 0, y = 0 }
estado.activa = nil
ok("sin ventana activa no fallan", pulsar("SUPER + Equal") == "no_op()" and pulsar("SUPER + ALT + Backslash") == "")

print("== Shell")
s = pulsar("CTRL + SUPER + ALT + R")
ok("Ctrl+Super+Alt+R mata el shell y lo arranca", s:find("serpantinum kill; sleep .1; ", 1, true) and s:find("serpantinumd start", 1, true), s)
ok("el reinicio lleva las reglas de logs de Qt", s:find("QT_LOGGING_RULES=", 1, true), s)
s = pulsar("SUPER + ALT + L")
ok("Super+Alt+L arranca el shell y bloquea", s:find("serpantinumd start", 1, true) and s:find("serpantinum lock", 1, true), s)

---------------------------------------------------------------------------------------------
-- Comandos de shell (se ejecutan de verdad, con programas falsos y un HOME temporal)
---------------------------------------------------------------------------------------------

local function mktemp()
    local p = assert(io.popen("mktemp -d"))
    local d = p:read("l")
    p:close()
    return d
end
local function escribir(ruta, texto)
    local f = assert(io.open(ruta, "w"))
    f:write(texto)
    f:close()
end
local function leer(ruta)
    local f = io.open(ruta)
    if not f then return "" end
    local t = f:read("a")
    f:close()
    return t
end
local TEMP = mktemp()
os.execute("mkdir -p '" .. TEMP .. "/bin' '" .. TEMP .. "/home'")
-- Un programa falso que anota cómo lo llamaron en TEMP/llamadas. «fallos»: las primeras N
-- veces sale con error (para probar los reintentos).
local function programa_falso(nombre, fallos_iniciales)
    escribir(TEMP .. "/bin/" .. nombre, string.format([[#!/bin/sh
echo "%s $*" >> "%s/llamadas"
n=$(cat "%s/cuenta-%s" 2>/dev/null || echo 0)
echo $((n + 1)) > "%s/cuenta-%s"
[ "$n" -ge %d ]
]], nombre, TEMP, TEMP, nombre, TEMP, nombre, fallos_iniciales or 0))
    os.execute("chmod +x '" .. TEMP .. "/bin/" .. nombre .. "'")
end
-- Corre un comando como lo haría Hyprland (sh -c) y devuelve las llamadas a programas falsos.
local function correr(comando)
    os.remove(TEMP .. "/llamadas")
    os.execute("rm -f '" .. TEMP .. "'/cuenta-*")
    escribir(TEMP .. "/comando.sh", comando)
    os.execute(string.format("env PATH='%s/bin':\"$PATH\" HOME='%s/home' XDG_CACHE_HOME='%s/home/.cache' sh '%s/comando.sh' >/dev/null 2>&1",
        TEMP, TEMP, TEMP, TEMP))
    return leer(TEMP .. "/llamadas")
end
local function comando_de_atajo(teclas)
    local accion = acciones[normalizar(teclas)]
    return accion and accion.args and accion.args[1]
end

print("== Alt+Impr (captura de la ventana activa)")
-- La lógica (sin ventana activa avisa y no captura) vive en bin/vortex-captura.
ok("Alt+Impr llama a vortex-captura ventana", comando_de_atajo("ALT + Sys_Req") == "vortex-captura ventana", comando_de_atajo("ALT + Sys_Req"))

print("== Super+. (selector de emojis)")
s = pulsar("SUPER + Period")
ok("Super+. escribe la pestaña 2 y abre el lanzador, en ese orden",
    s:match("^exec_cmd%(.*last_tab%.txt%) ; event%(serpantinum:toggle launcher%)$"), s)
ok("…en la carpeta de caché que vigila el lanzador", s:find("~/.cache/serpantinum/launcher/last_tab.txt", 1, true), s)
local escribe_pestana = registro.despachos[1] and registro.despachos[1].args[1]
correr(escribe_pestana or "false")
ok("el comando deja un 2 en last_tab.txt", leer(TEMP .. "/home/.cache/serpantinum/launcher/last_tab.txt") == "2\n",
    leer(TEMP .. "/home/.cache/serpantinum/launcher/last_tab.txt"))
-- Y funciona aunque la carpeta ya exista y tenga otra pestaña.
escribir(TEMP .. "/home/.cache/serpantinum/launcher/last_tab.txt", "0\n")
correr(escribe_pestana or "false")
ok("…también si ya había otra pestaña guardada", leer(TEMP .. "/home/.cache/serpantinum/launcher/last_tab.txt") == "2\n")

---------------------------------------------------------------------------------------------
-- Respaldo de hyprland.lua
---------------------------------------------------------------------------------------------

print("== Si atajos.lua falta o falla")
local function arrancar() -- lo que hace Hyprland al iniciar sesión
    for _, cb in ipairs(registro.eventos["hyprland.start"] or {}) do cb() end
end
local function comandos_con(texto)
    local halladas = {}
    for _, c in ipairs(registro.ejecutados) do if c:find(texto, 1, true) then halladas[#halladas + 1] = c end end
    return halladas
end
local function respaldo(titulo, carpeta, extra)
    local cargo, error_carga = pcall(cargar_config, false, carpeta)
    ok(titulo .. ": la config carga igual", cargo, error_carga)
    if not cargo then return end
    local nuevos = {}
    for _, b in ipairs(registro.binds) do nuevos[normalizar(b.teclas)] = describir(b.accion) end
    ok(titulo .. ": siguen los atajos de Serpantinum (Super+D abre el lanzador)",
        (nuevos[normalizar("SUPER + D")] or ""):find("toggle launcher", 1, true), nuevos[normalizar("SUPER + D")])
    ok(titulo .. ": siguen los de Vortex (Super+A habla con Claude)",
        (nuevos[normalizar("SUPER + A")] or ""):find("vortex claude", 1, true), nuevos[normalizar("SUPER + A")])
    ok(titulo .. ": no queda ningún atajo de Mike (Super+W sigue en los fondos)",
        (nuevos[normalizar("SUPER + W")] or ""):find("wallpaper", 1, true), nuevos[normalizar("SUPER + W")])
    arrancar()
    local aviso = registro.avisos[1]
    ok(titulo .. ": avisa con el aviso de Hyprland al iniciar", aviso and tostring(aviso.text):find("no se pudieron cargar los atajos", 1, true)
        and (aviso.timeout or 0) >= 5000, aviso and aviso.text)
    ok(titulo .. ": el aviso es de una sola línea y corto", aviso and not aviso.text:find("\n", 1, true) and #aviso.text < 400,
        aviso and #aviso.text)
    local envios = comandos_con("notify-send")
    ok(titulo .. ": también por notify-send, con reintentos (el shell tarda en tener servidor de notificaciones)",
        #envios == 1 and envios[1]:find("seq", 1, true) and envios[1]:find("sleep", 1, true), envios[1])
    if extra then extra(envios[1]) end
end

local vacia = TEMP .. "/sin-atajos"
os.execute("mkdir -p '" .. vacia .. "'")
respaldo("sin atajos.lua", vacia)

local rota = TEMP .. "/atajos-rota"
os.execute("mkdir -p '" .. rota .. "'")
escribir(rota .. "/atajos.lua", "esto no es lua (\n")
respaldo("atajos.lua con error de sintaxis", rota)

local falla = TEMP .. "/atajos-falla"
os.execute("mkdir -p '" .. falla .. "'")
escribir(falla .. "/atajos.lua", 'return { reemplaza = function() return false end, registrar = function() error("boom") end }\n')
respaldo("atajos.lua que falla al registrar", falla, function(comando)
    -- El comando de notify-send, ejecutado de verdad: reintenta hasta que funcione.
    programa_falso("notify-send", 2)
    local llamadas = correr(comando)
    local _, veces_notify = llamadas:gsub("notify%-send", "")
    ok("el aviso por notify-send reintenta hasta que hay servidor (falla 2 veces, sale a la 3.ª)", veces_notify == 3, llamadas)
    ok("…y el texto llega entero", llamadas:find("Vortex: atajos", 1, true) and llamadas:find("no se pudieron cargar los atajos", 1, true), llamadas)
end)

-- Un Serpantinum que estrena Super+O: Miku (Vortex) gana, y su atajo no sale dos veces.
print("== Un Serpantinum que estrena Super+O")
-- Copia de Serpantinum a cuyos atajos se les suma un Super+O.
local estreno = TEMP .. "/serp-estreno"
os.execute("mkdir -p '" .. estreno .. "' && cp -r '" .. RAIZ .. "/shell/compositors/hyprland/.' '" .. estreno .. "/'")
local keybinds = assert(io.open(estreno .. "/config/keybinds.lua", "a"))
keybinds:write('\nhl.bind("SUPER + O", hl.dsp.exec_cmd("serpantinum-estreno"))\n')
keybinds:close()
local estreno_ok, estreno_error = pcall(cargar_config, true, RAIZ .. "/config/hypr", estreno)
ok("la config carga con ese Serpantinum", estreno_ok, estreno_error)
if estreno_ok then
    local o = {}
    for _, b in ipairs(registro.binds) do
        if normalizar(b.teclas) == normalizar("SUPER + O") then o[#o + 1] = describir(b.accion) end
    end
    ok("Super+O queda registrado una sola vez", #o == 1, #o)
    ok("…y es el de Miku, no el de Serpantinum", o[1] and o[1]:find("vortex-miku", 1, true) and not o[1]:find("estreno", 1, true), o[1])
end

-- Con atajos.lua sano no hay ningún aviso.
local sana_ok, sana_error = pcall(cargar_config, true, RAIZ .. "/config/hypr")
ok("con atajos.lua sano la config carga", sana_ok, sana_error)
arrancar()
ok("…y no avisa de nada", #registro.avisos == 0 and #comandos_con("notify-send") == 0)

os.execute("rm -rf '" .. TEMP .. "'")

print()
if fallos > 0 then
    print(fallos .. " pruebas fallaron.")
    os.exit(1)
end
print("Todas las pruebas pasaron.")
