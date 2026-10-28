"""Textos de la app en español e inglés, en palabras sencillas (sin tecnicismos para la persona).

El idioma sale de LC_ALL / LC_MESSAGES / LANG («es_MX.UTF-8» → español; cualquier otro → inglés).
VORTEX_APP_INSTALLER_IDIOMA=es|en lo fuerza (pruebas y capturas).

    tr("hecho_instalado", nombre="Firefox")   →  «Firefox ya está instalado»
    «{n|cosa|cosas}» elige singular o plural según n.
"""
from __future__ import annotations

import os
import re

TEXTOS: dict[str, tuple[str, str]] = {
    # --- ventana ---
    "pestana_archivo": ("Archivo", "File"),
    "pestana_instalado": ("Instalado", "Installed"),
    "vacio_titulo": ("Arrastra aquí un archivo", "Drop a file here"),
    "vacio_texto": ("Un programa, un comprimido o un instalador que bajaste de internet. Te digo qué es y lo instalo o lo descomprimo por ti.",
                    "A program, an archive or an installer you downloaded. I'll tell you what it is and install or extract it for you."),
    "boton_elegir_archivo": ("Elegir archivo", "Choose file"),
    "soltar_aqui": ("Suelta el archivo para abrirlo", "Drop the file to open it"),
    "leyendo": ("Revisando el archivo…", "Checking the file…"),
    "boton_otro_archivo": ("Abrir otro archivo", "Open another file"),
    "boton_cancelar": ("Cancelar", "Cancel"),
    "boton_aceptar": ("Aceptar", "OK"),
    "boton_listo": ("Listo", "Done"),
    "boton_reintentar": ("Volver", "Back"),
    "boton_copiar": ("Copiar", "Copy"),
    "copiado": ("Copiado", "Copied"),
    "boton_abrir_app": ("Abrir ahora", "Open now"),
    "boton_ver_carpeta": ("Ver la carpeta", "Show folder"),
    "trabajando": ("Trabajando…", "Working…"),
    "trabajando_descomprimir": ("Descomprimiendo…", "Extracting…"),
    "trabajando_instalar": ("Instalando…", "Installing…"),
    "trabajando_flathub": ("Bajando de Flathub…", "Downloading from Flathub…"),
    "cerrar_aviso": ("Cerrar aviso", "Close notice"),
    "contenido_titulo": ("Qué trae", "What's inside"),
    "contenido_y_mas": ("…y {n} {n|cosa|cosas} más", "…and {n} more"),
    "vista_titulo": ("Así empieza el instalador", "How the installer starts"),
    "vista_binario": ("Es un programa ya armado (no se puede leer como texto).", "It's a compiled program (it can't be read as text)."),
    "admin_titulo": ("Si te pide ser administrador", "If it asks for administrator rights"),
    "admin_texto": ("Esta app nunca usa ni guarda tu contraseña. Si el instalador la necesita, copia este comando y pégalo tú en una terminal:",
                    "This app never uses or stores your password. If the installer needs it, copy this command and paste it in a terminal yourself:"),
    "falta_titulo": ("Hace falta un programa", "A program is missing"),
    "falta_texto": ("Para instalarlo copia este comando y pégalo en una terminal (te pedirá tu contraseña):",
                    "To install it, copy this command and paste it in a terminal (it will ask for your password):"),
    "flathub_titulo": ("En Flathub", "On Flathub"),
    "instancias_titulo": ("¿A qué Minecraft lo mando?", "Which Minecraft should it go to?"),
    "instancia_incompatible": ("Usa otro cargador de mods", "Uses a different mod loader"),
    "boton_mandar": ("Mandar aquí", "Send here"),
    "contrasena_titulo": ("Este comprimido tiene contraseña", "This archive is password-protected"),
    "contrasena_texto": ("Escribe la contraseña que te dio quien te lo mandó. No se guarda en ningún lado.",
                         "Type the password you were given. It isn't saved anywhere."),
    "contrasena_campo": ("Contraseña", "Password"),
    "boton_descomprimir_con": ("Descomprimir", "Extract"),
    "conflicto_titulo": ("Ya existe «{nombre}»", "“{nombre}” already exists"),
    "conflicto_texto": ("No voy a escribir encima de lo que ya tienes. ¿Lo guardo como «{sugerido}»?",
                        "I won't overwrite what you already have. Save it as “{sugerido}”?"),
    "boton_otro_nombre": ("Guardar como «{sugerido}»", "Save as “{sugerido}”"),
    "reemplazar_titulo": ("«{nombre}» ya está instalado", "“{nombre}” is already installed"),
    "reemplazar_texto": ("¿Lo reemplazo por este? La versión que tienes se va a la papelera, por si la necesitas.",
                         "Replace it with this one? The version you have goes to the trash, just in case."),
    "boton_reemplazar": ("Reemplazar", "Replace"),
    "instalado_vacio_titulo": ("Todavía no hay nada aquí", "Nothing here yet"),
    "instalado_vacio_texto": ("Lo que instales o descomprimas con esta app aparece aquí, y lo puedes quitar cuando quieras.",
                              "Whatever you install or extract with this app shows up here, and you can remove it any time."),
    "instalado_ayuda": ("Lo que quites va a la papelera, por si te arrepientes.", "Anything you remove goes to the trash, in case you change your mind."),
    "quitar_titulo": ("¿Quitar «{nombre}»?", "Remove “{nombre}”?"),
    "quitar_texto_app": ("Se desinstala y desaparece del lanzador de apps. Sus archivos van a la papelera.",
                         "It gets uninstalled and disappears from the app launcher. Its files go to the trash."),
    "quitar_texto_flatpak": ("Se desinstala de tu equipo. Podrás volver a instalarlo cuando quieras.",
                             "It gets uninstalled. You can install it again any time."),
    "quitar_texto_otro": ("Se va a la papelera. Si cambias de opinión, la puedes sacar de ahí.",
                          "It goes to the trash. If you change your mind, you can restore it from there."),
    "boton_abrir_instalado": ("Abrir", "Open"),
    "soltar_varios": ("Solo uno a la vez: abrí el primero.", "One at a time: I opened the first one."),
    "seleccionar_archivo": ("Elige un archivo", "Choose a file"),
    "seleccionar_carpeta": ("Elige una carpeta", "Choose a folder"),
    "sin_deshacer": ("Esto no se puede deshacer desde aquí: lo que haga el programa queda por su cuenta.",
                     "This can't be undone from here: whatever the program does is up to it."),

    # --- tipos ---
    "tipo_comprimido": ("Archivo comprimido", "Compressed archive"),
    "tipo_programa": ("Programa para Linux", "Linux program"),
    "tipo_appimage": ("Programa para Linux (AppImage)", "Linux program (AppImage)"),
    "tipo_script": ("Instalador (corre sus propias instrucciones)", "Installer (runs its own instructions)"),
    "tipo_programa_suelto": ("Programa o instalador para Linux", "Linux program or installer"),
    "tipo_deb": ("Paquete de Debian o Ubuntu", "Debian or Ubuntu package"),
    "tipo_rpm": ("Paquete de Fedora", "Fedora package"),
    "tipo_flatpak": ("App de Flatpak", "Flatpak app"),
    "tipo_windows": ("Instalador de Windows", "Windows installer"),
    "tipo_mod": ("Mod de Minecraft", "Minecraft mod"),
    "tipo_jar": ("Programa de Java", "Java program"),
    "tipo_desconocido": ("Archivo que no reconozco", "Unrecognized file"),
    "tipo_carpeta": ("Carpeta", "Folder"),
    "tipo_falta": ("El archivo ya no está", "The file is gone"),
    "instalado_appimage": ("AppImage", "AppImage"),
    "instalado_programa": ("Programa", "Program"),
    "instalado_flatpak": ("Flatpak", "Flatpak"),
    "instalado_mod": ("Mod de Minecraft", "Minecraft mod"),
    "instalado_descomprimido": ("Carpeta descomprimida", "Extracted folder"),
    "instalado_descomprimido_archivo": ("Archivo descomprimido", "Extracted file"),

    # --- de dónde vino ---
    "origen_sitio": ("Bajado de {sitio}", "Downloaded from {sitio}"),
    "origen_descargas": ("En tu carpeta de descargas", "In your downloads folder"),
    "origen_carpeta": ("En la carpeta «{carpeta}»", "In the “{carpeta}” folder"),
    "aviso_internet": ("Viene de internet y puede ejecutar programas en tu equipo. Úsalo solo si confías en quien lo hizo.",
                       "It comes from the internet and can run programs on your computer. Only use it if you trust whoever made it."),

    # --- descripciones ---
    "contenido_resumen": ("{n} {n|cosa|cosas} · {tam} en total", "{n} {n|item|items} · {tam} in total"),
    "desc_comprimido": ("Es un archivo comprimido. Al descomprimirlo, lo que trae queda en la carpeta «{destino}», aquí mismo o donde elijas.",
                        "It's a compressed archive. When extracted, its contents go into the “{destino}” folder, right here or wherever you choose."),
    "desc_comprimido_sin_lista": ("Es un archivo comprimido. Para ver qué trae hace falta su contraseña.",
                                  "It's a compressed archive. Its password is needed to see what's inside."),
    "desc_comprimido_vacio": ("Es un archivo comprimido, pero viene vacío.", "It's a compressed archive, but it's empty."),
    "desc_programa": ("Es un programa para Linux listo para usar. Al instalarlo queda en tu lanzador de apps (se guarda en {carpeta}) y lo puedes desinstalar cuando quieras.",
                      "It's a ready-to-use Linux program. Once installed it shows up in your app launcher (it's kept in {carpeta}) and you can uninstall it any time."),
    "desc_appimage": ("Es un programa para Linux que funciona tal cual, sin instalar nada más. Puedes instalarlo (queda en tu lanzador de apps) o solo abrirlo esta vez.",
                      "It's a Linux program that works as is. You can install it (it shows up in your app launcher) or just open it this once."),
    "desc_script": ("Es un instalador: al abrirlo corre sus propias instrucciones en una terminal. Revisa que venga de alguien de confianza.",
                    "It's an installer: when opened it runs its own instructions in a terminal. Make sure it comes from someone you trust."),
    "desc_binario": ("Es un programa para Linux. Se abre en una terminal para que veas lo que hace.",
                     "It's a Linux program. It opens in a terminal so you can see what it does."),
    "desc_deb": ("Es un paquete para Debian o Ubuntu. Aquí no se instala directo porque podría descomponer el sistema.",
                 "It's a package for Debian or Ubuntu. It isn't installed directly here because it could break the system."),
    "desc_rpm": ("Es un paquete para Fedora. Aquí no se instala directo porque podría descomponer el sistema.",
                 "It's a package for Fedora. It isn't installed directly here because it could break the system."),
    "desc_en_flathub": ("Pero «{nombre}» está en Flathub y se instala de forma segura, sin contraseña.",
                        "But “{nombre}” is on Flathub and installs safely, without a password."),
    "desc_no_flathub": ("No encontré «{nombre}» en Flathub. Puedes sacar su contenido para verlo.",
                        "I couldn't find “{nombre}” on Flathub. You can extract its contents to look at them."),
    "desc_flathub_sin_red": ("No hay internet para buscarlo en Flathub. Puedes sacar su contenido para verlo.",
                             "There's no internet to look it up on Flathub. You can extract its contents to look at them."),
    "desc_flatpakref": ("Es un enlace para instalar «{nombre}» con Flatpak. Se instala para tu usuario, sin contraseña.",
                        "It's a link to install “{nombre}” with Flatpak. It installs for your user, without a password."),
    "desc_flatpak": ("Es una app de Flatpak en un solo archivo. Se instala para tu usuario, sin contraseña.",
                     "It's a Flatpak app in a single file. It installs for your user, without a password."),
    "desc_windows_con": ("Es un programa de Windows. Puedes intentar abrirlo con {programa}; no todos funcionan.",
                         "It's a Windows program. You can try opening it with {programa}; not all of them work."),
    "desc_windows_sin": ("Es un programa de Windows. Para abrirlo aquí haría falta Wine, que no está instalado.",
                         "It's a Windows program. Opening it here would need Wine, which isn't installed."),
    "desc_windows_flathub": ("Hay una versión para Linux: «{nombre}», en Flathub.", "There's a Linux version: “{nombre}”, on Flathub."),
    "desc_mod": ("Es un mod de Minecraft: «{nombre}» ({loader}). Mándalo al Minecraft donde lo quieras usar.",
                 "It's a Minecraft mod: “{nombre}” ({loader}). Send it to the Minecraft where you want to use it."),
    "desc_jar": ("Es un programa de Java. Se abre con Java.", "It's a Java program. It opens with Java."),
    "desc_carpeta": ("Es una carpeta, no un archivo. Arrastra un archivo de los que bajaste.",
                     "That's a folder, not a file. Drop one of the files you downloaded."),
    "desc_falta": ("Ese archivo ya no existe (¿se movió o se borró?).", "That file no longer exists (was it moved or deleted?)."),
    "desc_desconocido": ("No sé qué tipo de archivo es este, así que no hago nada con él. Puedes abrir su carpeta.",
                         "I don't know what kind of file this is, so I won't do anything with it. You can open its folder."),
    "nota_peligrosas": ("Trae {n} {n|cosa|cosas} que {n|intenta|intentan} guardarse fuera de su carpeta: no se van a descomprimir.",
                        "It has {n} {n|item|items} trying to be saved outside its folder: they won't be extracted."),
    "nota_mod_instancias": ("Los que usan otro cargador de mods salen en gris.", "The ones using a different mod loader are greyed out."),
    "nota_mod_sin_comando": ("Elige la carpeta de tu Minecraft (la que tiene «mods»).", "Choose your Minecraft folder (the one with “mods”)."),
    "nota_mod_sin_instancias": ("No encontré ningún Minecraft. Elige su carpeta a mano.", "I couldn't find any Minecraft. Choose its folder by hand."),

    # --- botones ---
    "boton_instalar": ("Instalar", "Install"),
    "boton_descomprimir": ("Descomprimir aquí", "Extract here"),
    "boton_descomprimir_aqui": ("Solo descomprimir", "Just extract"),
    "boton_descomprimir_en": ("En otra carpeta…", "To another folder…"),
    "boton_solo_abrir": ("Solo abrir", "Just open"),
    "boton_abrir": ("Abrir", "Open"),
    "boton_abrir_terminal": ("Abrir en una terminal", "Open in a terminal"),
    "boton_instalar_flathub": ("Instalar de Flathub", "Install from Flathub"),
    "boton_instalar_flathub_linux": ("Instalar la de Linux", "Install the Linux one"),
    "boton_ver_contenido": ("Sacar su contenido", "Extract its contents"),
    "boton_abrir_con": ("Abrir con {programa}", "Open with {programa}"),
    "boton_elegir_carpeta": ("Elegir la carpeta…", "Choose the folder…"),
    "boton_abrir_java": ("Abrir con Java", "Open with Java"),
    "boton_abrir_carpeta": ("Abrir su carpeta", "Open its folder"),
    "boton_desinstalar": ("Desinstalar", "Uninstall"),
    "boton_quitar": ("Quitar", "Remove"),

    # --- confirmaciones ---
    "conf_instalar_titulo": ("¿Instalar «{nombre}»?", "Install “{nombre}”?"),
    "conf_instalar_appimage": ("Viene de internet. Al instalarlo se va a ejecutar para sacar su ícono. Hazlo solo si confías en quien lo hizo.",
                               "It comes from the internet. Installing it runs it briefly to get its icon. Only do it if you trust whoever made it."),
    "conf_abrir_titulo": ("¿Abrir «{nombre}»?", "Open “{nombre}”?"),
    "conf_abrir_appimage": ("Se le dará permiso para ejecutarse y se abrirá, sin instalar nada.",
                            "It will be allowed to run and will open, without installing anything."),
    "conf_correr_titulo": ("¿Correr «{nombre}»?", "Run “{nombre}”?"),
    "conf_correr": ("Se abrirá una terminal y el instalador hará lo que tenga programado. Esto no se puede deshacer desde aquí.",
                    "A terminal will open and the installer will do whatever it's programmed to do. This can't be undone from here."),
    "conf_windows": ("Se abrirá con Wine. Lo que instale se queda dentro de Wine.", "It will open with Wine. Whatever it installs stays inside Wine."),
    "conf_java": ("Se abrirá con Java y podrá hacer lo que quiera en tu equipo, como cualquier programa.",
                  "It will open with Java and can do anything on your computer, like any program."),

    # --- resultados ---
    "hecho_descomprimido": ("Listo, ya se descomprimió", "Done, it's extracted"),
    "hecho_en_carpeta": ("Quedó en {carpeta}.", "It's in {carpeta}."),
    "hecho_saltados": ("Se {n|saltó|saltaron} {n} {n|cosa peligrosa|cosas peligrosas} que {n|quería|querían} salirse de la carpeta.",
                       "{n} dangerous {n|item was|items were} skipped for trying to escape the folder."),
    "hecho_instalado": ("«{nombre}» ya está instalado", "“{nombre}” is installed"),
    "hecho_en_lanzador": ("Lo encuentras en tu lanzador de apps. Para quitarlo, ve a la pestaña «Instalado».",
                          "You'll find it in your app launcher. To remove it, go to the “Installed” tab."),
    "hecho_abierto": ("Se abrió «{nombre}»", "“{nombre}” is open"),
    "hecho_abierto_appimage": ("No se instaló nada. Si te gusta, puedes instalarlo después.", "Nothing was installed. If you like it, you can install it later."),
    "hecho_terminal": ("Se abrió en una terminal", "It opened in a terminal"),
    "hecho_terminal_texto": ("Sigue ahí las instrucciones del instalador.", "Follow the installer's instructions there."),
    "hecho_windows": ("Puede tardar un poco la primera vez.", "It may take a while the first time."),
    "hecho_java": ("Si no ves una ventana, puede que sea un programa de terminal.", "If you don't see a window, it might be a terminal program."),
    "hecho_mod": ("«{nombre}» quedó en {donde}", "“{nombre}” is now in {donde}"),
    "hecho_mod_texto": ("Abre ese Minecraft para usarlo. Para quitarlo, ve a la pestaña «Instalado».",
                        "Open that Minecraft to use it. To remove it, go to the “Installed” tab."),
    "deshecho_app": ("Se desinstaló «{nombre}».", "“{nombre}” was uninstalled."),
    "deshecho_otro": ("«{nombre}» se fue a la papelera.", "“{nombre}” went to the trash."),
    "desinstalado_terminal": ("{nombre} se desinstaló.", "{nombre} was uninstalled."),

    # --- errores ---
    "contrasena_pide": ("Este comprimido tiene contraseña.", "This archive is password-protected."),
    "contrasena_mala": ("Esa contraseña no es. Revisa mayúsculas y espacios.", "That's not the password. Check capitals and spaces."),
    "falta_programa": ("Para abrir este comprimido hace falta «{programa}», que no está instalado.",
                       "Opening this archive needs “{programa}”, which isn't installed."),
    "comprimido_danado": ("No se pudo abrir: el archivo parece dañado o incompleto. Prueba a bajarlo otra vez.",
                          "It couldn't be opened: the file looks damaged or incomplete. Try downloading it again."),
    "comprimido_tarda": ("El comprimido tardó demasiado en abrirse.", "The archive took too long to open."),
    "sin_espacio": ("No hay espacio suficiente en el disco.", "There isn't enough disk space."),
    "sin_permiso": ("No tengo permiso para escribir ahí. Elige otra carpeta.", "I don't have permission to write there. Choose another folder."),
    "sin_internet": ("No hay internet. Revisa tu conexión y vuelve a intentarlo.", "No internet connection. Check it and try again."),
    "falta_flatpak": ("Flatpak no está instalado en este equipo.", "Flatpak isn't installed on this computer."),
    "flatpak_fallo": ("Flatpak no pudo instalarlo.", "Flatpak couldn't install it."),
    "falta_wine": ("Para abrir programas de Windows hace falta Wine.", "Opening Windows programs needs Wine."),
    "falta_java": ("Para abrirlo hace falta Java.", "Opening it needs Java."),
    "falta_terminal": ("No encontré una terminal para abrirlo (kitty, foot, alacritty o konsole).",
                       "I couldn't find a terminal to open it (kitty, foot, alacritty or konsole)."),
    "no_se_pudo_abrir": ("No se pudo abrir.", "It couldn't be opened."),
    "programa_sin_ejecutable": ("No encontré qué abrir dentro de este programa. Puedes solo descomprimirlo.",
                                "I couldn't find what to open inside this program. You can just extract it."),
    "script_con_sudo": ("Este instalador se quiere correr como administrador. Esta app no lo hace: copia el comando de abajo si confías en él.",
                        "This installer wants to run as administrator. This app won't do that: copy the command below if you trust it."),
    "mods_sin_comando": ("No está el comando «vortex mods». Elige la carpeta a mano.", "The “vortex mods” command isn't there. Choose the folder by hand."),
    "mods_no_se_movio": ("No se pudo pasar el mod a ese Minecraft.", "The mod couldn't be moved to that Minecraft."),
    "ya_no_esta": ("Eso ya no está.", "That's no longer there."),
    "error_general": ("Algo salió mal ({detalle}).", "Something went wrong ({detalle})."),
    "bytes": ("{n} bytes", "{n} bytes"),
}


def idioma() -> str:
    forzado = os.environ.get("VORTEX_APP_INSTALLER_IDIOMA", "").strip().lower()
    if forzado in ("es", "en"):
        return forzado
    for var in ("LC_ALL", "LC_MESSAGES", "LANG", "LANGUAGE"):
        v = os.environ.get(var, "")
        if v:
            return "es" if v.lower().startswith("es") else "en"
    return "es"


_PLURAL = re.compile(r"\{(\w+)\|([^|{}]*)\|([^|{}]*)\}")


def tr(clave: str, **valores) -> str:
    par = TEXTOS.get(clave)
    if par is None:
        return clave
    texto = par[0] if idioma() == "es" else par[1]
    texto = _PLURAL.sub(lambda m: m.group(2) if valores.get(m.group(1)) == 1 else m.group(3), texto)
    for k, v in valores.items():
        texto = texto.replace("{%s}" % k, str(v))
    return texto


def todos() -> dict[str, str]:
    """Todos los textos del idioma actual, para la interfaz (QML los usa como t.clave)."""
    i = 0 if idioma() == "es" else 1
    return {k: v[i] for k, v in TEXTOS.items()}
