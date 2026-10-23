-- Vortexine's OS: atajos de teclado (~/.config/hypr/atajos.lua).
-- Son los de Mike en CachyOS con Caelestia (keybinds.lua, variables.lua y sus cambios en
-- hypr-vars.lua / hypr-user.lua), con cada acción traducida a su equivalente en Serpantinum:
-- lanzador y paneles por evento, «serpantinum screenshot», bloqueo, sesión, etc. Los
-- monitores, el ratón y las reglas de juegos de su máquina NO se copian.
--
-- hyprland.lua carga este archivo antes de los atajos de Serpantinum: los de Serpantinum que
-- chocan con uno de aquí se dejan de registrar (reemplaza) y el resto siguen como estaban.
-- Hay atajos que ya eran iguales en Serpantinum y no se repiten: Super+Q (cerrar),
-- Super+flechas (enfocar), Ctrl+Super+arriba/abajo (mover la ventana), Super+L (bloquear),
-- Super+Alt+F (maximizar), Super+clic izquierdo/derecho (mover/redimensionar), Impr (captura
-- de región), Super+Impr (pantalla completa) y las teclas de brillo, mute, micrófono y
-- reproducción. Los de Vortex (Super+A, Super+Shift+A, Super+G, Super+E) están en hyprland.lua.
-- Lo tuyo va en ~/.config/vortex/hypr-mio.lua (se carga al final).
local M = {}

-- Porcentaje que suben y bajan las teclas de volumen (en Caelestia: volumeStep = 10, tope 100).
local PASO_VOLUMEN = 10

-- Devuelve el comando que arranca el shell; hyprland.lua pone el suyo (lleva las reglas de
-- logs de Qt).
M.comando_shell = function() return "serpantinumd start" end

---------------------------------------------------------------------------------------------
-- Registro de atajos
---------------------------------------------------------------------------------------------

local MODIFICADORES = {
    super = "super", win = "super", windows = "super", mod4 = "super", meta = "super",
    ctrl = "ctrl", control = "ctrl", shift = "shift", alt = "alt", mod1 = "alt",
}

