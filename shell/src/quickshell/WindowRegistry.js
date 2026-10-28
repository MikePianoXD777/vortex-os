.pragma library

function getScale(mw, mh, userScale) {
    return (userScale !== undefined && userScale !== null) ? userScale : 1.0;
}

function s(val, scale) {
    return Math.round(val * scale);
}

function getWidgetLauncherEntries(i18n) {
    function tr(key, fallback) {
        if (i18n && typeof i18n.t === "function") {
            let res = i18n.t(key);
            if (res && res !== key) return res;
        }
        return fallback;
    }

    return [
        {
            id: "wallpaper",
            name: tr("widgets.wallpaper.name", "Wallpaper Picker"),
            description: tr("widgets.wallpaper.desc", "Browse and set desktop wallpaper"),
            icon: "preferences-desktop-wallpaper",
            fontIcon: "󰸉"
        },
        {
            id: "network",
            name: tr("widgets.network.name", "Network Settings"),
            description: tr("widgets.network.desc", "Manage WiFi, Ethernet, and network connections"),
            icon: "network-wireless",
            fontIcon: "󰤨"
        },
        {
            id: "volume",
            name: tr("widgets.volume.name", "Volume & Audio"),
            description: tr("widgets.volume.desc", "Manage sound devices and volume levels"),
            icon: "audio-volume-high",
            fontIcon: "󰕾"
        },
        {
            id: "guide",
            name: tr("widgets.guide.name", "Settings"),
            description: tr("widgets.guide.desc", "Serpantinum settings"),
            icon: "help-browser",
            fontIcon: "󰋖"
        },
        {
            id: "calendar",
            name: tr("widgets.calendar.name", "Calendar & Clock"),
            description: tr("widgets.calendar.desc", "View date, time, and schedule"),
            icon: "x-office-calendar",
            fontIcon: "󰸗"
        },
        {
            id: "music",
            name: tr("widgets.music.name", "Media Player"),
            description: tr("widgets.music.desc", "Control music and media playback"),
            icon: "multimedia-audio-player",
            fontIcon: "󰝚"
        },
        {
            // Vortex 1.10: temporizador de estudio (el singleton Estudio; la cuenta sale en la barra).
            id: "estudio",
            name: tr("widgets.estudio.name", "Study timer"),
            description: tr("widgets.estudio.desc", "Pomodoro: study blocks with breaks, with no notifications while you study"),
            icon: "chronometer",
            fontIcon: "󰔛"
        },
        {
            // Vortex (1.7.7): la hoja de atajos (Super+F1), para buscarla en el lanzador.
            id: "keybinds",
            name: tr("widgets.keybinds.name", "Keyboard shortcuts"),
            description: tr("widgets.keybinds.desc", "Every Vortexine's OS shortcut"),
            icon: "preferences-desktop-keyboard-shortcuts",
            fontIcon: "󰌌"
        },
        {
            // Vortex 1.10: OneDrive como la carpeta ~/OneDrive (vortex-onedrive).
            id: "onedrive",
            name: "OneDrive",
            description: tr("widgets.onedrive.desc", "Your OneDrive files as a folder (Microsoft, Office)"),
            icon: "folder-remote",
            fontIcon: "󰏊"
        },
        {
            // Vortex 1.10: metrónomo y afinador (musica/musica.py). La descripción lleva las palabras
            // con las que se buscaría: tempo, compás, afinar, guitarra, solfeo.
            id: "musica",
            name: tr("widgets.musica.name", "Metronome and tuner"),
            description: tr("widgets.musica.desc", "Tempo, time signature and tap; tune your voice or instrument (solfège, guitar)"),
            icon: "audio-x-generic",
            fontIcon: "󰝚"
        },
        {
            // Vortex 1.9: «Algo no funciona» (vortex-arreglar). La descripción lleva las palabras
            // con las que alguien lo buscaría: sonido, internet, wifi…
            id: "arreglar",
            name: tr("widgets.arreglar.name", "Something's not working"),
            description: tr("widgets.arreglar.desc", "Check and fix sound, internet, Wi-Fi, Bluetooth, disk and time"),
            icon: "system-help",
            fontIcon: "󰁨"
        },
        {
            // Vortex 1.10: Vortexine Restore (vortex-restore), puntos de restauración de la configuración.
            id: "restore",
            name: tr("widgets.restore.name", "Vortexine Restore"),
            description: tr("widgets.restore.desc", "Restore points: go back to an earlier Hyprland, theme, bar or widgets if something breaks"),
            icon: "document-revert",
            fontIcon: "󰁯"
        },
        {
            // Vortex 1.10: tus widgets del escritorio en HTML sencillo (htmlwidgets/).
            id: "htmlwidgets",
            name: tr("widgets.htmlwidgets.name", "HTML widgets"),
            description: tr("widgets.htmlwidgets.desc", "Program your own desktop widgets in simple HTML, with a live preview"),
            icon: "text-html",
            fontIcon: "󰌝"
        },
        {
            // Vortex 1.10: una nota para apuntar algo al vuelo (notas/), también con Super+Alt+N.
            id: "notas",
            name: tr("widgets.notas.name", "Quick note"),
            description: tr("widgets.notas.desc", "Jot something down on the fly; it saves itself (Super+Alt+N)"),
            icon: "accessories-text-editor",
            fontIcon: "󰎞"
        },
        {
            // Vortex 1.10: buscaminas tryhard (buscaminas/), idea de THYRVAREN.
            id: "buscaminas",
            name: tr("widgets.buscaminas.name", "Tryhard Minesweeper"),
            description: tr("widgets.buscaminas.desc", "Minesweeper with records, 3BV/s and modes to suffer: fog, against the clock, no flags"),
            icon: "applications-games",
            fontIcon: "\u{F0691}"
        },
        {
            // Vortex 1.10: mods de Minecraft de Modrinth y CurseForge a la instancia que elijas (minecraft/).
            id: "modrinth",
            name: tr("widgets.minecraft.name", "Minecraft mods"),
            description: tr("widgets.minecraft.desc", "Install Modrinth and CurseForge mods into Prism, TLauncher or any Minecraft folder"),
            icon: "applications-games",
            fontIcon: "󰍳"
        },
        {
            // Vortex 1.8: la biblioteca de juegos (Super+Shift+G).
            id: "juegos",
            name: tr("widgets.juegos.name", "Your games"),
            description: tr("widgets.juegos.desc", "Steam, Heroic and your other games, with covers"),
            icon: "applications-games",
            fontIcon: "󰊴"
        },
        {
            // Vortex 1.8: tu celular con KDE Connect (Super+Shift+P).
            id: "celular",
            name: tr("widgets.celular.name", "Your phone"),
            description: tr("widgets.celular.desc", "Your phone's notifications, files and battery"),
            icon: "smartphone",
            fontIcon: "󰄜"
        },
        {
            id: "system",
            name: tr("widgets.system.name", "System Controls"),
            description: tr("widgets.system.desc", "Performance monitor, power, and quick settings"),
            icon: "preferences-system",
            fontIcon: "󰒓"
        }
    ];
}

