# Instalar Vortexine's OS junto a Windows SIN memoria USB

Para quien no tiene USB, tarjeta SD ni disco externo. El instalador se copia a una partición
pequeña del mismo disco y la PC arranca desde ahí. Funciona con Secure Boot activo, sin entrar a
la BIOS.

Ejemplo de esta guía: un disco con Windows y unos 250 GB libres. Se usan **180 GB** para
Vortexine's OS y **6 GB** para el instalador (que se borran al final).

Necesitas:

- Unos 9 GB libres en C: para bajar la ISO (se borran al final).
- Internet durante la instalación.
- Una hora más o menos.

> Si en algún momento algo no sale como dice aquí, **detente** y pide ayuda antes de seguir. Nada
> de esto borra tus archivos de Windows, pero en las particiones hay que ir con calma.

---

> **¿Prefieres apagar Secure Boot?** También funciona, y así no sale ninguna pantalla azul
> (MokManager): sáltate esos pasos. Antes de apagarlo:
>
> 1. Si Windows tiene **BitLocker**, ve a «Administrar BitLocker» → **«Suspender la protección»**.
>    Si no, al apagar Secure Boot Windows pedirá la clave de recuperación.
> 2. Entra a la BIOS: apaga la PC y al prenderla presiona varias veces **F2** (casi todas las
>    laptops), **Supr** o **F1** (PCs de escritorio), o **Esc** y luego F10 (HP).
> 3. Busca «Secure Boot» (en Seguridad, Boot o Arranque) → **Disabled** → guarda con **F10**.
>
> Si algún día lo vuelves a activar, antes, en Vortexine's, corre `sudo vortex-firmar inscribir` y
> reinicia: así tu PC acepta su llave.

## 0. Bajar la ISO y unirla

1. Entra a la página de la versión 1.0:
   <https://github.com/MikePianoXD777/vortex-os/releases/tag/v1.0.0>
2. Abajo, en **Assets**, da clic y descarga estos 3 archivos (quedan en tu carpeta **Descargas**):
   - `vortexines-os-1.0.iso.parte0` (2 GB)
   - `vortexines-os-1.0.iso.parte1` (2 GB)
   - `vortexines-os-1.0.iso.sha256` (chiquito)

   La ISO viene en 2 partes porque GitHub no deja subir archivos de más de 2 GB. Espera a que
   terminen las dos.
3. **Únelas.**
   1. Abre la carpeta **Descargas**.
   2. Da clic en la barra de dirección de arriba (donde dice «Descargas»), escribe **cmd** y Enter.
      Se abre una ventana negra ya en esa carpeta.
   3. Copia y pega este comando y presiona Enter:

      ```
      copy /b vortexines-os-1.0.iso.parte0 + vortexines-os-1.0.iso.parte1 vortexines-os-1.0.iso
      ```

      Tarda un minuto. Al final dice «1 archivos copiados» y en Descargas aparece
      `vortexines-os-1.0.iso` (unos 4 GB).
4. **Revisa que quedó bien.** En la misma ventana negra pega esto y Enter:

   ```
   certutil -hashfile vortexines-os-1.0.iso SHA256
   ```

   Debe salir exactamente:
   `0c75808c3fba597bdf43a2ae51dc0551b472de605ca80cf43a3c26c2927bb088`
   (también está dentro de `vortexines-os-1.0.iso.sha256`; ábrelo con el Bloc de notas).
   Si no es igual, la descarga se cortó: borra las partes, bájalas otra vez y vuelve a unirlas.
5. Ya puedes borrar las dos partes (`.parte0` y `.parte1`) para liberar espacio. La ISO se queda.

## 1. Preparar Windows

1. **Respalda lo importante** (fotos, documentos, partidas) en la nube.
2. **BitLocker:** en el menú Inicio busca «Administrar BitLocker».
   - Si dice que está activado, guarda tu **clave de recuperación**: entra a
     <https://account.microsoft.com/devices/recoverykey> y anótala o tómale foto.
   - Si no aparece o dice desactivado, sigue.
3. **Apaga el «Inicio rápido»:**
   1. Panel de control → Opciones de energía.
   2. «Elegir el comportamiento de los botones de inicio/apagado».
   3. «Cambiar la configuración actualmente no disponible».
   4. Quita la palomita de **«Activar inicio rápido»** → Guardar cambios.

## 2. Hacer espacio: 186 GB

