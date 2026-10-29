// Miku: la parte de interfaz de la página (globo, estado, escribir, micrófono, crédito).
// No necesita three.js: si el 3D no carga, esto sigue funcionando (voz + texto).
// Python maneja la página con runJavaScript("miku.xxx(...)") y recibe por QWebChannel (objeto «puente»).
(function () {
  'use strict';
  const $ = (id) => document.getElementById(id);
  const q = new URLSearchParams(location.search);

  const ETIQUETAS = {
    reposo: 'Lista', escuchando: 'Escuchando...', transcribiendo: 'Un momento...',
    pensando: 'Pensando...', viendo: 'Mirando tu pantalla...', hablando: 'Hablando',
  };

  const miku = {
    puente: null,
    estadoActual: 'reposo',
    // Estado que lee la escena 3D en cada cuadro.
    emocionNombre: null, emocionPeso: 0,
    sonrojoMeta: 0, sonrojoHasta: 0,   // se sonroja con esta fuerza hasta esta hora (performance.now)
    pista: null,                       // visemas en curso: {datos, hz, t0}
    mirada: { x: 0, y: 0 },
    // Cuántos ms tarda el audio en salir desde que Python manda empezar (pw-play arranca y llena su búfer).
    retrasoAudioMs: Number(q.get('retraso') || 110),
  };
  window.miku = miku;

  // ---- Puente con Python ----------------------------------------------------------------------
  function conectar() {
    if (typeof QWebChannel === 'undefined' || typeof qt === 'undefined') return;
    new QWebChannel(qt.webChannelTransport, (canal) => {
      miku.puente = canal.objects.puente;
      if (miku.puente) miku.puente.listo();
    });
  }
  const aPython = (f) => { try { if (miku.puente) f(miku.puente); } catch (e) { console.error(e); } };

  // ---- Interfaz -------------------------------------------------------------------------------
  miku.estado = (nombre, texto) => {
    miku.estadoActual = nombre;
    const e = $('estado');
    e.className = nombre;
    $('estado-texto').textContent = texto || ETIQUETAS[nombre] || nombre;
    const mic = $('mic');
    mic.classList.toggle('activo', nombre === 'escuchando');
    if (nombre !== 'escuchando') mic.style.removeProperty('--nivel');
    $('texto').classList.toggle('pensando', (nombre === 'pensando' || nombre === 'viendo') && !$('texto').textContent);
    if ((nombre === 'pensando' || nombre === 'viendo') && $('globo').hidden) mostrarGlobo();
  };

  function mostrarGlobo() { $('globo').hidden = false; $('credito').hidden = true; }

  miku.usuario = (texto) => {
    $('tu').textContent = texto ? 'Tú: ' + texto : '';
    $('texto').textContent = '';
    $('texto').classList.remove('aviso');
    mostrarGlobo();
  };

  miku.respuesta = (texto, final) => {
    const t = $('texto');
    t.classList.remove('aviso');
    t.textContent = texto;
    t.classList.toggle('pensando', !final && !texto);
    mostrarGlobo();
    const g = $('globo');
    g.scrollTop = g.scrollHeight;
  };

  miku.aviso = (texto) => {
    const t = $('texto');
    t.textContent = texto;
    t.classList.add('aviso');
    t.classList.remove('pensando');
    mostrarGlobo();
  };

  miku.nivel = (v) => { $('mic').style.setProperty('--nivel', String(Math.max(0, Math.min(1, v)).toFixed(2))); };

  miku.credito = (texto, resumen) => {
    $('credito').textContent = texto || 'Este modelo 3D no trae un archivo CREDITO.txt.';
    $('pie').textContent = resumen || '';
  };

  // Colores de la paleta del fondo de pantalla (Python manda un mapa de variables CSS).
  miku.tema = (vars) => {
    const r = document.documentElement.style;
    for (const k in vars) r.setProperty('--' + k, vars[k]);
  };

  // Medidor de cariño (2.0): el corazón se llena de 0 a 100; al cambiar, late y muestra cuánto.
  const NIVELES = { conocidos: 'Conocidos', amigos: 'Amigos', cercanos: 'Cercanos', 'muy cercanos': 'Muy cercanos',
                    'le gusta': 'Le gustas', enamorada: 'Enamorada', novios: 'Novios', casados: 'Casados 💍' };
  let afectoPrevio = null;
  miku.afecto = (valor, nivel) => {
    const c = $('carino');
    if (valor < 0) { c.hidden = true; afectoPrevio = null; return; }
    const v = Math.max(0, Math.min(100, valor));
    c.hidden = false;
    $('carino-nivel').setAttribute('y', String(21 - 16 * v / 100));      // el corazón va de y=5 a y=21
    $('carino-nombre').textContent = NIVELES[nivel] || nivel;
    $('carino-num').textContent = Math.round(v);
    c.classList.toggle('enamorada', ['enamorada', 'novios', 'casados'].includes(nivel));
    acomodarBoda();
    c.title = `Cariño de Miku: ${Math.round(v)} de 100. Crece despacio con cómo la tratas. Lo que siente y lo que sabe de ti está en Documentos/Miku.`;
    if (afectoPrevio !== null && Math.abs(v - afectoPrevio) >= 0.25) {
      const d = v - afectoPrevio, cambio = $('carino-cambio');
      cambio.textContent = (d > 0 ? '+' : '−') + Math.abs(d).toLocaleString('es', { maximumFractionDigits: 1 });
      cambio.classList.toggle('baja', d < 0);
      const ancla = $('b-boda').hidden ? c : $('b-boda');       // 2.1: después del botón de matrimonio, si está
      cambio.style.left = (ancla.offsetLeft + ancla.offsetWidth + 6) + 'px';
      cambio.classList.remove('sube'); void cambio.offsetWidth; cambio.classList.add('sube');
      if (d > 0) { c.classList.remove('late'); void c.offsetWidth; c.classList.add('late'); }
    }
    afectoPrevio = v;
  };

  // 2.1: «lista» → el botón «Pedirle matrimonio» (manda la pregunta como si la escribieras); «boda» → ¡se
  // casaron!: lluvia de corazones; «casados» o «» → sin botón.
  const acomodarBoda = () => {
    const c = $('carino'), b = $('b-boda');
    b.style.left = (c.hidden ? 10 : c.offsetLeft + c.offsetWidth + 8) + 'px';
  };
  $('b-boda').addEventListener('click', () => {
    $('b-boda').hidden = true;
    aPython((p) => p.enviarTexto('¿Te quieres casar conmigo?'));
  });
  miku.boda = (estado) => {
    $('b-boda').hidden = estado !== 'lista';
    acomodarBoda();
    if (estado !== 'boda') return;
    const caja = $('corazones');
    for (let i = 0; i < 40; i++) {
      const h = document.createElement('span');
      h.textContent = i % 7 === 0 ? '💍' : '♥';
      h.style.left = (Math.random() * 100) + '%';
      h.style.fontSize = (12 + Math.random() * 18) + 'px';
      h.style.animationDuration = (3 + Math.random() * 3) + 's';
      h.style.animationDelay = (Math.random() * 1.5) + 's';
      caja.appendChild(h);
      setTimeout(() => h.remove(), 8000);
    }
  };

  // Primera vez: descarga de la voz y el oído. fraccion < 0 = ya no hay descarga.
  miku.progreso = (titulo, detalle, fraccion) => {
    const p = $('prep');
    if (fraccion < 0) { p.hidden = true; return; }
    $('prep-titulo').textContent = titulo;
    $('prep-detalle').textContent = detalle;
    $('prep-relleno').style.width = Math.round(Math.max(0, Math.min(1, fraccion)) * 100) + '%';
    p.hidden = false;
    mostrarGlobo();
  };

  // Botones bajo la respuesta: [[id, texto], ...] (vacía = ninguno).
  miku.acciones = (lista) => {
    const a = $('acciones');
    a.textContent = '';
    for (const [id, texto] of lista || []) {
      const b = document.createElement('button');
      b.type = 'button';
      b.textContent = texto;
      b.addEventListener('click', () => aPython((p) => p.accion(id)));
      a.appendChild(b);
    }
    a.hidden = !(lista && lista.length);
    if (!a.hidden) mostrarGlobo();
  };

  miku.sinModelo = (texto, html) => {
    const c = $('sin-modelo');
    c.hidden = !texto;
    if (texto) { c.innerHTML = ''; const p = document.createElement('div'); p.textContent = texto; c.appendChild(p);
      if (html) { const k = document.createElement('code'); k.textContent = html; c.appendChild(k); } }
  };

  // ---- Boca, emoción y mirada (la escena 3D las lee) ---------------------------------------
  // `pista`: un cuadro por 1/hz s con [aa, ih, ou, ee, oh] de 0 a 100.
  miku.visemas = (datos, hz) => { miku.pista = { datos, hz: hz || 30, t0: performance.now() + miku.retrasoAudioMs }; };
  miku.pararBoca = () => { miku.pista = null; };
  miku.bocaActual = () => {
    const p = miku.pista;
    if (!p) return null;
    const x = (performance.now() - p.t0) / 1000 * p.hz;
    if (x < 0) return [0, 0, 0, 0, 0];
    const i = Math.floor(x);
    if (i >= p.datos.length - 1) { if (i > p.datos.length + 3) miku.pista = null; return [0, 0, 0, 0, 0]; }
    const f = x - i, a = p.datos[i], b = p.datos[i + 1];
    return a.map((v, k) => (v + (b[k] - v) * f) / 100);
  };
  miku.emocion = (nombre, peso) => { miku.emocionNombre = nombre || null; miku.emocionPeso = peso == null ? 1 : peso; };
  miku.sonrojo = (v) => {                        // dura más cuanto más fuerte (de 4 a 10 s)
    miku.sonrojoMeta = Math.max(0, Math.min(1, v));
    miku.sonrojoHasta = performance.now() + 4000 + miku.sonrojoMeta * 6000;
  };
  miku.mirar = (x, y) => { miku.mirada.x = Math.max(-1, Math.min(1, x)); miku.mirada.y = Math.max(-1, Math.min(1, y)); };

  // ---- Eventos ----------------------------------------------------------------------------------
  $('b-cerrar').addEventListener('click', () => aPython((p) => p.cerrar()));
  $('b-info').addEventListener('click', () => {
    const c = $('credito');
    c.hidden = !c.hidden;
    if (!c.hidden) $('globo').hidden = true; else if ($('texto').textContent) mostrarGlobo();
  });
  $('mic').addEventListener('click', () => aPython((p) => p.microfono()));
  $('form').addEventListener('submit', (e) => {
    e.preventDefault();
    const t = $('entrada').value.trim();
    if (!t) return;
    $('entrada').value = '';
    aPython((p) => p.enviarTexto(t));
  });
  $('entrada').addEventListener('input', () => aPython((p) => p.tocada()));
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') { e.preventDefault(); aPython((p) => p.cerrar()); return; }
    // Escribir en cualquier parte de la ventana escribe en el cuadro.
    if (e.key.length === 1 && !e.ctrlKey && !e.altKey && !e.metaKey && document.activeElement !== $('entrada')) $('entrada').focus();
  });
  let ultimaTocada = 0;
  document.addEventListener('pointermove', (e) => {
    miku.mirar((e.clientX / innerWidth) * 2 - 1, -((e.clientY / innerHeight) * 2 - 1));
    const ahora = performance.now();
    if (ahora - ultimaTocada > 1000) { ultimaTocada = ahora; aPython((p) => p.tocada()); }
  });
  document.addEventListener('contextmenu', (e) => e.preventDefault());

  conectar();
})();