function getLayout(name, mx, my, mw, mh, userScale, barPosition) {
    let scale = (userScale !== undefined && userScale !== null) ? userScale : 1.0;
    if (!barPosition) barPosition = "top";

    let base = {
        "network": { 
            w: 720, h: 600, comp: "network/NetworkPopup.qml", 
            pos: { 
                "top": { anchor: "top-right", mt: 52, mr: 4 }, 
                "bottom": { anchor: "bottom-right", mb: 52, mr: 4 }, 
                "left": { anchor: "bottom-left", ml: 52, mb: 4 }, 
                "right": { anchor: "bottom-right", mr: 52, mb: 4 } 
            } 
        },
        // Vortex 1.10: OneDrive (onedrive/OneDrivePopup.qml), donde la red y el celular.
        "onedrive": {
            w: 520, h: 560, comp: "onedrive/OneDrivePopup.qml",
            pos: {
                "top": { anchor: "top-right", mt: 52, mr: 4 },
                "bottom": { anchor: "bottom-right", mb: 52, mr: 4 },
                "left": { anchor: "bottom-left", ml: 52, mb: 4 },
                "right": { anchor: "bottom-right", mr: 52, mb: 4 }
            }
        },
        // Vortex 1.8: «Tu celular» (celular/CelularPopup.qml), donde la red.
        "celular": {
            w: 520, h: 600, comp: "celular/CelularPopup.qml",
            pos: {
                "top": { anchor: "top-right", mt: 52, mr: 4 },
                "bottom": { anchor: "bottom-right", mb: 52, mr: 4 },
                "left": { anchor: "bottom-left", ml: 52, mb: 4 },
                "right": { anchor: "bottom-right", mr: 52, mb: 4 }
            }
        },
        "volume": { 
            w: 410, h: 580, comp: "volume/VolumePopup.qml", 
            pos: { 
                "top": { anchor: "top-right", mt: 52, mr: 5 }, 
                "bottom": { anchor: "bottom-right", mb: 52, mr: 5 }, 
                "left": { anchor: "bottom-left", ml: 52, mb: 5 }, 
                "right": { anchor: "bottom-right", mr: 52, mb: 5 } 
            } 
        },
        "guide": { 
            w: 1200, h: 750, comp: "guide/GuidePopup.qml", draggable: true,
            pos: { 
                "top": { anchor: "center" }, 
                "bottom": { anchor: "center" }, 
                "left": { anchor: "center" }, 
                "right": { anchor: "center" } 
            } 
        },
        "calendar": { 
            w: 1360, h: 694, comp: "calendar/CalendarPopup.qml", // Vortex: +120 por el panel de rendimiento y +64 por los botones de mods 
            pos: { 
                "top": { anchor: "top-center", mt: 52 }, 
                "bottom": { anchor: "bottom-center", mb: 52 }, 
                "left": { anchor: "top-center", mt: 12 }, 
                "right": { anchor: "top-center", mt: 12 } 
            } 
        },
        "wallpaper": { 
            w: "fill", h: 650, comp: "wallpaper/WallpaperPicker.qml", 
            pos: { 
                "top": { anchor: "center-left" }, 
                "bottom": { anchor: "center-left" }, 
                "left": { anchor: "center-left" }, 
                "right": { anchor: "center-left" } 
            } 
        },
        "music": { 
            w: 625, h: 565, comp: "media/MusicPopup.qml", sinEscala: true,
            pos: { 
                "top": { anchor: "top-left", mt: 52, ml: 5 }, 
                "bottom": { anchor: "bottom-left", mb: 52, ml: 5 }, 
                "left": { anchor: "top-left", mt: 5, ml: 52 }, 
                "right": { anchor: "top-right", mt: 5, mr: 52 } 
            } 
        },
        "notifications": { 
            w: 500, h: "fill", comp: "notifications/NotificationCenter.qml", 
            pos: { 
                "top": { anchor: "left" }, 
                "bottom": { anchor: "left" }, 
                "left": { anchor: "left" }, 
                "right": { anchor: "left" } 
            } 
        },
        "system": { 
            w: 500, h: "fill", comp: "syspanel/SystemPanel.qml", 
            pos: { 
                "top": { anchor: "right" }, 
                "bottom": { anchor: "right" }, 
                "left": { anchor: "right" }, 
                "right": { anchor: "left" } 
            } 
        },
        // Vortex (1.7.7): hoja de atajos (keybinds/KeybindsPopup.qml), al centro como Ajustes.
        "keybinds": {
            w: 1360, h: 820, comp: "keybinds/KeybindsPopup.qml", draggable: true,
            pos: {
                "top": { anchor: "center" },
                "bottom": { anchor: "center" },
                "left": { anchor: "center" },
                "right": { anchor: "center" }
            }
        },
        // Vortex 1.8: biblioteca de juegos (juegos/JuegosPopup.qml), al centro como la hoja de atajos.
        "juegos": {
            w: 1180, h: 780, comp: "juegos/JuegosPopup.qml", draggable: true,
            pos: {
                "top": { anchor: "center" },
                "bottom": { anchor: "center" },
                "left": { anchor: "center" },
                "right": { anchor: "center" }
            }
        },
        // Vortex 1.10: temporizador de estudio (estudio/EstudioPopup.qml), al centro.
        "estudio": {
            w: 520, h: 680, comp: "estudio/EstudioPopup.qml", draggable: true,
            pos: {
                "top": { anchor: "center" },
                "bottom": { anchor: "center" },
                "left": { anchor: "center" },
                "right": { anchor: "center" }
            }
        },
        // Vortex 1.10: metrónomo y afinador (musica/MusicaPopup.qml), al centro.
        "musica": {
            w: 680, h: 640, comp: "musica/MusicaPopup.qml", draggable: true,
            pos: {
                "top": { anchor: "center" },
                "bottom": { anchor: "center" },
                "left": { anchor: "center" },
                "right": { anchor: "center" }
            }
        },
        // Vortex 1.10: «Widgets en HTML» (htmlwidgets/HtmlWidgetsPopup.qml), al centro.
        "htmlwidgets": {
            w: 1000, h: 680, comp: "htmlwidgets/HtmlWidgetsPopup.qml", draggable: true, sinEscala: true,
            pos: {
                "top": { anchor: "center" },
                "bottom": { anchor: "center" },
                "left": { anchor: "center" },
                "right": { anchor: "center" }
            }
        },
        // Vortex 1.10: la nota rápida (notas/NotasPopup.qml), al centro.
        "notas": {
            w: 620, h: 640, comp: "notas/NotasPopup.qml", draggable: true, sinEscala: true,
            pos: {
                "top": { anchor: "center" },
                "bottom": { anchor: "center" },
                "left": { anchor: "center" },
                "right": { anchor: "center" }
            }
        },
        // Vortex 1.10: el buscaminas tryhard (buscaminas/BuscaminasPopup.qml), al centro. Cabe el
        // tablero Tryhard de 30×20 con casillas de ~30 px.
        "buscaminas": {
            w: 1040, h: 840, comp: "buscaminas/BuscaminasPopup.qml", draggable: true, sinEscala: true,
            pos: {
                "top": { anchor: "center" },
                "bottom": { anchor: "center" },
                "left": { anchor: "center" },
                "right": { anchor: "center" }
            }
        },
        // Vortex 1.10: «Mods de Minecraft» (minecraft/MinecraftPopup.qml), al centro. Los botones del
        // calendario lo abren en su pestaña: «modrinth» o «curseforge».
        "modrinth": {
            w: 1140, h: 760, comp: "minecraft/ModrinthPopup.qml", draggable: true, sinEscala: true,
            pos: {
                "top": { anchor: "center" },
                "bottom": { anchor: "center" },
                "left": { anchor: "center" },
                "right": { anchor: "center" }
            }
        },
        "curseforge": {
            w: 1140, h: 760, comp: "minecraft/CurseForgePopup.qml", draggable: true, sinEscala: true,
            pos: {
                "top": { anchor: "center" },
                "bottom": { anchor: "center" },
                "left": { anchor: "center" },
                "right": { anchor: "center" }
            }
        },
        // Vortex 1.10: Vortexine Restore (restore/RestorePopup.qml), al centro.
        "restore": {
            w: 720, h: 720, comp: "restore/RestorePopup.qml", draggable: true,
            pos: {
                "top": { anchor: "center" },
                "bottom": { anchor: "center" },
                "left": { anchor: "center" },
                "right": { anchor: "center" }
            }
        },
        // Vortex 1.9: «Algo no funciona» (arreglar/ArreglarPopup.qml), al centro.
        "arreglar": {
            w: 720, h: 720, comp: "arreglar/ArreglarPopup.qml", draggable: true,
            pos: {
                "top": { anchor: "center" },
                "bottom": { anchor: "center" },
                "left": { anchor: "center" },
                "right": { anchor: "center" }
            }
        },
        "hidden": { 
            w: 1, h: 1, comp: "", 
            pos: { 
                "top": { anchor: "hidden" }, 
                "bottom": { anchor: "hidden" }, 
                "left": { anchor: "hidden" }, 
                "right": { anchor: "hidden" } 
            } 
        }
    };

    if (!base[name]) return null;
    let t = base[name];
    let p = t.pos[barPosition] || t.pos["top"];

    // Vortex: «sinEscala» = el contenido no se encoge con general.uiScale (Scaler.s no escala), así
    // que el marco tampoco; si no, con el 0.85 de la semilla el contenido se sale y se corta.
    let sizeScale = t.sinEscala ? 1.0 : scale;
    let finalW = t.w === "fill" ? mw : s(t.w, sizeScale);
    let finalH = t.h === "fill" ? mh : s(t.h, sizeScale);

    let rx = 0;
    let ry = 0;

    switch (p.anchor) {
        case "center":
            rx = Math.floor((mw / 2) - (finalW / 2));
            ry = Math.floor((mh / 2) - (finalH / 2));
            break;
        case "top-right":
            rx = mw - finalW - s(p.mr || 0, scale);
            ry = s(p.mt || 0, scale);
            break;
        case "bottom-right":
            rx = mw - finalW - s(p.mr || 0, scale);
            ry = mh - finalH - s(p.mb || 0, scale);
            break;
        case "top-left":
            rx = s(p.ml || 0, scale);
            ry = s(p.mt || 0, scale);
            break;
        case "bottom-left":
            rx = s(p.ml || 0, scale);
            ry = mh - finalH - s(p.mb || 0, scale);
            break;
        case "top-center":
            rx = Math.floor((mw / 2) - (finalW / 2));
            ry = s(p.mt || 0, scale);
            break;
        case "bottom-center":
            rx = Math.floor((mw / 2) - (finalW / 2));
            ry = mh - finalH - s(p.mb || 0, scale);
            break;
        case "left":
            rx = s(p.ml || 0, scale);
            ry = Math.floor((mh / 2) - (finalH / 2));
            break;
        case "right":
            rx = mw - finalW - s(p.mr || 0, scale);
            ry = Math.floor((mh / 2) - (finalH / 2));
            break;
        case "center-left":
            rx = s(p.ml || 0, scale);
            ry = Math.floor((mh / 2) - (finalH / 2));
            break;
        case "center-right":
            rx = mw - finalW - s(p.mr || 0, scale);
            ry = Math.floor((mh / 2) - (finalH / 2));
            break;
        case "fill":
            rx = 0;
            ry = 0;
            break;
        case "hidden":
            rx = -5000;
            ry = -5000;
            break;
    }

    return { w: finalW, h: finalH, rx: mx + rx, ry: my + ry, comp: t.comp, draggable: Boolean(t.draggable) };
}

function getPopupLayout(mw, mh, userScale) {
    let scale = (userScale !== undefined && userScale !== null) ? userScale : 1.0;
    return {
        w: s(350, scale), marginTop: s(52, scale), marginRight: s(20, scale),
        spacing: s(12, scale), radius: s(14, scale), padding: s(12, scale)
    };
}
