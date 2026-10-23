-- Vortexine's OS: config de Hyprland (~/.config/hypr/hyprland.lua).
-- Carga los módulos de Serpantinum desde el sistema (/usr/share/serpantinum), así sus
-- actualizaciones llegan solas, y encima pone lo de Vortex: los atajos de teclado
-- (atajos.lua, al lado de este archivo) y lo demás de este archivo. Tus cambios van en
-- ~/.config/vortex/hypr-mio.lua (se carga al final y gana sobre todo lo demás).
local home = os.getenv("HOME")
local serp = "/usr/share/serpantinum/compositors/hyprland"
package.path = serp .. "/?.lua;" .. package.path

-- Atajos de Serpantinum que Vortex usa para otra cosa (se dejan de registrar).
local reemplazados = {
    ["SUPER + A"] = true, -- autohide de la barra → Claude (va a SUPER + SHIFT + A)
    ["SUPER + E"] = true, -- nautilus
    ["SUPER + O"] = true, -- hoy libre en Serpantinum; si algún día lo usa, el atajo es de Miku (el asistente)
}

-- Atajos de teclado de Mike (los de su Caelestia, traducidos a Serpantinum). Si el archivo
-- falta o falla, el escritorio arranca igual con los atajos de Serpantinum y un aviso.
require("config/variables")
-- Aviso de que los atajos no cargaron. notify-send solo no basta: el servidor de notificaciones
-- es el propio shell, que arranca en este mismo evento, y si notify-send corre antes falla en
-- silencio. Por eso el aviso sale primero por Hyprland (no depende del shell) y después por
-- notify-send, que reintenta hasta que el shell esté arriba. Con un solo renglón del motivo: el
-- error de require lista todas las rutas probadas.
local function avisar_atajos(motivo)
    motivo = tostring(motivo):match("[^\n]*"):gsub("[^%w%s%.:/_%-]", ""):sub(1, 160)
    local texto = "Vortex: no se pudieron cargar los atajos de ~/.config/hypr/atajos.lua " ..
        "(prueba vortex configurar-usuario). " .. motivo
    hl.on("hyprland.start", function()
        pcall(function() hl.notification.create({ text = texto, timeout = 20000, color = "rgba(ff5555ff)" }) end)
        hl.exec_cmd("sh -c 'for i in $(seq 1 40); do notify-send -u critical \"Vortex: atajos\" \"" ..
            texto .. "\" && exit 0; sleep 1; done'")
    end)
end
local ok_atajos, atajos = pcall(require, "atajos")
-- Con un atajos.lua que no compila, require de Hyprland anota el error y devuelve algo vacío.
if not (ok_atajos and type(atajos) == "table" and atajos.reemplaza and atajos.registrar) then
    ok_atajos = false
    avisar_atajos(atajos)
    atajos = { reemplaza = function() return false end, registrar = function() end }
end

local bind_original = hl.bind
hl.bind = function(teclas, accion, opciones)
    if reemplazados[teclas] or atajos.reemplaza(teclas) then return end
    return bind_original(teclas, accion, opciones)
end

-- Comando que arranca el shell. Avisos del shell que no son errores: Qt avisa que la app ya
-- estaba registrada con el portal (el primer registro sí funcionó) y, sin adaptador
-- Bluetooth, que no existe org.bluez. Se silencian solo esas categorías y solo para el shell.
-- textinput: al cerrar un panel con un campo de texto, Qt avisa que desactiva la
-- entrada de texto de una superficie cuando el foco ya pasó a otra («disableSurface
-- … is focused»); es ruido del protocolo text-input-v3, la escritura funciona igual.
local function comando_shell()
    local reglas = "qt.qpa.services.warning=false;qt.qpa.wayland.textinput.warning=false"
    local bt = io.open("/sys/class/bluetooth/hci0/address")
    if bt then bt:close() else reglas = reglas .. ";quickshell.dbus.objectmanager.warning=false" end
    return "QT_LOGGING_RULES='" .. reglas .. "' serpantinumd start"
end
if ok_atajos then atajos.comando_shell = comando_shell end

