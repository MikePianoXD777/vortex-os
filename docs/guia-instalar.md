# Cómo instalar Vortexine's OS (guía fácil)

Para quien casi no le sabe a Windows. Vas paso por paso, sin saltarte ninguno. Tarda **una hora más o menos**,
y la mitad es esperar a que algo se baje.

Si en algún paso no ves lo que dice la guía, **detente** y pide ayuda (al final dice dónde). No le des clic a
cosas al azar.

---

## Lo que necesitas

- **Una PC o laptop** con al menos **4 GB de RAM** (mejor 8 GB) y **30 GB libres** en el disco.
- **Una USB de 8 GB o más.** Se borra todo lo que tenga: saca antes lo que quieras guardar.
- **Internet.** La primera vez, mejor con cable o con un Wi-Fi que jale bien.
- **Una hora** sin prisa, con la laptop conectada a la corriente.

## Primero elige: ¿qué pasa con Windows?

| | **A. Solo Vortexine's OS** | **B. Junto a Windows** |
|---|---|---|
| Qué pasa | Se **borra todo** el disco: Windows, juegos, fotos, todo. | Windows se queda. Al prender eliges cuál usar. |
| Qué tan fácil | Fácil. Es esta guía. | Más pasos: esta guía hasta el paso 3, luego [la guía junto a Windows](guia-dual-boot.md) (te avisa cuándo). |
| Para quién | PC que no tiene nada importante, o que ya respaldaste. | Si todavía necesitas Windows para algo. |

> **¿No estás seguro?** Elige **B**, o **pruébalo primero sin instalar**: la USB abre Vortexine's OS completo sin
> tocar tu disco (paso 4). Si no te convence, quitas la USB, reinicias y tu PC sigue igual que antes.

---

## Paso 1. Respalda lo importante

Copia a otra USB, a un disco externo o a la nube (Google Drive, OneDrive) tus **fotos, documentos, tareas y
partidas guardadas**. Con la opción **A** todo lo del disco se borra y **no se puede recuperar**.

## Paso 2. Baja Vortexine's OS

Vortexine's OS viene en un archivo llamado **ISO**. GitHub no deja subir archivos tan grandes, así que viene en
**3 pedazos** que luego juntas en uno.

1. Entra a: <https://github.com/MikePianoXD777/vortex-os/releases/tag/v1.6.3>
2. Baja hasta donde dice **Assets** y da clic en estos 3 archivos para bajarlos (cada uno pesa 1.4 GB):
   - `vortexines-os-1.6.3.iso.parte0`
   - `vortexines-os-1.6.3.iso.parte1`
   - `vortexines-os-1.6.3.iso.parte2`

   Espera a que **terminen los 3**. Quedan en tu carpeta **Descargas**.

   > La USB trae la versión 1.6.3, pero no te preocupes: al terminar de instalar se pone sola en la más nueva
   > (paso 7).

3. **Junta los 3 pedazos:**
   1. Abre la carpeta **Descargas** (en el Explorador de archivos, la carpeta amarilla de la barra de abajo).
   2. Da clic **arriba, en la barra donde dice «Descargas»**. Se pone azul. Borra lo que dice, escribe **cmd** y
      presiona **Enter**. Se abre una ventana negra.
   3. Copia este comando completo, pégalo en la ventana negra con **clic derecho** y presiona **Enter**:

      ```
      copy /b vortexines-os-1.6.3.iso.parte0 + vortexines-os-1.6.3.iso.parte1 + vortexines-os-1.6.3.iso.parte2 vortexines-os-1.6.3.iso
      ```

   4. Espera hasta que diga **«1 archivos copiados»**. En Descargas aparece **`vortexines-os-1.6.3.iso`** (unos
      4.3 GB). Ese es el bueno; los 3 pedazos ya los puedes borrar.

   > Si dice «El sistema no puede encontrar el archivo especificado», alguno de los 3 no terminó de bajarse o
   > quedó con otro nombre, por ejemplo `parte0 (1)`. Revisa que estén los 3, con el nombre exacto.

