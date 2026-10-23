# Instalar Vortexine's OS junto a Windows (en el mismo disco)

Para quien nunca ha instalado Linux. Al final, al prender la PC entras a Vortexine's OS, y cuando
quieras Windows lo eliges desde Vortexine's con **«Reiniciar en Windows»** o desde el menú de
arranque de la PC. Windows y tus archivos de Windows no se tocan.

Ejemplo de esta guía: un disco con Windows y unos 250 GB libres, de los que **180 GB** son para
Vortexine's OS.

Necesitas:

- Una USB de 8 GB o más (se borra).
- La ISO de Vortexine's OS (`vortexines-os-….iso`).
- Internet por cable o Wi-Fi durante la instalación (para las apps que elijas).
- Una hora más o menos.

---

## 1. En Windows: preparar

1. **Respalda lo importante** (fotos, documentos, partidas guardadas) en otro disco o en la nube.
   No debería pasar nada, pero es lo que se hace siempre antes de tocar particiones.
2. **BitLocker:** busca «Administrar BitLocker» en el menú Inicio.
   - Si dice «BitLocker activado», guarda tu **clave de recuperación**: entra a
     <https://account.microsoft.com/devices/recoverykey> y anótala o tómale foto.
   - Si está desactivado, sigue.
3. **Apaga el «Inicio rápido» de Windows.** Ve a Panel de control → Opciones de energía → «Elegir
   el comportamiento de los botones de inicio/apagado» → «Cambiar la configuración actualmente no
   disponible» → quita la palomita de **«Activar inicio rápido»** → Guardar cambios. Si no, Windows
   deja el disco «a medio apagar» y Linux no puede usarlo bien.
4. **Hazle espacio a Vortexine's OS (180 GB):**
   1. Clic derecho en el botón Inicio → **Administración de discos**.
   2. Clic derecho en **(C:)** → **Reducir volumen…**.
   3. En «Tamaño del espacio que desea reducir, en MB» escribe **184320** (son 180 GB) → **Reducir**.
   4. Queda un bloque negro que dice **«No asignado» de 180 GB**. Déjalo así: no le crees nada encima.
5. **Mira el tamaño de la partición EFI.** En Administración de discos, en el mismo disco, hay una
   partición chica que dice **«Partición de sistema EFI»**.
   - Casi siempre mide **100 MB**. Es muy chica para Vortexine's (los kernels con NVIDIA pesan más),
     así que en el paso 4 vas a crear **una partición EFI propia** de 1 GB dentro de los 180 GB.
   - Si mide 500 MB o más, puedes hacer lo mismo: la EFI propia siempre funciona.

## 2. Hacer la USB

1. Baja **Rufus** de <https://rufus.ie> (la versión portable sirve).
2. Abre Rufus, elige tu USB en «Dispositivo» y en «Elección de arranque» elige la ISO.
3. Si pregunta cómo escribirla, elige **«Escribir en modo Imagen DD»**.
4. **EMPEZAR** y espera a que diga «PREPARADO».

## 3. Arrancar desde la USB (sin entrar a la BIOS)

1. Con la USB puesta, en Windows: Inicio → Encendido → mantén **Shift** y da clic en **Reiniciar**.
2. Sale una pantalla azul de Windows: **Usar un dispositivo** → elige la **USB** (puede salir como
   «UEFI: …» con el nombre de la USB).
   - Si no sale esa opción: apaga la PC y al prenderla presiona varias veces la tecla del **menú
     de arranque**. Suele ser **F12** (Lenovo, Dell, Acer), **F8** (ASUS), **F11** (MSI) o **Esc**
     (HP, y luego F9). Ahí eliges la USB.
3. **Secure Boot.** Si tu PC lo tiene activo (con Windows 11 casi siempre), la primera vez sale una
   **pantalla azul en inglés (MokManager)**. Es normal y es una sola vez:
   1. Si dice «Verification failed», presiona **Enter**.
   2. Elige **Enroll key from disk** (con las flechas y Enter).
   3. Elige el disco que dice **ARCHISO_EFI**, luego la carpeta **EFI** y luego **vortexines.cer**.
   4. **Continue** → **Yes** → **Reboot**.
   5. Vuelve a elegir la USB en el menú de arranque (paso 2).
   No hace falta entrar a la BIOS ni apagar Secure Boot.