require("config/env")
-- No se carga config/autostart: activa EasyEffects en cada inicio de sesión.
hl.on("hyprland.start", function()
    hl.exec_cmd("wl-paste --type text --watch cliphist store")
    hl.exec_cmd("wl-paste --type image --watch cliphist store")
    hl.exec_cmd(comando_shell())
    -- Primer inicio: pone el primer fondo de Vortex si todavía no hay ninguno.
    hl.exec_cmd("vortex primer-inicio")
    -- Gestor de recursos automático (prioridad a la ventana enfocada, modo juego solo y
    -- presión de memoria). «vortex gestor apagar» lo deshabilita.
    hl.exec_cmd("systemctl --user -q is-enabled vortex-gestor.service && systemctl --user start vortex-gestor.service")
    -- Solo en el live (/etc/vortex/live): que el teclado del escritorio siga al que se elige en la
    -- página «Teclado» de Calamares, que en Wayland solo se lo dice a localed. Escucha a D-Bus,
    -- no gasta CPU; en el sistema instalado la marca ya no existe y esto no hace nada.
    local live = io.open("/etc/vortex/live")
    if live then
        live:close()
        hl.exec_cmd("vortex-teclado-live")
    end
end)
-- Escala de los monitores: la de Serpantinum (1.0). No se pone scale = "auto" porque Hyprland lo
-- calcula solo con los píxeles por pulgada y cambiaría pantallas de 1080p: 1.5 desde 140 PPI (una
-- laptop de 15.6" o menos) y 2 si la pantalla no informa su tamaño (p. ej. algunas VMs). Quien tenga una
-- pantalla 2.8K/4K lo pone en ~/.config/vortex/hypr-mio.lua:
--   hl.monitor({ output = "", mode = "preferred", position = "auto", scale = "auto" })
require("config/monitors")
-- Cada pantalla a su frecuencia más alta (hasta 240 Hz) en su resolución máxima: «preferred» deja
-- muchos monitores de 144-240 Hz en 60. Lo hace bin/vortex-pantallas al entrar y al conectar un
-- monitor; no toca los que configuraste en hypr-mio.lua o en Ajustes → Pantalla.
hl.on("hyprland.start", function() hl.exec_cmd("vortex-pantallas") end)
hl.on("monitor.added", function() hl.exec_cmd("vortex-pantallas") end)
require("config/settings")
-- Vortexine's 1.2: ventanas más redondeadas, con curva suave (como el shell, borderRadius 18), y
-- un 15 % más de espacio entre ventanas y en las orillas (4 → 5 y 6 → 7 px).
hl.config({
    decoration = { rounding = 18, rounding_power = 2.5 },
    general = { gaps_in = 5, gaps_out = 7 },
})
require("config/keybinds")
hl.bind = bind_original

-- Iconos de las apps Qt del shell sin forzar un tema Qt a todo el sistema.
hl.env("QS_ICON_THEME", "Adwaita")

hl.config({
    ecosystem = { no_update_news = true, no_donation_nag = true },
    -- Los juegos con la regla «immediate» pueden ir sin vsync (menos latencia).
    general = { allow_tearing = true },
    -- allow_session_lock_restore: sin esto Hyprland rechaza el bloqueo nuevo si el shell murió con
    -- la pantalla bloqueada y Super+Alt+L (recuperar el bloqueo) no puede hacer nada.
    misc = { font_family = "Sans", allow_session_lock_restore = true },
})

-- Teclado del sistema (lo guarda el instalador; Serpantinum fija «us»).
local ok, teclado = pcall(dofile, home .. "/.config/vortex/teclado.lua")
if ok and type(teclado) == "string" then
    hl.config({ input = { kb_layout = teclado } })
elseif ok and type(teclado) == "table" then
    hl.config({ input = { kb_layout = teclado.layout, kb_variant = teclado.variant or "" } })
end

-- Juegos de Steam, Lutris y gamescope: sin vsync y sin que se bloquee la pantalla.
-- Ventanas como en Caelestia: un poco transparentes (95 %) con el fondo desenfocado detrás, menos
-- en pantalla completa y en los juegos (ahí, opacas). El modo juego apaga el desenfoque.
hl.config({ decoration = { blur = { ignore_opacity = true, popups = true, input_methods = true } } })
hl.window_rule({ match = { fullscreen = false }, opacity = "0.95 override" })
hl.window_rule({ match = { class = "^(steam_app_.*|gamescope)$" }, opacity = "1.0 override" })
hl.window_rule({ match = { class = "^(steam_app_.*|gamescope)$" }, immediate = true, idle_inhibit = "always" })

-- Miku, el asistente de voz (vortex-miku, app de Qt con app_id «vortex-miku»): una ventana
-- flotante sin marco que el atajo abre y cierra. Wayland no deja que la app se coloque ni se
-- ponga encima, así que eso lo hacen estas reglas: fija en todos los escritorios (pin), sin
-- borde, sombra, blur ni animación (el fondo de la ventana es transparente: el contorno del
-- personaje no debe llevar nada alrededor), sin quitarte el teclado al abrirse ni al darle clic,
-- y pegada al borde derecho justo debajo de la barra (que mide 44 px arriba: 40 de barra y 4 de
-- margen; 52 deja 8 de aire). No lleva «size»: la app fija su tamaño (460x760 con el 3D, 420x240
-- sin él) y lo cambia al reabrirse. Si la barra está en otro lado o es más gruesa, cambia el
-- segundo número de «move» (o muévela a mano: hyprctl dispatch movewindowpixel).
hl.window_rule({
    name = "vortex-miku",
    match = { class = "^vortex-miku$" },
    float = true,
    pin = true,
    no_initial_focus = true,
    no_focus = true,
    no_shadow = true,
    no_blur = true,
    no_dim = true,
    no_anim = true,
    border_size = 0,
    rounding = 0,
    move = "monitor_w-window_w-12 52",
})

-- Atajos de Vortex.
hl.bind("SUPER + A", hl.dsp.exec_cmd("vortex claude"))
hl.bind("SUPER + SHIFT + A", hl.dsp.event("serpantinum:toggle autohide"))
hl.bind("SUPER + G", hl.dsp.exec_cmd("vortex modo-juego alternar"))
hl.bind("SUPER + E", hl.dsp.exec_cmd("thunar"))
-- Vortexine's Shop (la tienda de apps; I de «instalar»). Super+S ya es el especial y Super+Shift+S, la captura.
hl.bind("SUPER + I", hl.dsp.exec_cmd("vortex-shop"))

-- Miku: abre el asistente de voz (o lo cierra del todo si ya está abierto). Super+O estaba libre
-- (ni Serpantinum ni atajos.lua lo usan); aun así está en «reemplazados», por si una actualización de
-- Serpantinum lo estrena.
hl.bind("SUPER + O", hl.dsp.exec_cmd("vortex-miku"))

-- Atajos de teclado de Mike (atajos.lua).
local ok_registro, fallo_registro = pcall(atajos.registrar)
if not ok_registro then avisar_atajos(fallo_registro) end

pcall(dofile, home .. "/.config/vortex/hypr-mio.lua")
