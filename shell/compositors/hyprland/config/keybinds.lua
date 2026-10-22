local mainMod = _G.mainMod or "SUPER"
local terminal = _G.terminal or "kitty"

hl.gesture({ fingers = 3, direction = "horizontal", action = "workspace" })
hl.bind(mainMod .. " + mouse:272", hl.dsp.window.drag(), { mouse = true })
hl.bind(mainMod .. " + mouse:273", hl.dsp.window.resize(), { mouse = true })

hl.bind(mainMod .. " + SHIFT + Left", hl.dsp.window.resize({ x = -50, y = 0, relative = true }), { repeating = true })
hl.bind(mainMod .. " + SHIFT + Right", hl.dsp.window.resize({ x = 50, y = 0, relative = true }), { repeating = true })
hl.bind(mainMod .. " + SHIFT + Up", hl.dsp.window.resize({ x = 0, y = -50, relative = true }), { repeating = true })
hl.bind(mainMod .. " + SHIFT + Down", hl.dsp.window.resize({ x = 0, y = 50, relative = true }), { repeating = true })

hl.bind(mainMod .. " + CTRL + Left", hl.dsp.window.move({ direction = "l" }))
hl.bind(mainMod .. " + CTRL + Right", hl.dsp.window.move({ direction = "r" }))
hl.bind(mainMod .. " + CTRL + Up", hl.dsp.window.move({ direction = "u" }))
hl.bind(mainMod .. " + CTRL + Down", hl.dsp.window.move({ direction = "d" }))

hl.bind(mainMod .. " + Left", hl.dsp.focus({ direction = "left" }))
hl.bind(mainMod .. " + Right", hl.dsp.focus({ direction = "right" }))
hl.bind(mainMod .. " + Up", hl.dsp.focus({ direction = "up" }))
hl.bind(mainMod .. " + Down", hl.dsp.focus({ direction = "down" }))

hl.bind(mainMod .. " + Q", hl.dsp.window.close())
hl.bind(mainMod .. " + ALT + F", hl.dsp.window.fullscreen({ mode = "maximized", action = "toggle" }))
hl.bind(mainMod .. " + SHIFT + F", hl.dsp.window.float({ action = "toggle" }))

hl.bind("XF86MonBrightnessDown", hl.dsp.exec_cmd("serpantinum brightness lower"), { locked = true })
hl.bind("XF86MonBrightnessUp", hl.dsp.exec_cmd("serpantinum brightness raise"), { locked = true })

hl.bind("Print", hl.dsp.exec_cmd("serpantinum screenshot"), { locked = true })
hl.bind("SHIFT + Print", hl.dsp.exec_cmd("serpantinum screenshot --edit"), { locked = true })
hl.bind("SUPER + Print", hl.dsp.exec_cmd("serpantinum screenshot --full"), { locked = true })
hl.bind("SUPER + SHIFT + Print", hl.dsp.exec_cmd("serpantinum screenshot --full --edit"), { locked = true })

hl.bind("XF86PowerOff", hl.dsp.exec_cmd("serpantinum lock"), { locked = true })
hl.bind(mainMod .. " + L", hl.dsp.exec_cmd("serpantinum lock"), { repeating = true, locked = true })

hl.bind(mainMod .. " + SPACE", hl.dsp.exec_cmd("playerctl play-pause"), { locked = true })
hl.bind("XF86AudioPause", hl.dsp.exec_cmd("playerctl play-pause"), { locked = true })
hl.bind("XF86AudioPlay", hl.dsp.exec_cmd("playerctl play-pause"), { locked = true })
hl.bind("XF86AudioPrev", hl.dsp.exec_cmd("playerctl previous"), { locked = true })
hl.bind("XF86AudioNext", hl.dsp.exec_cmd("playerctl next"), { locked = true })
hl.bind("XF86AudioMicMute", hl.dsp.exec_cmd("serpantinum volume mic-toggle"), { locked = true })
hl.bind("XF86AudioMute", hl.dsp.exec_cmd("serpantinum volume mute-toggle"), { locked = true })
hl.bind("XF86AudioLowerVolume", hl.dsp.exec_cmd("serpantinum volume lower"), { repeating = true, locked = true })
hl.bind("XF86AudioRaiseVolume", hl.dsp.exec_cmd("serpantinum volume raise"), { repeating = true, locked = true })

hl.bind(mainMod .. " + F", hl.dsp.exec_cmd("firefox"))
hl.bind(mainMod .. " + E", hl.dsp.exec_cmd("nautilus"))
hl.bind(mainMod .. " + RETURN", hl.dsp.exec_cmd(terminal))

hl.bind(mainMod .. " + R", hl.dsp.exec_cmd("serpantinum reload"))
-- Vortex: los paneles se abren con el evento «custom>>serpantinum:toggle NOMBRE», que
-- Main.qml oye por el socket de eventos que Quickshell ya tiene abierto. Antes cada
-- pulsación lanzaba «serpantinum msg» (bash, su config, qs_manager.sh, python y un
-- cliente de quickshell: ~50 procesos y 50-75 ms antes de que el panel empezara a abrir).
-- Los fondos siguen por «serpantinum msg»: ese camino prepara las miniaturas.
local function panel(nombre)
  return hl.dsp.event("serpantinum:toggle " .. nombre)
end
hl.bind(mainMod .. " + C", panel("clipboard"))
hl.bind(mainMod .. " + D", panel("launcher"))
hl.bind(mainMod .. " + M", panel("music"))
hl.bind(mainMod .. " + B", panel("system"))
hl.bind(mainMod .. " + W", hl.dsp.exec_cmd("serpantinum msg toggle wallpaper"))
hl.bind(mainMod .. " + S", panel("calendar"))
hl.bind(mainMod .. " + N", panel("network"))
hl.bind(mainMod .. " + V", panel("volume"))
hl.bind(mainMod .. " + H", panel("guide"))
hl.bind(mainMod .. " + A", panel("autohide"))

-- Vortex: SUPER+número va directo a Hyprland. Antes «serpantinum msg workspace N»
-- lanzaba en cada pulsación bash, qs_manager.sh, hyprctl y un cliente de quickshell
-- (para cerrar los popups). Ahora el evento «custom>>serpantinum:close» le pide al
-- shell que cierre sus popups (Main.qml), igual que hacía qs_manager.sh.
local close_popups = hl.dsp.event("serpantinum:close")
-- Vortex: Bloq Mayús y Bloq Num avisan al shell para su OSD (singletons/widgetcontrols/
-- OsdController.qml). El atajo no se queda con la tecla (non_consuming) y funciona con
-- cualquier modificador y con la pantalla bloqueada; así el shell no tiene que vigilar
-- los LED del teclado con un proceso residente.
local avisar_bloqueos = hl.dsp.event("serpantinum:locks")
for _, tecla in ipairs({ "Caps_Lock", "Num_Lock" }) do
  hl.bind(tecla, avisar_bloqueos, { non_consuming = true, locked = true, ignore_mods = true })
end

for i = 1, 10 do
  local ws = tostring(i)
  local key = tostring(i % 10)
  local go_to = hl.dsp.focus({ workspace = ws })
  local move_to = hl.dsp.window.move({ workspace = ws })
  hl.bind(mainMod .. " + " .. key, function()
    hl.dispatch(go_to)
    hl.dispatch(close_popups)
  end)
  hl.bind(mainMod .. " + SHIFT + " .. key, function()
    hl.dispatch(move_to)
    hl.dispatch(close_popups)
  end)
end