1. Clic derecho en el botón Inicio → **Administración de discos**.
2. Clic derecho en **(C:)** → **Reducir volumen…**.
3. En «Tamaño del espacio que desea reducir, en MB» escribe **190464** → **Reducir**.
   Queda un bloque negro **«No asignado»** de unos 186 GB.

## 3. La partición del instalador (6 GB)

1. Clic derecho en el bloque **No asignado** → **Nuevo volumen simple…** → Siguiente.
2. Tamaño: **6144** MB → Siguiente.
3. Letra de unidad: elige **V** → Siguiente.
4. Formatear con:
   - Sistema de archivos: **FAT32**
   - Etiqueta: **VORTEXINE**
   - Formato rápido: sí
   → Siguiente → Finalizar.
5. Debe quedar: **(C:)**, luego **VORTEXINE (V:)** de 6 GB, y luego **No asignado** de unos 180 GB.
   **No crees nada en esos 180 GB:** son para Vortexine's.

## 4. Copiar el instalador

1. Doble clic en `vortexines-os-1.0.iso`. Windows la abre como si fuera un disco (por ejemplo, la
   unidad E:).
2. Entra a ese disco, selecciona **todo** (Ctrl+A) y **cópialo** (Ctrl+C).
3. Abre **VORTEXINE (V:)** y **pega** (Ctrl+V). Son unos 4 GB; espera a que termine.
4. Revisa que en V: haya las carpetas **EFI**, **arch**, **boot** y **loader**.
5. Cierra la ventana del disco de la ISO (clic derecho en él → Expulsar).

## 5. Decirle a la PC que arranque desde ahí

1. En el menú Inicio escribe **cmd**, clic derecho en «Símbolo del sistema» → **Ejecutar como
   administrador** → Sí.
2. Copia y pega estos comandos **uno por uno**, presionando Enter después de cada uno:

   ```
   for /f "tokens=2 delims={}" %g in ('bcdedit /copy {bootmgr} /d "Instalador Vortexines OS"') do set GUID={%g}
   ```

   ```
   echo %GUID%
   ```

   Debe salir algo como `{1a2b3c4d-....}`. Si sale `%GUID%` o nada, **detente** y pide ayuda.

   ```
   bcdedit /set %GUID% device partition=V:
   ```

   ```
   bcdedit /set %GUID% path \EFI\BOOT\BOOTx64.EFI
   ```

   ```
   bcdedit /set {fwbootmgr} displayorder %GUID% /addfirst
   ```

   Cada uno debe decir **«La operación se completó correctamente»**.
3. Guarda lo que tengas abierto y reinicia: escribe `shutdown /r /t 0` y Enter.

Desde ahora la PC arranca primero el instalador (hasta que termines; en el paso 9 se quita). Si en
vez de eso entra a Windows, apágala y al prenderla presiona varias veces la tecla del **menú de
arranque**. Suele ser:

- **F12**: Lenovo, Dell, Acer.
- **F8**: ASUS.
- **F11**: MSI.
- **Esc** y luego **F9**: HP.

Ahí elige **«Instalador Vortexines OS»**.

## 6. Secure Boot (pantalla azul, una sola vez)

Si tu PC tiene Secure Boot (con Windows 11 casi siempre), sale una **pantalla azul en inglés:
MokManager**. Es normal:

1. Si dice «Verification failed», presiona **Enter**. Si dice «Press any key», presiona una tecla
   **rápido**: son unos 10 segundos.
2. Elige **Enroll key from disk** (se mueve con las flechas y Enter).
3. Elige el disco que tenga la carpeta **EFI** con el archivo **vortexines.cer** (la partición
   VORTEXINE), entra a **EFI** y elige **vortexines.cer**.
4. **Continue** → **Yes** → **Reboot**.
5. Al reiniciar vuelve a arrancar el instalador, ya sin la pantalla azul (si entra a Windows, usa la
   tecla del menú de arranque).

Entra el escritorio de Vortexine's OS y se abre solo el **instalador**.

## 7. El instalador