## Paso 3. Haz la USB

1. Conecta la USB.
2. Entra a <https://rufus.ie>, baja **Rufus** (sirve el que dice «Portable») y ábrelo. Si Windows pregunta si
   quieres que haga cambios, di **Sí**. Si pregunta por actualizaciones de Rufus, di **No**.
3. En **Dispositivo**, elige tu USB. Fíjate en el tamaño: que sea la USB y no otro disco.
4. En **Elección de arranque**, da clic en **SELECCIONAR** y elige `vortexines-os-1.6.3.iso` de tu carpeta
   Descargas.
5. Lo demás no lo toques. Da clic en **EMPEZAR**.
6. Si sale una ventana que dice «Imagen ISOHybrid detectada», elige **«Escribir en modo Imagen DD»** → **OK**.
7. Te avisa que se borrará la USB → **Aceptar**.
8. Espera a que la barra verde diga **PREPARADO**. Cierra Rufus.

> **¿Elegiste B (junto a Windows)?** Antes del paso 4, todavía en Windows, haz el **paso 1** de
> [la guía junto a Windows](guia-dual-boot.md#1-en-windows-preparar): le hace espacio a Vortexine's OS en tu disco.

## Paso 4. Prende la PC desde la USB

Con la USB conectada:

1. En Windows, abre **Inicio** → el botón de **apagar** → mantén apretada la tecla **Shift** (la flecha gorda
   de la izquierda, arriba de Ctrl) y, sin soltarla, da clic en **Reiniciar**.
2. Sale una pantalla azul de Windows. Elige **Usar un dispositivo** y luego tu **USB** (puede decir «UEFI:» y la
   marca de la USB).

   **Si no sale «Usar un dispositivo»:** apaga la PC. Al prenderla, presiona **muchas veces seguidas** la tecla
   de tu marca hasta que salga un menú, y ahí elige la USB:

   | Marca | Tecla |
   |---|---|
   | Lenovo, Dell, Acer | **F12** |
   | ASUS | **F8** o **Esc** |
   | MSI | **F11** |
   | HP | **Esc** y luego **F9** |
   | Otra o PC armada | prueba **F12**, **F11**, **F8** o **Esc** |

3. **Si sale una pantalla azul en inglés (se llama MokManager):** es normal y sale **una sola vez**. Muévete
   con las **flechas** y elige con **Enter**:
   1. Si dice «Verification failed», presiona **Enter**.
   2. **Enroll key from disk**.
   3. El que dice **ARCHISO_EFI** → **EFI** → **vortexines.cer**.
   4. **Continue** → **Yes** → **Reboot**.
   5. La PC reinicia: vuelve a elegir la USB, como en el paso 2.

4. Entras al escritorio de Vortexine's OS y se abre solo el **instalador**. Aquí todavía no se ha instalado
   nada: puedes cerrar el instalador y probarlo todo lo que quieras. Para instalarlo, ábrelo otra vez con la tecla
   **Windows** → «Instalar Vortexine's OS».

> **¿Elegiste B (junto a Windows)?** Desde aquí sigue [la guía junto a Windows](guia-dual-boot.md#4-el-instalador),
> en su paso 4 («El instalador»). No elijas «Borrar disco»: borraría Windows.

## Paso 5. Instala (opción A: solo Vortexine's OS)

1. **Idioma:** Español (México) → **Siguiente**.
2. **Ubicación:** tu ciudad, por ejemplo America/Mexico_City → **Siguiente**.
3. **Teclado:** Latinoamericano, que ya viene puesto. Escribe algo en el cuadrito de prueba para ver que la
   **ñ** y los acentos salgan bien → **Siguiente**.
4. **Particiones:**
   1. Arriba, en **«Seleccionar dispositivo de almacenamiento»**, elige el **disco de la PC**, no la USB. Guíate
      por el tamaño: la USB es la chica (8, 16 o 32 GB).
   2. Elige **«Borrar disco»**.
   3. **No marques «Cifrar sistema»**, a menos que quieras escribir una contraseña extra cada vez que prendas.
   4. **Siguiente.**
5. **Usuario:** tu nombre, un nombre de usuario (en minúsculas y sin espacios) y una **contraseña que no se te
   olvide**. Apúntala: la vas a usar para actualizar. Si quieres, marca «Iniciar sesión automáticamente».
6. **Apps:** marca las que quieras: Steam, Discord, Minecraft (Prism Launcher), Roblox (Sober), OBS… Se bajan de
   internet, así que más apps = más tiempo. Las demás las instalas después con un clic desde **Vortexine's Shop**.
7. **Resumen:** revisa que diga el disco correcto → **Instalar** → **Instalar ahora**.
8. Espera. Puede tardar de 10 a 40 minutos, según tu internet. No apagues la PC.
9. Cuando diga **Listo**, deja marcado «Reiniciar ahora» → **Hecho**. **Quita la USB** cuando la pantalla se
   ponga negra.

## Paso 6. La primera vez que prende

- **Si sale otra vez la pantalla azul en inglés (MokManager):** es para tu PC y también es **una sola vez**:
  1. **Enroll MOK** → **Continue** → **Yes**.
  2. Te pide una contraseña: escribe **1234** y **Enter**.
  3. **Reboot**.
- Entra Vortexine's OS. Si tienes tarjeta NVIDIA, sus drivers se instalan solos. Si no había internet, se
  instalan la primera vez que te conectes y te avisa que reinicies.

## Paso 7. Ponlo en la versión más nueva

1. Conéctate a internet: si es por Wi-Fi, da clic en el icono de Wi-Fi de la barra y elige tu red.
2. Unos **10 minutos** después de entrar sale un aviso: **«Hay actualizaciones»**. Da clic en **Actualizar**.

   *¿No quieres esperar?* Presiona **Windows + T** (se abre una ventana negra), escribe `vortex actualizar` y
   presiona **Enter**.
3. Te pide **tu contraseña**. **Al escribirla no se ve nada, ni puntitos**: es normal. Escríbela completa y
   presiona **Enter**.
4. Si pregunta algo con **[Y/n]**, presiona **Enter**.
5. Cuando diga **«Listo. Presiona Enter para cerrar.»**, presiona **Enter**. La pantalla parpadea un momento:
   es el escritorio cargando lo nuevo.

Desde ahí, cada vez que salga **«Hay actualizaciones»**, das clic en **Actualizar** y ya.

---

## Lo básico para empezar

- **Tecla Windows:** abre el menú para buscar y abrir apps.
- **Windows + T:** una terminal (la ventana negra). Casi nunca la vas a necesitar.
- **Vortexine's Shop:** la tienda. Ahí bajas apps con un clic, sin contraseña.
- **Algo no funciona:** búscalo con la tecla Windows. Revisa los problemas comunes y los arregla.

## Si algo sale mal

- **La USB no sale en el menú:** hazla otra vez con Rufus en **modo Imagen DD**, o conéctala en otro puerto (en
  PC de escritorio, uno de atrás).
- **La PC prende directo en Windows:** no le diste a tiempo a la tecla. Apágala y presiónala muchas veces desde
  que la prendes (tabla del paso 4).
- **«El sistema no puede encontrar el archivo» al juntar los pedazos:** revisa el paso 2. Tienen que estar los 3
  completos y con el nombre exacto.
- **Windows pide una «clave de recuperación de BitLocker»** (si elegiste B): es la llave de tu disco. Está en
  <https://account.microsoft.com/devices/recoverykey>, con la cuenta Microsoft de esa PC.
- **No te acuerdas de qué opción elegir en el instalador:** no le des a **Instalar**. Mientras no le des, no se
  ha borrado nada.

**¿Necesitas ayuda?** Escríbenos en <https://github.com/MikePianoXD777/vortex-os/issues> → **New issue**. Cuenta
en qué paso vas, qué ves en la pantalla (una foto con el cel sirve) y la marca y modelo de tu PC.
