// Vortex: vortex-eventos, el oído de vortex-gestor.
//
// Escucha a Hyprland (.socket2.sock) y la presión de memoria del kernel (PSI) y escribe
// una línea por cada cosa que le importa al gestor. No despierta la CPU: todo es poll()
// sobre descriptores, y solo hay un plazo mientras se espera a que el foco se quede
// quieto (antirrebote: pasar el ratón por encima de varias ventanas no genera trabajo).
//
//   vortex-eventos SOCKET2 SOCKET [PSI [UMBRAL_US VENTANA_US [ANTIRREBOTE_MS]]]
//
// SOCKET2 es el de eventos de Hyprland y SOCKET el de órdenes (para preguntar qué ventana
// quedó enfocada). PSI es /proc/pressure/memory (o el memory.pressure de un cgroup); con
// «-» no se vigila. Sin privilegios, VENTANA_US tiene que ser múltiplo de 2 s.
//
// Salida: campos separados por \037 (separador de unidad: ningún campo lo lleva y, a
// diferencia del tabulador, bash no junta dos seguidos, así que un campo vacío se respeta).
//   foco  PID DIRECCION TIPO CLASE   ventana enfocada (PID 0 si no hay ninguna)
//   abre  DIRECCION TIPO CLASE       ventana nueva (al arrancar, una por cada abierta)
//   cierra DIRECCION
//   psi   memoria                    la presión de memoria pasó el umbral
//   psi   no MOTIVO                  no se puede vigilar la presión
// DIRECCION va sin «0x», como en los eventos de Hyprland; TIPO es el contentType de la
// ventana («game», «video»...), vacío si no se sabe.
//
// Termina con 0 cuando Hyprland cierra el socket (fin de la sesión) y con 2 si no puede
// conectarse.
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/time.h>
#include <sys/un.h>
#include <time.h>
#include <unistd.h>

#define SEP "\037"

static const char *socket_ordenes;
static char respuesta[512 * 1024];

static int conectar(const char *ruta) {
    struct sockaddr_un dir = {.sun_family = AF_UNIX};
    if (strlen(ruta) >= sizeof dir.sun_path) {
        errno = ENAMETOOLONG;
        return -1;
    }
    strcpy(dir.sun_path, ruta);
    int fd = socket(AF_UNIX, SOCK_STREAM | SOCK_CLOEXEC, 0);
    if (fd < 0) return -1;
    if (connect(fd, (struct sockaddr *)&dir, sizeof dir) < 0) {
        int e = errno;
        close(fd);
        errno = e;
        return -1;
    }
    return fd;
}

static long long ahora_ms(void) {
    struct timespec t;
    clock_gettime(CLOCK_MONOTONIC, &t);
    return t.tv_sec * 1000LL + t.tv_nsec / 1000000;
}

// Pregunta algo a Hyprland por el socket de órdenes y lee la respuesta entera (Hyprland
// cierra la conexión al terminar). Con tope de 2 s para no quedarse colgado nunca.
static size_t pedir(const char *orden) {
    respuesta[0] = 0;
    int fd = conectar(socket_ordenes);
    if (fd < 0) return 0;
    struct timeval tope = {.tv_sec = 2};
    setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &tope, sizeof tope);
    setsockopt(fd, SOL_SOCKET, SO_SNDTIMEO, &tope, sizeof tope);
    size_t largo = strlen(orden), hecho = 0;
    while (hecho < largo) {
        ssize_t n = write(fd, orden + hecho, largo - hecho);
        if (n <= 0) {
            close(fd);
            return 0;
        }
        hecho += (size_t)n;
    }
    size_t total = 0;
    ssize_t n;
    while (total < sizeof respuesta - 1 && (n = read(fd, respuesta + total, sizeof respuesta - 1 - total)) > 0)
        total += (size_t)n;
    close(fd);
    respuesta[total] = 0;
    return total;
}

// Lo que sigue a «"clave":» entre p y fin (o NULL).
static const char *valor(const char *p, const char *fin, const char *clave) {
    char patron[64];
    int largo = snprintf(patron, sizeof patron, "\"%s\":", clave);
    if (!p || largo <= 0 || p >= fin) return NULL;
    const char *q = memmem(p, (size_t)(fin - p), patron, (size_t)largo);
    if (!q) return NULL;
    for (q += largo; q < fin && (*q == ' ' || *q == '\t' || *q == '\n' || *q == '\r'); q++) {
    }
    return q < fin ? q : NULL;
}

