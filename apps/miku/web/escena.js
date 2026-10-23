// Miku en 3D: three.js + @pixiv/three-vrm (MToon, huesos de pelo y falda, expresiones y mirada).
// Se carga como módulo; si three.js o el modelo fallan, la interfaz (ui.js) sigue funcionando.
const q = new URLSearchParams(location.search);
const MODELO = q.get('modelo');                 // file:///.../modelo.vrm
const FPS = Number(q.get('fps') || 30);         // tope de cuadros: no hace falta más para una asistente
const ENCUADRE = q.get('encuadre') || 'muslo';  // cuerpo | muslo | busto
const PRUEBA = q.has('prueba');                 // solo las pruebas sin pantalla: conserva el búfer para leerlo

const clamp = (v, a = 0, b = 1) => Math.min(b, Math.max(a, v));
const miku = window.miku;
const estado = { listo: false, cuadros: 0, error: null, expresiones: [], huesos: 0 };
miku.escena = estado;

function sinModelo(titulo, orden) { miku.sinModelo(titulo, orden); }

async function iniciar() {
  if (!MODELO) {
    sinModelo('No encuentro el modelo 3D de Miku, pero puedo ayudarte igual con la voz y el texto. Para ponerle uno (un archivo .vrm):',
              'vortex-miku modelo RUTA.vrm');
    return;
  }
  let THREE, GLTFLoader, VRMLoaderPlugin, VRMUtils;
  try {
    THREE = await import('three');
    ({ GLTFLoader } = await import('three/addons/loaders/GLTFLoader.js'));
    ({ VRMLoaderPlugin, VRMUtils } = await import('@pixiv/three-vrm'));
  } catch (e) {
    estado.error = 'bibliotecas: ' + e;
    sinModelo('No pude cargar las bibliotecas del 3D (' + e + '). Sigo como asistente de voz.', '');
    return;
  }

  // ---- Render con fondo transparente de verdad (alpha + premultiplied) ----
  const lienzo = document.getElementById('lienzo');
  let renderer;
  try {
    renderer = new THREE.WebGLRenderer({
      canvas: lienzo, alpha: true, antialias: true, premultipliedAlpha: true,
      powerPreference: 'low-power', preserveDrawingBuffer: PRUEBA,
    });
  } catch (e) {
    estado.error = 'webgl: ' + e;
    sinModelo('Esta computadora no me deja dibujar en 3D (WebGL). Sigo como asistente de voz. Puedes apagar el 3D con:', 'vortex-miku avatar off');
    return;
  }
  renderer.setClearColor(0x000000, 0);
  renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));

  const escena = new THREE.Scene();
  const camara = new THREE.PerspectiveCamera(22, 1, 0.1, 30);
  const luz = new THREE.DirectionalLight(0xffffff, Math.PI);       // MToon: una luz direccional
  luz.position.set(0.6, 1.0, 1.0).normalize();
  escena.add(luz);
  const objetivoMirada = new THREE.Object3D();                      // a dónde mira Miku
  escena.add(objetivoMirada);

  function ajustarTamano() {
    const w = lienzo.clientWidth || innerWidth, h = lienzo.clientHeight || innerHeight;
    renderer.setSize(w, h, false);
    camara.aspect = w / h;
    camara.updateProjectionMatrix();
    if (vrm) encuadrar();
  }
  new ResizeObserver(ajustarTamano).observe(lienzo);

  // ---- Carga del VRM ----
  let vrm = null;
  const huesos = {};
  let caja = null;
  const cargador = new GLTFLoader();
  cargador.register((parser) => new VRMLoaderPlugin(parser));
  cargador.load(MODELO, (gltf) => {
    vrm = gltf.userData.vrm;
    VRMUtils.removeUnnecessaryVertices(gltf.scene);
    VRMUtils.combineSkeletons(gltf.scene);       // menos huesos, menos trabajo por cuadro
    VRMUtils.rotateVRM0(vrm);                    // VRM 0.x mira a -Z: se gira 180° para que mire a la cámara (+Z)
    vrm.scene.traverse((o) => { o.frustumCulled = false; });
    escena.add(vrm.scene);
    for (const n of ['hips', 'spine', 'chest', 'upperChest', 'neck', 'head', 'leftShoulder', 'rightShoulder',
                     'leftUpperArm', 'rightUpperArm', 'leftLowerArm', 'rightLowerArm']) {
      huesos[n] = vrm.humanoid.getNormalizedBoneNode(n);
    }
    if (vrm.lookAt) vrm.lookAt.target = objetivoMirada;
    estado.expresiones = Object.keys(vrm.expressionManager?.expressionMap || {});
    estado.huesos = Object.values(huesos).filter(Boolean).length;
    estado.vrm0 = vrm.meta?.metaVersion === '0';
    caja = new THREE.Box3().setFromObject(vrm.scene);
    encuadrar();
    estado.listo = true;
  }, undefined, (e) => {
    estado.error = 'modelo: ' + e;
    console.error('VRM: ' + e);
    sinModelo('No pude abrir el modelo 3D de Miku. Prueba con otro archivo .vrm:', 'vortex-miku modelo RUTA.vrm');
  });

  // Encuadre: el personaje ocupa la parte baja y deja arriba sitio para el globo de texto.
  function encuadrar() {
    if (!caja) return;
    const alto = caja.max.y - caja.min.y;
    const desde = caja.min.y + alto * ({ cuerpo: 0.0, muslo: 0.36, busto: 0.52 }[ENCUADRE] ?? 0.36);
    const tramo = caja.max.y - desde;
    const fraccion = ENCUADRE === 'cuerpo' ? 0.62 : 0.66;     // cuánto de la altura de la ventana ocupa
    const arriba = 0.265;                                      // dónde queda la cabeza (0 = borde de arriba)
    const visible = tramo / fraccion;
    const dist = visible / (2 * Math.tan(THREE.MathUtils.degToRad(camara.fov / 2)));
    const yCentro = caja.max.y - visible * (0.5 - arriba);
    camara.position.set(0, yCentro, dist);
    camara.lookAt(0, yCentro, 0);
    miraBaseY = caja.min.y + alto * 0.9;
    objetivoMirada.position.set(0, miraBaseY, dist);
  }
  let miraBaseY = 1.4;

  // ---- Vida: parpadeo, respiración, mirada, boca y emoción ----
  let tParpadeo = 2.5, faseParpadeo = -1;
  const boca = [0, 0, 0, 0, 0];                    // aa ih ou ee oh, suavizadas
  const NOMBRES_BOCA = ['aa', 'ih', 'ou', 'ee', 'oh'];
  let emocionActual = null, emocionPeso = 0;
  let miraX = 0, miraY = 0, tMirada = 2, sacadaX = 0, sacadaY = 0;
  const pose = { inclina: 0, giro: 0, adelante: 0, mirarX: 0, mirarY: 0 };   // posturas por estado, suavizadas

  function postura(nombre) {
    switch (nombre) {
      case 'escuchando': return { inclina: 0.07, giro: 0, adelante: 0.03, mirarX: 0, mirarY: 0 };
      case 'pensando': case 'viendo': return { inclina: 0.05, giro: -0.12, adelante: 0, mirarX: -0.35, mirarY: 0.3 };
      case 'transcribiendo': return { inclina: 0.04, giro: 0, adelante: 0.02, mirarX: 0, mirarY: 0 };
      default: return { inclina: 0, giro: 0, adelante: 0, mirarX: 0, mirarY: 0 };
    }
  }

  function vivir(dt, t) {
    const em = vrm.expressionManager;
    const h = huesos;

    // Parpadeo: cada 2 a 6 s; cierra en 0,07 s y abre en 0,12 s.
    tParpadeo -= dt;
    if (tParpadeo <= 0 && faseParpadeo < 0) faseParpadeo = 0;
    if (faseParpadeo >= 0) {
      faseParpadeo += dt;
      const p = faseParpadeo < 0.07 ? faseParpadeo / 0.07 : 1 - (faseParpadeo - 0.07) / 0.12;
      em.setValue('blink', clamp(p));
      if (faseParpadeo > 0.19) { faseParpadeo = -1; tParpadeo = 2 + Math.random() * 4; em.setValue('blink', 0); }
    }

    // Boca: las cinco vocales siguen la voz (ataque rápido, cierre suave).
    const objetivo = miku.bocaActual() || [0, 0, 0, 0, 0];
    let total = 0;
    for (let i = 0; i < 5; i++) {
      boca[i] += (objetivo[i] - boca[i]) * (objetivo[i] > boca[i] ? 0.6 : 0.3);
      em.setValue(NOMBRES_BOCA[i], clamp(boca[i]));
      total += boca[i];
    }

    // Emoción suave (happy, sad, relaxed...): entra y sale despacio.
    if (miku.emocionNombre !== emocionActual) {
      if (emocionActual) em.setValue(emocionActual, 0);
      emocionActual = miku.emocionNombre; emocionPeso = 0;
    }
    if (emocionActual) {
      emocionPeso += (miku.emocionPeso - emocionPeso) * 0.08;
      em.setValue(emocionActual, clamp(emocionPeso));
    }
    if (miku.estadoActual === 'reposo' && miku.emocionNombre && !miku.pista && total < 0.02) miku.emocionPeso *= 0.985;   // se apaga sola

    // Postura según lo que esté haciendo.
    const meta = postura(miku.estadoActual);
    for (const k in pose) pose[k] += (meta[k] - pose[k]) * 0.06;

    // Mirada: sacadas pequeñas cada 1-4 s alrededor del punto indicado (el ratón o el gesto de pensar).
    tMirada -= dt;
    if (tMirada <= 0) { tMirada = 1 + Math.random() * 3; sacadaX = (Math.random() - 0.5) * 0.15; sacadaY = (Math.random() - 0.5) * 0.1; }
    const pensando = miku.estadoActual === 'pensando' || miku.estadoActual === 'viendo';
    const objX = (pensando ? pose.mirarX : miku.mirada.x * 0.8) + sacadaX;
    const objY = (pensando ? pose.mirarY : miku.mirada.y * 0.5) + sacadaY;
    miraX += (objX - miraX) * 0.15; miraY += (objY - miraY) * 0.15;
    objetivoMirada.position.x = miraX;
    objetivoMirada.position.y = miraBaseY + miraY * 0.5;

    // Pose en reposo y respiración (rotaciones en el rig normalizado; signos de VRM 0.x girado con rotateVRM0).
    const r = Math.sin(t * 1.6) * 0.5 + 0.5;
    const signo = estado.vrm0 === false ? -1 : 1;                   // en VRM 1.0 los brazos giran al revés
    h.leftUpperArm.rotation.z = 1.25 * signo;  h.rightUpperArm.rotation.z = -1.25 * signo;       // brazos abajo
    h.leftLowerArm.rotation.z = 0.12 * signo;  h.rightLowerArm.rotation.z = -0.12 * signo;
    h.spine.rotation.x = 0.012 * r + pose.adelante * 0.5;
    h.chest.rotation.x = 0.018 * r + pose.adelante * 0.5;
    if (h.upperChest) h.upperChest.rotation.x = 0.010 * r;
    h.leftShoulder.rotation.z = 0.02 * r * signo;  h.rightShoulder.rotation.z = -0.02 * r * signo;
    // Al hablar, asiente muy poco con la fuerza de la voz.
    h.head.rotation.y = Math.sin(t * 0.4) * 0.04 + pose.giro;
    h.head.rotation.x = Math.sin(t * 0.55 + 1) * 0.015 + total * 0.03 * Math.sin(t * 5.0);
    h.head.rotation.z = pose.inclina;
    h.hips.rotation.y = Math.sin(t * 0.25) * 0.02;
  }

  // ---- Bucle con tope de cuadros por segundo ----
  let ultimo = performance.now(), activo = true;
  const t0 = ultimo;
  function bucle(ahora) {
    if (!activo) return;
    requestAnimationFrame(bucle);
    const dt = (ahora - ultimo) / 1000;
    if (dt < 1 / FPS - 0.002) return;
    ultimo = ahora;
    if (!vrm) return;
    const paso = Math.min(dt, 0.1);
    vivir(paso, (ahora - t0) / 1000);
    vrm.update(paso);                               // después de tocar huesos y expresiones
    renderer.render(escena, camara);
    estado.cuadros++;
  }
  ajustarTamano();
  requestAnimationFrame(bucle);

  // Para pausar al ocultar la ventana y para las pruebas.
  miku.pausar = () => { activo = false; };
  miku.reanudar = () => { if (!activo) { activo = true; ultimo = performance.now(); requestAnimationFrame(bucle); } };
  miku.captura = () => {                            // pinta un cuadro y devuelve el PNG con alfa (pruebas)
    vivir(0.016, (performance.now() - t0) / 1000); vrm.update(0.016); renderer.render(escena, camara);
    return lienzo.toDataURL('image/png');
  };
  miku.forzar = (o) => { for (const k in o) vrm.expressionManager.setValue(k, o[k]); };
  miku.info = () => {
    const gl = renderer.getContext();
    const e = gl.getExtension('WEBGL_debug_renderer_info');
    return { gl: gl.getParameter(gl.VERSION), gpu: e ? gl.getParameter(e.UNMASKED_RENDERER_WEBGL) : '?',
             three: THREE.REVISION, tris: renderer.info.render.triangles, calls: renderer.info.render.calls,
             tamano: [lienzo.width, lienzo.height], expresiones: estado.expresiones };
  };
}

iniciar().catch((e) => { estado.error = String(e); console.error(e); });
