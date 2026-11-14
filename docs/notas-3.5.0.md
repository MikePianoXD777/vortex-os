# Vortexine's OS 3.5.0

## Lo nuevo

### Miku ve el cuarto con la cámara (Miku 2.2)

Miku ahora puede **mirar por tu webcam** cuando se lo pides: «Miku, mira el cuarto» (te dice si está ordenado, más o menos o
desordenado, con un par de cosas concretas) o «¿qué estoy haciendo?». Toma **una sola foto**, nunca video.

- **Apagado de fábrica.** No pasa nada hasta que lo actives tú. Un ajuste aparte permite o no que Miku use la cámara.
- **Nada a escondidas:** cada foto muestra el indicador «Usando la cámara» y manda una notificación. La foto vive solo en memoria o
  en una carpeta temporal privada y se **borra al terminar**. Va únicamente a la IA que elegiste (Claude o tu IA local), no se guarda y no se sube a ningún otro lado.
- `vortex claude pausa` corta la cámara.

### Modo compañera (opcional, apagado de fábrica)

Miku puede **hablarte por iniciativa propia**: preguntarte cómo vas, comentar el cuarto, qué andas haciendo, recordarte descansar.
Actívalo en **Ajustes → General → Asistente → Modo compañera** (la primera vez te explica todo y tienes que aceptar).

- **Antes de cada foto avisa con una cuenta de 5 segundos** que puedes cancelar (diciendo «no», con el botón o con Esc).
- **Límites:** como mucho una vez cada 45 min y 6 al día; nunca de 23:00 a 09:00, ni si estás jugando, en pantalla completa, en una llamada o con la pantalla bloqueada.
- **Tú mandas:** «Miku, ya no me molestes» (hasta las 9 del otro día), «Miku, cállate un rato» (2 horas) o «apaga el modo compañera».
- **Bitácora local, sin imágenes:** puedes ver y borrar cada vez que usó la cámara o te habló por su cuenta, desde Ajustes.
- **Tono:** cariñosa (de fábrica) o suave. Dice que es una IA; sin nada sexual, sin celos ni culpa.
- **Solo para mayores de 18:** una casilla aparte, apagada, deja que Miku se asome **sin cuenta atrás**. Se marca únicamente desde la ventana de Miku, confirmando que eres mayor de edad. El indicador, la notificación y la bitácora siguen en cada foto.
- **La IA no puede dar permisos por ti:** ni el consentimiento ni la casilla se pueden activar con comandos que corra la IA.

## Para actualizar

`vortex actualizar` (te pedirá tu contraseña). Se reconstruyen el shell y Miku.