// Copia un valor de texto JSON sin comillas. Los tabuladores, saltos de línea y el
// separador de la salida se cambian por espacios para que la línea no se rompa.
static void texto(const char *v, const char *fin, char *sal, size_t tam) {
    size_t n = 0;
    if (v && *v == '"') {
        for (v++; v < fin && *v != '"' && n + 1 < tam; v++) {
            char c = *v;
            if (c == '\\' && v + 1 < fin) {
                c = *++v;
                if (c == 'u') {
                    v += (fin - v > 4) ? 4 : 0;
                    c = '?';
                } else if (c == 'n' || c == 't' || c == 'r' || c == 'b' || c == 'f') {
                    c = ' ';
                }
            }
            if ((unsigned char)c < 0x20) c = ' ';
            sal[n++] = c;
        }
    }
    sal[n] = 0;
}

// Quita el «0x» de una dirección.
static const char *sin_0x(const char *d) { return (d[0] == '0' && d[1] == 'x') ? d + 2 : d; }

static char ultima[64];  // dirección de la última ventana enfocada que se informó

static void informar_foco(void) {
    size_t n = pedir("j/activewindow");
    const char *fin = respuesta + n;
    const char *v = valor(respuesta, fin, "pid");
    long pid = v ? strtol(v, NULL, 10) : 0;
    char dir[64] = "", tipo[64] = "", clase[256] = "";
    if (pid > 0) {
        texto(valor(respuesta, fin, "address"), fin, dir, sizeof dir);
        texto(valor(respuesta, fin, "contentType"), fin, tipo, sizeof tipo);
        texto(valor(respuesta, fin, "class"), fin, clase, sizeof clase);
    } else {
        pid = 0;
    }
    snprintf(ultima, sizeof ultima, "%s", sin_0x(dir));
    printf("foco" SEP "%ld" SEP "%s" SEP "%s" SEP "%s\n", pid, sin_0x(dir), tipo, clase);
}

// Al arrancar: una línea «abre» por cada ventana que ya está abierta (si el gestor se
// reinicia con un juego abierto, así lo sabe).
static void informar_abiertas(void) {
    size_t n = pedir("j/clients");
    const char *fin = respuesta + n;
    // «address» es la primera clave de cada ventana: lo que va hasta la siguiente es suyo.
    for (const char *p = valor(respuesta, fin, "address"), *sig; p; p = sig) {
        sig = valor(p, fin, "address");
        const char *tope = sig ? sig : fin;
        char dir[64], tipo[64], clase[256];
        texto(p, tope, dir, sizeof dir);
        texto(valor(p, tope, "contentType"), tope, tipo, sizeof tipo);
        texto(valor(p, tope, "class"), tope, clase, sizeof clase);
        if (dir[0]) printf("abre" SEP "%s" SEP "%s" SEP "%s\n", sin_0x(dir), tipo, clase);
    }
}

static int pendiente;         // hay un cambio de foco esperando a que pase el antirrebote
static long long plazo;       // cuándo informarlo (ms, reloj monotónico)
static long antirrebote = 120;
static char pedida[64];       // la ventana del último evento de foco mientras se espera

// Una línea del socket de eventos.
static void evento(char *l) {
    if (strncmp(l, "activewindowv2>>", 16) == 0) {
        const char *dir = sin_0x(l + 16);
        // Hyprland repite el evento cuando cambia el título de la ventana enfocada:
        // si es la misma ventana no hay nada que hacer (ni hay que alargar la espera).
        if (strcmp(dir, pendiente ? pedida : ultima) == 0) return;
        // El plazo se corre con cada ventana distinta: se informa cuando el foco se queda quieto.
        plazo = ahora_ms() + antirrebote;
        snprintf(pedida, sizeof pedida, "%s", dir);
        pendiente = 1;
    } else if (strncmp(l, "openwindow>>", 12) == 0) {
        // DIRECCION,ESPACIO,CLASE,TITULO
        char *dir = l + 12, *esp = strchr(dir, ','), *clase = esp ? strchr(esp + 1, ',') : NULL;
        if (!clase) return;
        *esp = 0;
        clase++;
        char *coma = strchr(clase, ',');
        if (coma) *coma = 0;
        for (char *c = clase; *c; c++)
            if ((unsigned char)*c < 0x20) *c = ' ';
        printf("abre" SEP "%s" SEP SEP "%s\n", sin_0x(dir), clase);
    } else if (strncmp(l, "closewindow>>", 13) == 0) {
        const char *dir = sin_0x(l + 13);
        // Se cerró la enfocada: lo que venga después (aunque sea «ninguna») se informa.
        if (strcmp(dir, ultima) == 0) strcpy(ultima, "?");
        printf("cierra" SEP "%s\n", dir);
    }
}

