//! Lo poco que pide el sistema y la biblioteca estándar no da: poll() con POLLPRI (presión de memoria),
//! las señales por un descriptor (signalfd, así el bucle principal no necesita manejadores), el uid y
//! `setpriority` (el nice de un hilo, sin lanzar `renice`). Es el único módulo con `unsafe`.
#![allow(unsafe_code)]

use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::path::Path;

pub fn uid() -> u32 {
    // SAFETY: getuid no tiene errores ni argumentos.
    unsafe { libc::getuid() }
}

pub fn euid() -> u32 {
    // SAFETY: geteuid no tiene errores ni argumentos.
    unsafe { libc::geteuid() }
}

/// poll() sobre los descriptores; devuelve cuántos tienen algo. `ms` negativo = sin plazo.
pub fn sondear(fds: &mut [libc::pollfd], ms: i32) -> io::Result<i32> {
    // SAFETY: el puntero y el largo salen de un slice válido.
    let r = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, ms) };
    if r < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(r)
    }
}

pub fn pollfd(fd: RawFd, events: i16) -> libc::pollfd {
    libc::pollfd {
        fd,
        events,
        revents: 0,
    }
}

/// El nice de un hilo (lo mismo que hace `renice --priority N -p TID`).
pub fn poner_nice(tid: u32, nice: i32) -> io::Result<()> {
    // SAFETY: setpriority solo recibe enteros.
    let r = unsafe { libc::setpriority(libc::PRIO_PROCESS, tid as libc::id_t, nice) };
    if r < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

/// Equivalente de `[[ -w ruta ]]` / `[[ -r ruta ]]`.
pub fn acceso(ruta: &Path, modo: i32) -> bool {
    use std::os::unix::ffi::OsStrExt;
    let mut c = ruta.as_os_str().as_bytes().to_vec();
    c.push(0);
    // SAFETY: `c` termina en NUL y vive durante la llamada.
    unsafe { libc::access(c.as_ptr().cast(), modo) == 0 }
}

pub fn se_puede_escribir(ruta: &Path) -> bool {
    acceso(ruta, libc::W_OK)
}

pub fn se_puede_leer(ruta: &Path) -> bool {
    acceso(ruta, libc::R_OK)
}

/// Bloquea TERM, INT, HUP y CHLD en este hilo (los demás hilos lo heredan, y los programas que se lanzan
/// lo dejan como estaba: std restablece la máscara antes del exec) y los entrega por un descriptor.
/// Hay que llamarlo antes de crear ningún otro hilo.
pub fn senales() -> io::Result<OwnedFd> {
    // SAFETY: sigset_t se inicializa con sigemptyset antes de usarse; signalfd recibe -1 para crear uno nuevo.
    unsafe {
        let mut set: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut set);
        for s in [libc::SIGTERM, libc::SIGINT, libc::SIGHUP, libc::SIGCHLD] {
            libc::sigaddset(&mut set, s);
        }
        if libc::pthread_sigmask(libc::SIG_BLOCK, &set, std::ptr::null_mut()) != 0 {
            return Err(io::Error::last_os_error());
        }
        let fd = libc::signalfd(-1, &set, libc::SFD_CLOEXEC | libc::SFD_NONBLOCK);
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(OwnedFd::from_raw_fd(fd))
    }
}

/// Lee todas las señales pendientes y dice cuáles son: (hubo_parada, hubo_hijo).
pub fn leer_senales(fd: &OwnedFd) -> (bool, bool) {
    let (mut parada, mut hijo) = (false, false);
    loop {
        // SAFETY: se lee a un buffer del tamaño exacto de signalfd_siginfo.
        let mut info: libc::signalfd_siginfo = unsafe { std::mem::zeroed() };
        let n = unsafe {
            libc::read(
                fd.as_raw_fd(),
                (&mut info as *mut libc::signalfd_siginfo).cast(),
                std::mem::size_of::<libc::signalfd_siginfo>(),
            )
        };
        if n != std::mem::size_of::<libc::signalfd_siginfo>() as isize {
            return (parada, hijo);
        }
        if info.ssi_signo as i32 == libc::SIGCHLD {
            hijo = true;
        } else {
            parada = true;
        }
    }
}

/// Que el programa lanzado arranque con las señales desbloqueadas (la máscara se hereda con el exec: sin
/// esto, un `timeout` o un programa que no oye TERM se quedaría colgado).
pub fn hijo_sin_mascara(cmd: &mut std::process::Command) {
    use std::os::unix::process::CommandExt;
    // SAFETY: entre el fork y el exec solo se llama a funciones async-signal-safe (sigemptyset y
    // pthread_sigmask sobre un sigset_t local).
    unsafe {
        cmd.pre_exec(|| {
            let mut set: libc::sigset_t = std::mem::zeroed();
            libc::sigemptyset(&mut set);
            libc::pthread_sigmask(libc::SIG_SETMASK, &set, std::ptr::null_mut());
            Ok(())
        });
    }
}

/// 4.5: un descriptor de inotify (bloqueante: lo lee un hilo propio que duerme mientras no pase nada).
pub fn inotify_nuevo() -> io::Result<OwnedFd> {
    // SAFETY: inotify_init1 solo recibe banderas y devuelve un descriptor nuevo o -1.
    let fd = unsafe { libc::inotify_init1(libc::IN_CLOEXEC) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: el descriptor es nuevo y solo lo tiene este OwnedFd.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

/// Vigila `ruta` con la máscara de inotify; devuelve el «watch descriptor».
pub fn inotify_vigilar(fd: &impl AsRawFd, ruta: &Path, mascara: u32) -> io::Result<i32> {
    use std::os::unix::ffi::OsStrExt;
    let mut c = ruta.as_os_str().as_bytes().to_vec();
    c.push(0);
    // SAFETY: la ruta termina en 0 y vive durante la llamada; fd es un inotify abierto.
    let wd = unsafe { libc::inotify_add_watch(fd.as_raw_fd(), c.as_ptr().cast(), mascara) };
    if wd < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(wd)
}