1. **Idioma:** Español (México) → Siguiente.
2. **Ubicación** → Siguiente.
3. **Teclado:** Latinoamericano (ya viene puesto) → Siguiente.
4. **Particiones:** elige **«Particionado manual»**.
   1. Verás las particiones de Windows, la **VORTEXINE** de 6 GB y el **Espacio libre** de ~180 GB.
      **No toques ninguna de las de Windows ni la VORTEXINE** (de ahí está corriendo el instalador).
   2. Clic en el **Espacio libre** → **Crear**:
      - Tamaño: **1024 MiB**
      - Sistema de archivos: **fat32**
      - Punto de montaje: **/boot**
      - Marcas: **boot** (y **esp** si aparece)
      → Aceptar.
   3. Clic en el **Espacio libre** que queda → **Crear**:
      - Tamaño: todo lo que queda
      - Sistema de archivos: **btrfs**
      - Punto de montaje: **/**
      → Aceptar.
   4. Si pregunta dónde instalar el gestor de arranque, elige **la partición de 1 GB** que creaste.
   5. Siguiente.
5. **Usuario:** tu nombre, usuario (minúsculas, sin espacios) y una contraseña que no olvides.
6. **Apps:** las que quieras (Steam, Prism Launcher, Sober…).
7. **Resumen:** revisa que solo se creen las 2 particiones nuevas y que **ninguna** de Windows ni
   la VORTEXINE diga «Formatear» o «Eliminar». → **Instalar**.
8. Al terminar, **Reiniciar ahora**.

## 8. El primer arranque

- Con Secure Boot sale otra vez **MokManager**, ahora para la llave de tu PC:
  1. Presiona una tecla rápido.
  2. **Enroll MOK**.
  3. **Continue**.
  4. **Yes**.
  5. Contraseña: **1234**.
  6. **Reboot**.
- Arranca **Vortexine's OS**. Los drivers de la tarjeta de video se instalan solos (con internet).
- Si arranca el instalador o Windows en vez de Vortexine's: apaga, prende con la tecla del menú de
  arranque y elige **«Vortexine's OS»**. (Vortexine's se pone primero solo al instalarse; si no,
  el paso 9 quita la entrada del instalador.)

## 9. Limpiar el instalador (en Windows)

Ya no se necesita la partición de 6 GB:

1. Entra a Windows («Reiniciar en Windows» desde Vortexine's, o con el menú de arranque).
2. **Quita la entrada de arranque del instalador.** Abre «Símbolo del sistema» como administrador
   y escribe:

   ```
   bcdedit /enum firmware
   ```

   Busca el bloque que dice `description  Instalador Vortexines OS` y copia su `identificador`
   (el texto entre `{ }`, con las llaves). Luego, con tu identificador:

   ```
   bcdedit /delete {EL-IDENTIFICADOR}
   ```

3. **Borra la partición.** En Administración de discos: clic derecho en **VORTEXINE (V:)** →
   **Eliminar volumen** → Sí. Queda un bloque «No asignado» de 6 GB.
4. Clic derecho en **(C:)** → **Extender volumen…** → Siguiente → Siguiente → Finalizar. Windows
   recupera esos 6 GB.
   **No toques** las particiones de Vortexine's (las de 1 GB y ~179 GB, que Windows ve como «sin
   letra» o «partición desconocida»).

## Usar Windows después

- Desde Vortexine's: en el lanzador, **«Reiniciar en Windows»** (o `vortex windows` en la terminal).
  Entra una vez a Windows; al apagarlo vuelve a Vortexine's.
- O al prender, con la tecla del menú de arranque → **Windows Boot Manager**.

## Si algo sale mal

- **Windows pide la clave de BitLocker:** escríbela (la guardaste en el paso 1). Pasa a veces al
  cambiar el arranque.
- **«Reducir volumen» deja menos espacio del que pediste:** Windows no puede mover algunos archivos
  (casi siempre puntos de restauración). Prueba esto, en «Símbolo del sistema» como administrador,
  y reinicia:
  - `vssadmin delete shadows /for=C: /all /quiet` (borra los puntos de restauración);
  - `powercfg /h off` (apaga la hibernación; después se regresa con `powercfg /h on`).

  **No quites el archivo de paginación** (memoria virtual): en una HP Victus eso dejó a Windows sin
  poder usar el PIN al reiniciar. Si aun así no deja reducir lo suficiente (al menos 60 GB), es
  mejor conseguir una USB o una microSD de 8 GB y usar la guía con USB
  ([guia-dual-boot.md](guia-dual-boot.md)).
- **No aparece «FAT32» al formatear:** el tamaño debe ser de 32 GB o menos (aquí son 6 GB).
- **En el paso 5 sale «Acceso denegado»:** el «Símbolo del sistema» no está como administrador.
- **Quiero quitar Vortexine's:**
  1. En Administración de discos, borra sus particiones (la de 1 GB y la grande).
  2. Extiende C:.
  3. Borra su entrada con `bcdedit /enum firmware` y `bcdedit /delete`, igual que en el paso 9.