-- «CTRL + SUPER + Up» y «super+ctrl+up» son el mismo atajo: modificadores ordenados + tecla.
local function normalizar(teclas)
    local partes = {}
    for parte in tostring(teclas):gmatch("[^+]+") do
        partes[#partes + 1] = (parte:gsub("%s+", "")):lower()
    end
    local tecla = table.remove(partes) or ""
    for i, m in ipairs(partes) do partes[i] = MODIFICADORES[m] or m end
    table.sort(partes)
    return table.concat(partes, "+") .. "+" .. tecla
end

local lista = {}       -- { teclas, acción, opciones } en el orden en que se declaran
local reemplazados = {} -- atajos (normalizados) de Serpantinum que dejan de registrarse

local function atajo(teclas, accion, opciones)
    if type(teclas) == "table" then
        for _, t in ipairs(teclas) do atajo(t, accion, opciones) end
        return
    end
    lista[#lista + 1] = { teclas, accion, opciones }
    reemplazados[normalizar(teclas)] = true
end

-- Como atajo(), pero se repite al mantener la tecla (menos con la rueda del ratón).
local function atajo_repetible(teclas, accion)
    if type(teclas) ~= "table" then teclas = { teclas } end
    for _, t in ipairs(teclas) do
        if t:lower():find("mouse", 1, true) then
            atajo(t, accion)
        else
            atajo(t, accion, { repeating = true })
        end
    end
end

-- ¿Este atajo de Serpantinum choca con uno de aquí? (lo usa el filtro de hyprland.lua)
function M.reemplaza(teclas)
    return reemplazados[normalizar(teclas)] == true
end

-- Registra todo en Hyprland (hyprland.lua llama a esto con hl.bind ya sin filtro).
function M.registrar()
    for _, a in ipairs(lista) do
        hl.bind(a[1], a[2], a[3])
    end
end

---------------------------------------------------------------------------------------------
-- Ayudas
---------------------------------------------------------------------------------------------

local bloqueado = { locked = true }
local bloqueado_repetible = { locked = true, repeating = true }
local al_soltar = { release = true }
local con_mouse = { mouse = true }

local function ejecutar(comando) return hl.dsp.exec_cmd(comando) end

-- Los paneles de Serpantinum se abren por evento (Main.qml), sin lanzar procesos.
local function panel(nombre) return hl.dsp.event("serpantinum:toggle " .. nombre) end
local cerrar_paneles = hl.dsp.event("serpantinum:close")

-- Escritorio N dentro del grupo de 10 escritorios del actual (Caelestia agrupa de 10 en 10:
-- Super+1..0 va al escritorio 1..10 del grupo; Ctrl+Super+1..0 salta de grupo conservando
-- la posición). En el primer grupo es igual que Super+número de Serpantinum. Como él, cierra
-- los paneles abiertos (evento «serpantinum:close»). Super+Shift+número, el de Serpantinum
-- para mandar la ventana, también va por grupo (igual que Super+Alt+número): el de siempre
-- la mandaba al escritorio absoluto y en el grupo 2 la ventana desaparecía de la vista.
local function wsaction(accion, rango, i)
    return function()
        local activo = hl.get_active_workspace()
        if not activo then return end
        local id = activo.id
        -- Posición dentro del grupo, de 1 a 10 (el 10, el 20, el 30… son la posición 10, no la 0).
        local posicion = ((id - 1) % 10) + 1
        local destino = (rango == "group") and ((i - 1) * 10 + posicion)
            or (math.floor((id - 1) / 10) * 10 + i)
        if destino < 1 then return end
        if accion == "move" then
            hl.dispatch(hl.dsp.window.move({ workspace = destino }))
        else
            hl.dispatch(hl.dsp.focus({ workspace = destino }))
        end
        hl.dispatch(cerrar_paneles)
    end
end

-- Tamaño en % de la pantalla del monitor activo. width y height son píxeles físicos: las
-- ventanas se miden en píxeles lógicos, así que se divide entre la escala del monitor.
local function tamano_por_pantalla(x, y)
    local pantalla = hl.get_active_monitor()
    if pantalla and type(pantalla.width) == "number" and type(pantalla.height) == "number" then
        if not (x == 0 and y == 0) then
            local escala = (type(pantalla.scale) == "number" and pantalla.scale > 0) and pantalla.scale or 1
            local ancho, alto = pantalla.width / escala, pantalla.height / escala
            local w = (x and x > 0) and math.floor(ancho * x / 100) or math.floor(ancho)
            local h = (y and y > 0) and math.floor(alto * y / 100) or math.floor(alto)
            return { x = w, y = h, relative = false }
        end
    end
end

-- Cambia el tamaño de la ventana activa un % de su tamaño (x, y en %).
local function redimensionar(x, y)
    return function()
        local ventana = hl.get_active_window()
        if ventana and ventana.size then
            hl.dispatch(hl.dsp.window.resize({
                x = ventana.size.x * (x / 100), y = ventana.size.y * (y / 100), relative = true,
            }))
        else
            hl.dispatch(hl.dsp.no_op())
        end
    end
end

-- Imagen en imagen: ventana flotante chica, fija, en la esquina de abajo a la derecha.
local function acciones_pip(ventana)
    local pantalla = hl.get_active_monitor()
    if pantalla and pantalla.width and pantalla.height and ventana and ventana.size then
        local alto = pantalla.height / pantalla.scale
        local ancho = pantalla.width / pantalla.scale
        local escala = (alto / 4) / ventana.size.y
        local w = math.floor(math.max(200, ventana.size.x * escala))
        local h = math.floor(math.max(150, ventana.size.y * escala))
        local margen = math.min(ancho, alto) * 0.03
        return {
            hl.dsp.window.resize({ x = w, y = h, window = ventana }),
            hl.dsp.window.move({
                x = math.floor(pantalla.x + ancho - w - margen),
                y = math.floor(pantalla.y + alto - h - margen),
                relative = false, window = ventana,
            }),
        }
    end
end

---------------------------------------------------------------------------------------------
-- Escritorios especiales con apps (Caelestia: «toggle» de caelestia-cli, con sus valores por
-- defecto). Al abrir uno se lanza la app si no está corriendo y se traen ahí sus ventanas.
-- Si la app no está instalada queda el escritorio vacío. Cambios: hypr-mio.lua.
---------------------------------------------------------------------------------------------

local especiales = {
    communication = {
        discord = { match = { { class = "discord" } }, command = { "discord" }, move = true },
        whatsapp = { match = { { class = "whatsapp" } }, move = true },
    },
    music = {
        spotify = {
            match = { { class = "Spotify" }, { class = "spotify" }, { initial_title = "Spotify" }, { initial_title = "Spotify Free" } },
            -- En Vortex Spotify es el Flatpak del instalador (en Caelestia: spicetify watch -s).
            command = { "sh", "-c", "command -v spotify >/dev/null && exec spotify; exec flatpak run com.spotify.Client" },
            move = true,
        },
        feishin = { match = { { class = "feishin" } }, move = true },
    },
    sysmon = {
        btop = {
            match = { { class = "btop", title = "btop", workspace = { name = "special:sysmon" } } },
            -- Caelestia usaba foot; en Vortex la terminal es kitty.
            command = { "kitty", "--class", "btop", "--title", "btop", "btop" },
        },
    },
    todo = {
        todoist = { match = { { class = "todoist" } }, command = { "todoist" }, move = true },
    },
}

local function campo(objeto, clave)
    local valor = objeto[clave]
    if valor == nil and type(clave) == "string" then
        valor = objeto[(clave:gsub("(%u)", "_%1")):lower()] -- initialTitle → initial_title
    end
    return valor
end

local function coincide(real, esperado)
    if type(esperado) == "table" then
        if type(real) ~= "table" and type(real) ~= "userdata" then return false end
        for clave, sub in pairs(esperado) do
            if not coincide(campo(real, clave), sub) then return false end
        end
        return true
    end
    return real and string.find(tostring(real), tostring(esperado), 1, true)
end

-- Ventanas abiertas que son de esta app, y si ya están en el escritorio especial.
local function ventanas_de(app, ventanas, especial)
    local halladas = {}
    for _, ventana in ipairs(ventanas) do
        for _, regla in ipairs(app.match or {}) do
            local es = true
            for clave, esperado in pairs(regla) do
                if not coincide(campo(ventana, clave), esperado) then es = false; break end
            end
            if es then
                local escritorio = ventana.workspace and ventana.workspace.name
                halladas[#halladas + 1] = { ventana = ventana, en_su_sitio = (escritorio == "special:" .. especial) }
                break
            end
        end
    end
    return halladas
end

local function comando_de(argv)
    local partes = {}
    for i, arg in ipairs(argv) do
        partes[i] = "'" .. (tostring(arg):gsub("'", [['"'"']])) .. "'"
    end
    return table.concat(partes, " ")
end

local function alternar_especial(nombre)
    return function()
        local activo = hl.get_active_special_workspace()

        -- Escritorio especial genérico: cierra el que esté abierto o abre «special».
        if nombre == "specialws" then
            local destino = activo and (activo.name:gsub("^special:", "")) or "special"
            return hl.dispatch(hl.dsp.workspace.toggle_special(destino))
        end

        local ya_abierto = activo and activo.name == "special:" .. nombre
        if not ya_abierto then
            hl.dispatch(hl.dsp.focus({ workspace = "special:" .. nombre }))
        end

        local destino = "special:" .. nombre
        local ventanas = hl.get_windows() or {}
        for _, app in pairs(especiales[nombre] or {}) do
            local halladas = ventanas_de(app, ventanas, nombre)
            if #halladas == 0 then
                if app.command then
                    hl.dispatch(hl.dsp.exec_cmd(comando_de(app.command), { workspace = destino }))
                end
            elseif app.move then
                for _, h in ipairs(halladas) do
                    if not h.en_su_sitio then
                        hl.dispatch(hl.dsp.window.move({ window = h.ventana, workspace = destino, follow = false }))
                    end
                end
            end
        end

        if ya_abierto then
            hl.dispatch(hl.dsp.workspace.toggle_special(nombre))
        end
    end
end

---------------------------------------------------------------------------------------------
-- Lanzador, paneles y sesión
---------------------------------------------------------------------------------------------

-- Super solo (al soltar) y Super+Espacio: lanzador (Caelestia: launcher).
atajo("SUPER + SUPER_L", panel("launcher"), al_soltar)
atajo("SUPER + Space", panel("launcher"))
-- Menú de sesión (Caelestia: session): en Serpantinum es el panel de sistema (bloquear,
-- suspender, hibernar, reiniciar, apagar y cerrar sesión).
atajo("CTRL + ALT + Delete", panel("system"))
-- Barra lateral con las notificaciones (Caelestia: sidebar).
atajo("SUPER + N", panel("notifications"))
atajo("CTRL + ALT + C", ejecutar("serpantinum ipc call main clearNotifications"), bloqueado)
-- «Mostrar todos los paneles» (Caelestia: showall) no existe; lo más parecido es el panel
-- grande de calendario, reloj y clima.
atajo("SUPER + K", panel("calendar"))
-- Historial del portapapeles (Caelestia: fuzzel). En el panel, Supr borra la entrada elegida.
atajo({ "SUPER + V", "SUPER + ALT + V" }, panel("clipboard"))
-- Ajustes de audio (Caelestia: pwvucontrol): el panel de volumen y dispositivos.
atajo("CTRL + ALT + V", panel("volume"))

-- Paneles de Serpantinum cuyo atajo de siempre ahora es de otra cosa en Caelestia: red (era
-- Super+N), música (Super+M) y fondos (Super+W) pasan a Super+Shift+N, Super+Alt+M y
-- Super+Shift+W. El calendario (era Super+S) es Super+K; el lanzador (Super+D), el portapapeles
-- (Super+C) y el volumen (Super+V) tienen su atajo de Caelestia, arriba.
atajo("SUPER + SHIFT + N", panel("network"))
atajo("SUPER + ALT + M", panel("music"))
-- Los fondos siguen por «serpantinum msg»: ese camino prepara las miniaturas.
atajo("SUPER + SHIFT + W", ejecutar("serpantinum msg toggle wallpaper"))

-- Reinicio del shell: Ctrl+Super+Shift+R lo mata y Ctrl+Super+Alt+R lo mata y lo vuelve a
-- arrancar (antes era Super+R, que ahora es el escritorio de tareas). Ambos al soltar.
atajo("CTRL + SUPER + SHIFT + R", ejecutar("serpantinum kill"), al_soltar)
atajo("CTRL + SUPER + ALT + R", function()
    hl.dispatch(ejecutar("serpantinum kill; sleep .1; " .. M.comando_shell()))
end, al_soltar)
-- Recuperar el bloqueo (Caelestia: restoreLock): si el shell murió con la pantalla bloqueada,
-- lo arranca y vuelve a bloquear (reintenta mientras levanta).
atajo("SUPER + ALT + L", function()
    hl.dispatch(ejecutar("(" .. M.comando_shell() .. ") >/dev/null 2>&1 & " ..
        "for i in 1 2 3 4 5 6 7 8 9 10; do serpantinum lock && break; sleep 0.5; done"))
end)

---------------------------------------------------------------------------------------------
-- Escritorios y ventanas
---------------------------------------------------------------------------------------------

for i = 1, 10 do
    local tecla = i % 10 -- el 10 es la tecla 0
    atajo("SUPER + " .. tecla, wsaction("focus", "", i))
    atajo({ "SUPER + ALT + " .. tecla, "SUPER + SHIFT + " .. tecla }, wsaction("move", "", i))
    atajo("CTRL + SUPER + " .. tecla, wsaction("focus", "group", i))
    atajo("CTRL + SUPER + ALT + " .. tecla, wsaction("move", "group", i))
end

-- Escritorio anterior/siguiente y grupo anterior/siguiente.
atajo_repetible({ "SUPER + mouse_up", "CTRL + SUPER + Left", "SUPER + Page_Up" }, hl.dsp.focus({ workspace = "-1" }))
atajo_repetible({ "SUPER + mouse_down", "CTRL + SUPER + Right", "SUPER + Page_Down" }, hl.dsp.focus({ workspace = "+1" }))
atajo_repetible("CTRL + SUPER + mouse_up", hl.dsp.focus({ workspace = "-10" }))
atajo_repetible("CTRL + SUPER + mouse_down", hl.dsp.focus({ workspace = "+10" }))

-- Mover la ventana al escritorio anterior/siguiente.
atajo_repetible({ "SUPER + ALT + mouse_up", "SUPER + ALT + Page_Up", "CTRL + SUPER + SHIFT + Left" },
    hl.dsp.window.move({ workspace = "-1" }))
atajo_repetible({ "SUPER + ALT + mouse_down", "SUPER + ALT + Page_Down", "CTRL + SUPER + SHIFT + Right" },
    hl.dsp.window.move({ workspace = "+1" }))

-- Mover la ventana al escritorio especial y sacarla de él.
atajo({ "SUPER + ALT + S", "CTRL + SUPER + SHIFT + Up" }, hl.dsp.window.move({ workspace = "special:special" }))
atajo("CTRL + SUPER + SHIFT + Down", hl.dsp.window.move({ workspace = "e+0" }))

-- Ciclar ventanas y grupos; agrupar.
atajo("ALT + TAB", hl.dsp.window.cycle_next(), { repeating = true })
atajo("SHIFT + ALT + TAB", hl.dsp.window.cycle_next({ next = false }), { repeating = true })
atajo("CTRL + ALT + TAB", hl.dsp.group.next(), { repeating = true })
atajo("CTRL + SHIFT + ALT + TAB", hl.dsp.group.prev(), { repeating = true })
atajo("SUPER + Comma", hl.dsp.group.toggle())
atajo("SUPER + U", hl.dsp.window.move({ out_of_group = true }))
atajo("SUPER + SHIFT + Comma", hl.dsp.group.lock_active())

-- Mover la ventana con las flechas (enfocar con Super+flechas es de Serpantinum). En
-- Serpantinum mover era Ctrl+Super+flechas (izq./der. ahora cambian de escritorio) y
-- Super+Shift+flechas redimensionaba (redimensionar es Super+Alt+flechas). Ctrl+Super+
-- arriba/abajo no chocan con nada de Caelestia y se quedan como estaban (mover la ventana).
for tecla, direccion in pairs({ Left = "left", Right = "right", Up = "up", Down = "down" }) do
    atajo("SUPER + SHIFT + " .. tecla, hl.dsp.window.move({ direction = direccion }))
end

-- Redimensionar.
atajo_repetible({ "SUPER + Minus", "SUPER + ALT + Left" }, redimensionar(-10, 0))
atajo_repetible({ "SUPER + Equal", "SUPER + ALT + Right" }, redimensionar(10, 0))
atajo_repetible({ "SUPER + SHIFT + Minus", "SUPER + ALT + Up" }, redimensionar(0, -10))
atajo_repetible({ "SUPER + SHIFT + Equal", "SUPER + ALT + Down" }, redimensionar(0, 10))

-- Mover y redimensionar con el teclado + ratón (con el ratón solo ya lo hace Serpantinum).
atajo("SUPER + Z", hl.dsp.window.drag(), con_mouse)
atajo("SUPER + X", hl.dsp.window.resize(), con_mouse)

atajo("CTRL + SUPER + Backslash", hl.dsp.window.center())
atajo("CTRL + SUPER + ALT + Backslash", function()
    local tamano = tamano_por_pantalla(55, 70)
    if tamano then hl.dispatch(hl.dsp.window.resize(tamano)) end
    hl.dispatch(hl.dsp.window.center())
end)
atajo("SUPER + ALT + Backslash", function()
    local ventana = hl.get_active_window()
    if not ventana then return end
    local pip = acciones_pip(ventana) or {}
    if not ventana.floating then table.insert(pip, 1, hl.dsp.window.float()) end
    table.insert(pip, hl.dsp.window.pin({ action = "on", window = "address:" .. ventana.address }))
    for _, accion in ipairs(pip) do hl.dispatch(accion) end
end)
atajo("SUPER + P", hl.dsp.window.pin())
atajo("SUPER + F", hl.dsp.window.fullscreen({ mode = "fullscreen" }))
atajo("SUPER + ALT + Space", hl.dsp.window.float())

---------------------------------------------------------------------------------------------
-- Escritorios especiales
---------------------------------------------------------------------------------------------

atajo("SUPER + S", alternar_especial("specialws"))
atajo("CTRL + SHIFT + Escape", alternar_especial("sysmon"))
atajo("SUPER + M", alternar_especial("music"))
atajo("SUPER + D", alternar_especial("communication"))
atajo("SUPER + R", alternar_especial("todo"))

---------------------------------------------------------------------------------------------
-- Apps
---------------------------------------------------------------------------------------------

atajo("SUPER + T", ejecutar(_G.terminal or "kitty"))
atajo("SUPER + W", ejecutar("firefox"))
-- Caelestia abría VS Codium; en Vortex el editor es Zed (el binario de Arch es «zeditor»).
atajo("SUPER + C", ejecutar("command -v zeditor >/dev/null && exec zeditor; exec zed"))

---------------------------------------------------------------------------------------------
-- Capturas, grabación y colores
---------------------------------------------------------------------------------------------

-- Capturas directas, como en Caelestia (bin/vortex-captura): sin preguntar nada, al portapapeles
-- y a Imágenes/Capturas. Impr y Super+Shift+S = región (congela toda la pantalla y se elige con el
-- ratón), Super+Impr = el monitor del ratón, Alt+Impr = la ventana activa (con Alt, Impr manda
-- Sys_Req). Shift+Impr sigue abriendo el editor de Serpantinum, y su panel (grabar video, etc.)
-- queda en Ctrl+Alt+R / Super+Alt+R.
atajo({ "Print", "SUPER + SHIFT + S", "SUPER + SHIFT + ALT + S" }, ejecutar("vortex-captura"))
atajo("SUPER + Print", ejecutar("vortex-captura pantalla"))
atajo("ALT + Sys_Req", ejecutar("vortex-captura ventana"))

-- Grabar la pantalla (se vuelve a pulsar para parar): sin sonido, con el sonido del equipo y
-- una región. Caelestia usaba gpu-screen-recorder igual; la región usa wf-recorder.
atajo("CTRL + ALT + R", ejecutar("serpantinum screenshot --record --full --desk-mute true --mic-mute true"))
atajo("SUPER + ALT + R", ejecutar("serpantinum screenshot --record --full --mic-mute true"))
atajo("SUPER + SHIFT + ALT + R", ejecutar(
    "pidof gpu-screen-recorder wf-recorder >/dev/null && exec serpantinum screenshot; " ..
    'region="$(slurp)" || exit 0; ' ..
    'exec serpantinum screenshot --record --backend wf-recorder --geometry "$region" --desk-mute true --mic-mute true'))

atajo("SUPER + SHIFT + C", ejecutar("hyprpicker -a"))

---------------------------------------------------------------------------------------------
-- Sonido y reproducción
---------------------------------------------------------------------------------------------

atajo("XF86AudioRaiseVolume", ejecutar(
    "wpctl set-mute @DEFAULT_AUDIO_SINK@ 0; wpctl set-volume -l 1.0 @DEFAULT_AUDIO_SINK@ " .. PASO_VOLUMEN .. "%+"),
    bloqueado_repetible)
atajo("XF86AudioLowerVolume", ejecutar(
    "wpctl set-mute @DEFAULT_AUDIO_SINK@ 0; wpctl set-volume @DEFAULT_AUDIO_SINK@ " .. PASO_VOLUMEN .. "%-"),
    bloqueado_repetible)
atajo("SUPER + SHIFT + M", ejecutar("serpantinum volume mute-toggle"), bloqueado)

atajo("CTRL + SUPER + Space", ejecutar("playerctl play-pause"), bloqueado)
atajo("CTRL + SUPER + Equal", ejecutar("playerctl next"), bloqueado)
atajo("CTRL + SUPER + Minus", ejecutar("playerctl previous"), bloqueado)
atajo({ "CTRL + SUPER + Backspace", "XF86AudioStop" }, ejecutar("playerctl stop"), bloqueado)

---------------------------------------------------------------------------------------------
-- Otros
---------------------------------------------------------------------------------------------

-- Dormir (Caelestia: suspend-then-hibernate). Bloquea primero, como el botón de dormir del
-- panel de sistema; si el equipo no puede hibernar, suspende normal.
atajo("SUPER + SHIFT + L", ejecutar(
    "serpantinum lock; sleep 0.5; systemctl suspend-then-hibernate || systemctl suspend"), bloqueado)

-- Pegar lo último del portapapeles escribiéndolo (necesita ydotool y su servicio ydotoold).
atajo("CTRL + SHIFT + ALT + V", ejecutar(
    "command -v ydotool >/dev/null || { notify-send -u low 'Pegar lo último' 'Falta ydotool (y su servicio ydotoold)'; exit 1; }; " ..
    [[sleep 0.5s && ydotool type -d 1 "$(cliphist list | head -1 | cliphist decode)"]]), bloqueado)

-- Notificación de prueba (la de Caelestia, para ver cómo se ven los avisos largos y sus botones).
atajo("SUPER + ALT + F12", ejecutar(
    "notify-send -u low -i dialog-information-symbolic 'Notificación de prueba' " ..
    [["Un aviso largo para probar cómo se corta y se ajusta\nSe cierra con clic medio o deslizándolo"]] ..
    " -a 'Shell' -A 'Test1=¡Listo!' -A 'Test2=Otra acción'"))

-- Selector de emojis (Caelestia: «caelestia emoji -p», que se cierra al repetir el atajo). En
-- Serpantinum es la pestaña «Emojis» del lanzador: al elegir uno lo copia (wl-copy) y se cierra.
-- El lanzador vigila ~/.cache/serpantinum/launcher/last_tab.txt (0 aplicaciones, 1 archivos,
-- 2 emojis), así que se escribe un 2 ahí y se abre. Como con cualquier cambio de pestaña, el
-- lanzador la recuerda: Super abrirá en Emojis hasta que vuelvas a Aplicaciones con Tab.
atajo("SUPER + Period", function()
    hl.dispatch(ejecutar("mkdir -p ~/.cache/serpantinum/launcher && " ..
        "echo 2 > ~/.cache/serpantinum/launcher/last_tab.txt"))
    hl.dispatch(panel("launcher"))
end)

return M
