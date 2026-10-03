/** Drawing owns only its entities and input handler; the viewer remains reusable for other tools. */
export function installRegionDrawing(Cesium, viewer, notify, canDraw) {
  let mode = 'polygon', points = [], complete = false, active = false;
  let entities = [], drag = -1, anchor = null, savedInputs = null;
  const controller = viewer.scene.screenSpaceCameraController;
  const handler = new Cesium.ScreenSpaceEventHandler(viewer.canvas);
  const send = () => notify({ type: 'geoforge-region', mode, points: points.map((p) => p.slice(0, 2)), complete, active });
  function unlock() {
    if (savedInputs !== null) controller.enableInputs = savedInputs;
    savedInputs = null; drag = -1; anchor = null;
  }
  function pick(pixel) {
    let world;
    if (viewer.scene.pickPositionSupported) {
      const hit = viewer.scene.pick(pixel);
      if (hit && !entities.includes(hit.id)) world = viewer.scene.pickPosition(pixel);
    }
    world ||= viewer.camera.pickEllipsoid(pixel, viewer.scene.globe.ellipsoid);
    if (!world) return null;
    const p = Cesium.Cartographic.fromCartesian(world);
    if (!p) return null;
    return [Cesium.Math.toDegrees(p.longitude), Cesium.Math.toDegrees(p.latitude), p.height];
  }
  function render() {
    entities.forEach((e) => viewer.entities.remove(e)); entities = [];
    const ring = mode === 'rectangle' && points.length === 2 ? [points[0], [points[1][0], points[0][1], points[0][2]], points[1], [points[0][0], points[1][1], points[1][2]]] : points;
    const positions = ring.map((p) => Cesium.Cartesian3.fromDegrees(p[0], p[1], p[2] + 0.1));
    if (positions.length > 1) entities.push(viewer.entities.add({
      polyline: { positions: [...positions, ...(complete || mode === 'rectangle' ? [positions[0]] : [])], width: 3, material: Cesium.Color.YELLOW, depthFailMaterial: Cesium.Color.YELLOW },
    }));
    if (positions.length >= 3) entities.push(viewer.entities.add({
      polygon: { hierarchy: positions, perPositionHeight: true, material: Cesium.Color.YELLOW.withAlpha(0.18) },
    }));
    points.forEach((p, i) => entities.push(viewer.entities.add({
      properties: { clipVertex: i }, position: Cesium.Cartesian3.fromDegrees(...p),
      point: { pixelSize: 12, color: Cesium.Color.YELLOW, outlineColor: Cesium.Color.BLACK, outlineWidth: 2, disableDepthTestDistance: Infinity },
    })));
    viewer.scene.requestRender(); send();
  }
  handler.setInputAction((event) => {
    if (!active || mode !== 'polygon' || complete || drag >= 0) return;
    const p = pick(event.position);
    if (p && points.length < 256) { points.push(p); render(); }
  }, Cesium.ScreenSpaceEventType.LEFT_CLICK);
  handler.setInputAction((event) => {
    if (!active) return;
    if (complete) {
      const hit = viewer.scene.pick(event.position);
      const index = hit?.id?.properties?.clipVertex?.getValue();
      if (Number.isInteger(index) && entities.includes(hit.id)) drag = index;
      // The filled region can cover a control point in the depth picking buffer.
      // Match the nearest visible handle in screen space as well.
      if (drag < 0) {
        let nearest = 10;
        points.forEach((p, i) => {
          const pixel = Cesium.SceneTransforms.worldToWindowCoordinates(viewer.scene, Cesium.Cartesian3.fromDegrees(...p));
          if (!pixel) return;
          const distance = Cesium.Cartesian2.distance(pixel, event.position);
          if (distance <= nearest) { nearest = distance; drag = i; }
        });
      }
    } else if (mode === 'rectangle') {
      const p = pick(event.position);
      if (!p) return;
      points = [p, p.slice()]; anchor = event.position; drag = 1;
    }
    if (drag >= 0) { savedInputs = controller.enableInputs; controller.enableInputs = false; }
  }, Cesium.ScreenSpaceEventType.LEFT_DOWN);
  handler.setInputAction((event) => {
    if (drag < 0) return;
    const p = pick(event.endPosition);
    if (p) { points[drag] = p; render(); }
  }, Cesium.ScreenSpaceEventType.MOUSE_MOVE);
  handler.setInputAction((event) => {
    if (drag < 0) return;
    const p = pick(event.position);
    if (p) points[drag] = p;
    if (mode === 'rectangle') complete = !anchor || Cesium.Cartesian2.distance(anchor, event.position) > 3;
    unlock(); render();
  }, Cesium.ScreenSpaceEventType.LEFT_UP);
  handler.setInputAction(() => finish(), Cesium.ScreenSpaceEventType.RIGHT_CLICK);
  function finish() { if (active && points.length >= (mode === 'rectangle' ? 2 : 3)) { complete = true; unlock(); render(); } }
  function command(data) {
    if (data.type !== 'geoforge-draw') return;
    if (data.action === 'stop') { active = false; unlock(); send(); return; }
    if (data.action === 'clear') { unlock(); points = []; complete = false; render(); return; }
    if (data.action === 'finish') { finish(); return; }
    if (data.action === 'undo') { unlock(); complete = false; points.pop(); render(); return; }
    if (data.action === 'start' && ['rectangle', 'polygon'].includes(data.mode) && canDraw()) {
      unlock(); mode = data.mode; active = true; complete = false; points = []; render();
    }
  }
  const blur = () => { unlock(); };
  window.addEventListener('blur', blur);
  return { command, destroy() { unlock(); handler.destroy(); entities.forEach((e) => viewer.entities.remove(e)); window.removeEventListener('blur', blur); } };
}