4. Entra el escritorio de Vortexine's OS y se abre solo el **instalador**.

## 4. El instalador

1. **Idioma:** Español (México) → Siguiente.
2. **Ubicación:** America/Mexico_City → Siguiente.
3. **Teclado:** Latinoamericano (ya viene puesto) → Siguiente.
4. **Particiones:** elige **«Particionado manual»**.
   1. Abajo, el disco correcto (el que tiene Windows). Verás tus particiones de Windows y el bloque
      **«Espacio libre» de ~180 GB**. **No toques las de Windows.**
   2. Clic en el **Espacio libre** → **Crear**:
      - Tamaño: **1024 MiB**
      - Sistema de archivos: **fat32**
      - Punto de montaje: **/boot**
      - Marcas: **boot** (y **esp** si aparece)
      - Aceptar.
   3. Clic otra vez en el **Espacio libre** que queda → **Crear**:
      - Tamaño: todo lo que queda (viene así).
      - Sistema de archivos: **btrfs**
      - Punto de montaje: **/**
      - Aceptar.
   4. Si abajo pregunta dónde instalar el gestor de arranque, elige **la partición de 1 GB que
      creaste** (la de /boot).
   5. Siguiente. Si avisa que hay otra partición EFI (la de Windows), está bien: Vortexine's usa la
      suya.
5. **Usuario:** tu nombre, el nombre de usuario (minúsculas, sin espacios) y una contraseña que no
   olvides. Puedes marcar «Iniciar sesión automáticamente».
6. **Apps:** marca las que quieras (Steam, Prism Launcher, Sober…). Claude viene apagado; se
   activa después desde Ajustes si lo quieres.
7. **Resumen:** revisa que en las particiones solo se creen las dos nuevas (1 GB y ~179 GB) y que
   ninguna de Windows diga «Formatear» o «Eliminar». → **Instalar**.
8. Cuando diga **Listo**, marca «Reiniciar ahora» y quita la USB cuando se apague la pantalla.

## 5. El primer arranque

- Si Secure Boot está activo, **otra vez sale la pantalla azul (MokManager)**, ahora para la llave
  de tu PC. Una sola vez:
  1. **Enroll MOK**.
  2. **Continue**.
  3. **Yes**.
  4. Contraseña: **1234**.
  5. **Reboot**.
- Arranca **Vortexine's OS**. Los drivers de la NVIDIA ya se instalaron solos. Si no había
  internet, se instalan solos la primera vez que haya red, y te avisa que reinicies.

## 6. Usar Windows

- Desde Vortexine's: abre el lanzador y busca **«Reiniciar en Windows»**. La PC reinicia una vez en
  Windows; al apagar Windows, la próxima vez vuelve a Vortexine's. También desde una terminal:
  `vortex windows`.
- O al prender la PC, con la tecla del menú de arranque (paso 3) → **Windows Boot Manager**.

## Si algo sale mal

- **La USB no aparece en el menú de arranque:** vuelve a hacerla con Rufus en modo DD, o prueba
  otro puerto USB (mejor uno directo de la placa, no uno de enfrente).
- **Windows pide la clave de BitLocker:** escríbela (la guardaste en el paso 1.2). Pasa a veces al
  cambiar el arranque y luego ya no la vuelve a pedir.
- **Arranca directo a Windows y no sale Vortexine's:** usa el menú de arranque (F12, F8…) y elige
  «Vortexine's OS». Para que sea siempre el primero, en Vortexine's corre
  `sudo efibootmgr` y avísale a quien te ayudó.
- **Quiero quitar Vortexine's:** en Windows, Administración de discos → borra las dos particiones
  de Vortexine's (la de 1 GB y la grande) y extiende C:. Windows sigue igual.

Para la persona que ayuda: el estado de Secure Boot se ve con `sudo vortex-firmar estado`.
