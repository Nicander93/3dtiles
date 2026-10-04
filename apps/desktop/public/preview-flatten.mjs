/** The plane, handle and preview shader share the processor's region-centre ENU coordinates. */
export function installFlattenTool(C, viewer, getTileset, notify, canEdit) {
  const handler = new C.ScreenSpaceEventHandler(viewer.canvas);
  const camera = viewer.scene.screenSpaceCameraController;
  let state = null, entities = [], drag = null, previousShader = null, shader = null;
  let removeRender = null;
  function unlock() { if (drag) camera.enableInputs = drag.inputs; drag = null; }
  function clear() {
    unlock(); entities.forEach((e) => viewer.entities.remove(e)); entities = [];
    if (removeRender) removeRender(); removeRender = null;
    const tileset = getTileset();
    if (tileset && shader && tileset.customShader === shader) tileset.customShader = previousShader;
    if (shader && !shader.isDestroyed()) shader.destroy();
    shader = null; state = null;
    delete window.__geoforgeFlatten;
  }
  function send() { if (state) notify({ type: 'geoforge-flatten-plane', regionKey: state.key, heightMeters: state.height, initialHeightMeters: state.initial, hasUndo: state.history.length > 0 }); }
  const world = (x, y, z) => C.Matrix4.multiplyByPoint(state.toWorld, new C.Cartesian3(x,y,z), new C.Cartesian3());
  function render() {
    if (!state) return;
    entities.forEach((e) => viewer.entities.remove(e)); entities = [];
    const positions = state.xy.map(([x,y]) => world(x,y,state.height));
    entities.push(viewer.entities.add({ polygon: { hierarchy: positions, perPositionHeight: true, material: C.Color.fromCssColorString('#55c6b4').withAlpha(0.9) } }));
    entities.push(viewer.entities.add({ polyline: { positions: [...positions,positions[0]], width: 3, material: C.Color.AQUA, depthFailMaterial: C.Color.AQUA } }));
    state.xy.forEach(([x,y],i) => {
      const j = (i+1)%state.xy.length, [nx,ny] = state.xy[j];
      const a = world(x,y,state.surface[i]), b = world(nx,ny,state.surface[j]);
      const fa = positions[i], fb = positions[j];
      entities.push(viewer.entities.add({ polygon: { hierarchy: [a,fa,fb,b], perPositionHeight: true, material: C.Color.fromCssColorString('#449589').withAlpha(0.5) } }));
    });
    const base = world(0,0,state.height), tip = world(0,0,state.height+state.scale);
    entities.push(viewer.entities.add({ polyline: { positions: [base,tip], width: 12, material: new C.PolylineArrowMaterialProperty(C.Color.CYAN), depthFailMaterial: new C.PolylineArrowMaterialProperty(C.Color.CYAN) } }));
    entities.push(viewer.entities.add({ position: tip, point: { pixelSize: 18, color: C.Color.CYAN, outlineColor: C.Color.BLACK, outlineWidth: 2, disableDepthTestDistance: Infinity },
      label: { text: `↕ ${state.height.toFixed(2)} m`, font: '14px sans-serif', pixelOffset: new C.Cartesian2(0,-28), disableDepthTestDistance: Infinity } }));
    // Exposed read-only values help diagnostics and browser tests locate the actual visible handle.
    window.__geoforgeFlatten = { heightMeters: state.height, handlePosition: tip, center: base, regionKey: state.key };
    viewer.scene.requestRender(); send();
  }
  function axisHeight(pixel) {
    if (Math.abs(C.Cartesian3.dot(viewer.camera.directionWC,state.up)) > 0.98) return null;
    const ray = viewer.camera.getPickRay(pixel); if (!ray) return null;
    const delta = C.Cartesian3.subtract(ray.origin,state.origin,new C.Cartesian3());
    const b = C.Cartesian3.dot(state.up,ray.direction), denominator = 1-b*b;
    if (denominator < 1e-5) return null;
    return (C.Cartesian3.dot(state.up,delta)-b*C.Cartesian3.dot(ray.direction,delta))/denominator;
  }
  handler.setInputAction((event) => {
    if (!state?.active) return;
    const base = C.SceneTransforms.worldToWindowCoordinates(viewer.scene,world(0,0,state.height));
    const tip = C.SceneTransforms.worldToWindowCoordinates(viewer.scene,world(0,0,state.height+state.scale));
    if (!base || !tip) return;
    const dx = tip.x-base.x, dy = tip.y-base.y;
    const t = Math.max(0,Math.min(1,((event.position.x-base.x)*dx+(event.position.y-base.y)*dy)/(dx*dx+dy*dy || 1)));
    if (Math.hypot(event.position.x-base.x-t*dx,event.position.y-base.y-t*dy) > 14) return;
    const distance = C.Cartesian3.distance(viewer.camera.positionWC,state.origin);
    drag = { inputs: camera.enableInputs, start: state.height, axis: axisHeight(event.position), y: event.position.y,
      metresPerPixel: distance * 2 * Math.tan((viewer.camera.frustum.fovy || Math.PI/3)/2) / viewer.canvas.clientHeight };
    camera.enableInputs = false;
  },C.ScreenSpaceEventType.LEFT_DOWN);
  handler.setInputAction((event) => {
    if (!drag || !state) return;
    const axis = axisHeight(event.endPosition);
    const delta = axis !== null && drag.axis !== null ? axis-drag.axis : (drag.y-event.endPosition.y)*drag.metresPerPixel;
    state.height = Math.max(-10000,Math.min(10000,drag.start+delta)); render();
  },C.ScreenSpaceEventType.MOUSE_MOVE);
  handler.setInputAction(() => {
    if (!drag || !state) return;
    if (Math.abs(state.height-drag.start) > 1e-6) { state.history.push(drag.start); state.history = state.history.slice(-64); }
    unlock(); send();
  },C.ScreenSpaceEventType.LEFT_UP);
  function start(data) {
    clear(); if (!canEdit() || !getTileset()) return;
    const region = data.region;
    let ring = region?.type === 'rectangle' ? [[region.bounds[0],region.bounds[1]],[region.bounds[2],region.bounds[1]],[region.bounds[2],region.bounds[3]],[region.bounds[0],region.bounds[3]]] : region?.coordinates?.[0]?.slice(0,-1);
    if (!ring || ring.length < 3 || ring.length > 256 || ring.some((p) => p.length !== 2 || !p.every(Number.isFinite))) return;
    const lon = ring.reduce((sum,p) => sum+p[0],0)/ring.length, lat = ring.reduce((sum,p) => sum+p[1],0)/ring.length;
    const origin = C.Cartesian3.fromDegrees(lon,lat,0);
    const toWorld = C.Transforms.eastNorthUpToFixedFrame(origin), toLocal = C.Matrix4.inverse(toWorld,new C.Matrix4());
    const xy = ring.map(([x,y]) => { const p = C.Matrix4.multiplyByPoint(toLocal,C.Cartesian3.fromDegrees(x,y,0),new C.Cartesian3()); return [p.x,p.y]; });
    const samples = (data.samples || []).filter((p) => Array.isArray(p) && p.length === 3 && p.every(Number.isFinite));
    const heights = samples.map(([x,y,z]) => C.Matrix4.multiplyByPoint(toLocal,C.Cartesian3.fromDegrees(x,y,z),new C.Cartesian3()).z);
    const initial = Math.max(-10000,Math.min(10000,heights.length ? heights.reduce((a,b) => a+b,0)/heights.length : 0));
    const surface = ring.map((p) => {
      let closest = 0, distance = Infinity;
      samples.forEach((s,i) => { const d = Math.hypot(s[0]-p[0],s[1]-p[1]); if (d < distance) { closest = i; distance = d; } });
      return heights[closest] ?? initial;
    });
    state = { key: data.regionKey, xy, surface, toWorld, toLocal, origin, up: new C.Cartesian3(toWorld[8],toWorld[9],toWorld[10]),
      initial, height: initial, active: true, history: [], scale: Math.max(4,Math.min(500,C.Cartesian3.distance(viewer.camera.positionWC,origin)*0.12)) };
    // Ray crossing handles concavity in either winding; horizontal edges never cross.
    const crossings = xy.flatMap((a,i) => {
      const b=xy[(i+1)%xy.length];
      if (Math.abs(b[1]-a[1]) < 1e-12) return [];
      const slope=(b[0]-a[0])/(b[1]-a[1]), intercept=a[0]-slope*a[1];
      return [`(((p.y < ${a[1].toFixed(9)}) != (p.y < ${b[1].toFixed(9)})) && p.x < ${slope.toFixed(12)} * p.y + ${intercept.toFixed(9)} ? 1.0 : 0.0)`];
    });
    previousShader = getTileset().customShader;
    // Eye-relative matrices avoid subtracting two large ECEF values in single-precision GLSL.
    shader = new C.CustomShader({ uniforms: { u_eyeToRegion: { type: C.UniformType.MAT4, value: C.Matrix4.IDENTITY } }, fragmentShaderText:
      `void fragmentMain(FragmentInput fsInput, inout czm_modelMaterial material) { vec3 p = (u_eyeToRegion * vec4(fsInput.attributes.positionEC,1.0)).xyz; if (mod(${crossings.join(' + ') || '0.0'}, 2.0) > 0.5) { discard; } }` });
    getTileset().customShader = shader;
    const update = () => shader.setUniform('u_eyeToRegion',C.Matrix4.multiply(toLocal,viewer.camera.inverseViewMatrix,new C.Matrix4()));
    update(); removeRender = viewer.scene.preRender.addEventListener(update); render();
  }
  function command(data) {
    if (data.type !== 'geoforge-flatten') return;
    if (data.action === 'start') start(data);
    else if (data.action === 'clear') clear();
    else if (state && data.action === 'stop') { state.active = false; unlock(); }
    else if (state && data.action === 'undo' && state.history.length) { state.height = state.history.pop(); render(); }
    else if (state && data.action === 'reset') { state.history.push(state.height); state.height = state.initial; render(); }
  }
  const blur = () => { if (drag && state) { state.height = drag.start; unlock(); render(); } };
  window.addEventListener('blur',blur);
  return { command, destroy() { clear(); handler.destroy(); window.removeEventListener('blur',blur); delete window.__geoforgeFlatten; } };
}
