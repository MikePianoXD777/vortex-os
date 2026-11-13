# Vortexine's OS 3.4.0

## Lo nuevo

### Claude puede manejar y depurar un navegador (MCP de Vortex 0.3.1)

El MCP de Vortex (el que deja que Claude vea tu pantalla) ahora también puede **abrir un navegador aparte, ver páginas,
usarlas y depurarlas**. Sirve, por ejemplo, para revisar una página web que estás programando y encontrar sus errores.

- **Cómo se abre:** `vortex claude navegador` (o `vortex claude navegador brave`). Es un navegador **aparte**, con su propio perfil
  vacío: no usa tus cuentas, tus contraseñas, tus cookies ni tu historial. Al cerrarlo, olvida todo. Funciona con **Firefox**
  y con **Brave, Chromium y Chrome** (si los tienes instalados).
- **Qué puede hacer:** abrir páginas (solo `http` y `https`), leerlas, hacer clic y escribir, ver la **consola**, las **peticiones de red**
  que fallan, los errores de JavaScript, el rendimiento y los estilos, y sacar capturas.
- **Descargar y subir archivos:** baja archivos a una carpeta propia (`~/Descargas/Claude`), con un aviso cada vez, límite de tamaño y sin
  ejecutar nunca nada. Puede leer lo descargado y subirlo a una página, **pero nunca** archivos personales o secretos (tus llaves,
  tus claves, tu historial, tus notas). Lo delicado (bajar un programa, subir a un sitio de internet o desde otra carpeta) **te lo pregunta a ti
  con una notificación con botones**, y si no contestas, no se hace.
- **Privacidad:** no escribe contraseñas, no paga, no abre correo ni mensajes y no devuelve cookies ni tokens. Cada cosa que cambia algo
  avisa con una notificación (`vortex claude avisos todo` para que avise hasta cuando solo mira).
- **`vortex claude pausa`** le corta todo, y también cierra el navegador.

## Para actualizar

`vortex actualizar` (te pedirá tu contraseña).