int main(int argc, char **argv) {
    if (argc < 3) {
        fprintf(stderr, "uso: %s SOCKET2 SOCKET [PSI [UMBRAL_US VENTANA_US [ANTIRREBOTE_MS]]]\n", argv[0]);
        return 64;
    }
    socket_ordenes = argv[2];
    const char *psi = argc > 3 ? argv[3] : "-";
    long umbral = argc > 4 ? strtol(argv[4], NULL, 10) : 150000;
    long ventana = argc > 5 ? strtol(argv[5], NULL, 10) : 2000000;
    if (argc > 6) antirrebote = strtol(argv[6], NULL, 10);
    if (antirrebote < 0) antirrebote = 0;

    setvbuf(stdout, NULL, _IOLBF, 0);

    int eventos = conectar(argv[1]);
    if (eventos < 0) {
        fprintf(stderr, "vortex-eventos: no puedo conectarme a %s: %s\n", argv[1], strerror(errno));
        return 2;
    }

    // Presión de memoria: el kernel avisa (POLLPRI) cuando, dentro de una ventana, las
    // tareas pasaron más de UMBRAL esperando memoria. Un aviso por ventana como mucho.
    int presion = -1;
    if (strcmp(psi, "-") != 0) {
        char disparador[64];
        snprintf(disparador, sizeof disparador, "some %ld %ld", umbral, ventana);
        presion = open(psi, O_RDWR | O_NONBLOCK | O_CLOEXEC);
        if (presion < 0 || write(presion, disparador, strlen(disparador) + 1) < 0) {
            printf("psi" SEP "no" SEP "%s\n", strerror(errno));
            if (presion >= 0) close(presion);
            presion = -1;
        }
    }

    informar_abiertas();
    pendiente = 1;  // y la ventana enfocada ahora mismo
    plazo = ahora_ms();

    static char buf[64 * 1024];
    size_t usado = 0;
    for (;;) {
        struct pollfd fds[2] = {{.fd = eventos, .events = POLLIN}, {.fd = presion, .events = POLLPRI}};
        int espera = -1;
        if (pendiente) {
            long long falta = plazo - ahora_ms();
            espera = falta > 0 ? (int)falta : 0;
        }
        int n = poll(fds, presion >= 0 ? 2 : 1, espera);
        if (n < 0) {
            if (errno == EINTR) continue;
            return 1;
        }
        if (fds[0].revents) {
            ssize_t r = read(eventos, buf + usado, sizeof buf - 1 - usado);
            if (r <= 0) {
                if (r < 0 && (errno == EINTR || errno == EAGAIN)) continue;
                return 0;  // Hyprland se cerró
            }
            usado += (size_t)r;
            buf[usado] = 0;
            char *ini = buf, *nl;
            while ((nl = memchr(ini, '\n', usado - (size_t)(ini - buf)))) {
                *nl = 0;
                evento(ini);
                ini = nl + 1;
            }
            usado -= (size_t)(ini - buf);
            memmove(buf, ini, usado);
            if (usado == sizeof buf - 1) usado = 0;  // línea absurda: se descarta
        }
        if (presion >= 0 && fds[1].revents) {
            if (fds[1].revents & (POLLERR | POLLNVAL)) {
                printf("psi" SEP "no" SEP "el kernel cerró el aviso\n");
                close(presion);
                presion = -1;
            } else if (fds[1].revents & POLLPRI) {
                printf("psi" SEP "memoria\n");
            }
        }
        if (pendiente && ahora_ms() >= plazo) {
            pendiente = 0;
            informar_foco();
        }
    }
}
