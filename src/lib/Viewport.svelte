<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { ArcRotateCamera } from "@babylonjs/core/Cameras/arcRotateCamera";
  import { Color3 } from "@babylonjs/core/Maths/math.color";
  import { Color4 } from "@babylonjs/core/Maths/math.color";
  import { Engine } from "@babylonjs/core/Engines/engine";
  import { HemisphericLight } from "@babylonjs/core/Lights/hemisphericLight";
  import { DirectionalLight } from "@babylonjs/core/Lights/directionalLight";
  import { Mesh } from "@babylonjs/core/Meshes/mesh";
  import { VertexData } from "@babylonjs/core/Meshes/mesh.vertexData";
  import { Scene } from "@babylonjs/core/scene";
  import { StandardMaterial } from "@babylonjs/core/Materials/standardMaterial";
  import { Material } from "@babylonjs/core/Materials/material";
  import { Vector3 } from "@babylonjs/core/Maths/math.vector";
  import { PointerEventTypes } from "@babylonjs/core/Events/pointerEvents";
  import { t } from "@i18n";
  import defaultTheme from "../themes/default";

  interface Props {
    objects: unknown[];
    objectIds: string;
    themeMode: "light" | "dark";
    lightingIntensity: number;
    resetKey: number;
    resizePaused?: boolean;
    onSelect?: (id: string | null) => void;
    onMeshesReady?: (resetKey: number, objectIds: string) => void;
    onMeshesError?: (resetKey: number, reason: unknown) => void;
  }
  type ViewportRecord = {
    kind?: string;
    payload?: {
      id?: string;
      display?: {
        color?: string;
        opacity?: number;
        visible?: boolean;
        z_min?: number;
        z_max?: number;
      };
      polygons?: { points: number[][]; holes?: number[][][] }[];
      bounds?: { min_x: number; min_y: number; max_x: number; max_y: number };
    };
  };
  type RenderedLayer = {
    mesh: Mesh;
    material: StandardMaterial;
    baseDepth: number;
    appearance: LayerAppearance;
  };
  type LayerAppearance = {
    color: string;
    opacity: number;
    visible: boolean;
    zMin: number;
    zMax: number;
  };
  type ViewportDisplayEvent = {
    objectId: string;
    update: {
      color?: string;
      opacity?: number;
      visible?: boolean;
      z_min?: number;
      z_max?: number;
    };
  };
  type WorkerLayerInput = {
    id: string;
    depth: number;
    polygons: { points: number[][]; holes?: number[][][] }[];
  };
  type WorkerLayerMesh = {
    id: string;
    positions: Float32Array;
    normals: Float32Array;
    indices: Uint32Array;
  };
  type MeshWorkerResponse =
    | { ok: true; layers: WorkerLayerMesh[] }
    | { ok: false; message: string };
  type CameraState = {
    target: Vector3;
    alpha: number;
    beta: number;
    radius: number;
  };
  const homeAlpha = -Math.PI / 2;
  const homeBeta = Math.PI / 3;
  const cameraResetDuration = 1_500;
  let {
    objects,
    objectIds,
    themeMode,
    lightingIntensity,
    resetKey,
    resizePaused = false,
    onSelect,
    onMeshesReady,
    onMeshesError,
  }: Props = $props();
  let canvas = $state<HTMLCanvasElement>();
  let scene: Scene | null = null;
  let camera: ArcRotateCamera | null = null;
  let meshes: Mesh[] = [];
  let renderedLayers = new Map<string, RenderedLayer>();
  let renderedObjectIds = "";
  let renderedResetKey = -1;
  let homeCameraState: CameraState = {
    target: Vector3.Zero(),
    alpha: homeAlpha,
    beta: homeBeta,
    radius: 100,
  };
  const homePanningSensibility = 3;
  let ambientLight: HemisphericLight | null = null;
  let keyLight: DirectionalLight | null = null;
  let requestEngineResize: (() => void) | null = null;
  let cameraAnimationFrame: number | undefined;
  let activeMeshWorker: Worker | null = null;
  let cancelMeshBuild: (() => void) | null = null;
  let meshBuildGeneration = 0;

  function preventContextMenu(event: MouseEvent) {
    event.preventDefault();
  }

  function preventPageZoom(event: WheelEvent) {
    event.preventDefault();
    stopCameraAnimation();
  }

  function stopCameraAnimation() {
    if (cameraAnimationFrame === undefined) return;
    cancelAnimationFrame(cameraAnimationFrame);
    cameraAnimationFrame = undefined;
  }

  function clearMeshes() {
    for (const { mesh } of renderedLayers.values()) mesh.dispose(false, true);
    meshes = [];
    renderedLayers.clear();
  }

  function configureLighting() {
    ambientLight?.dispose();
    keyLight?.dispose();
    ambientLight = null;
    keyLight = null;
    if (!scene) return;

    ambientLight = new HemisphericLight("ambient-light", new Vector3(0, 1, 0), scene);

    keyLight = new DirectionalLight("camera-key-light", new Vector3(0, -1, 0), scene);
    updateEnvironment();
  }

  function updateEnvironment() {
    if (!scene) return;
    const background = Color3.FromHexString(defaultTheme[themeMode].bgSecondary);
    scene.clearColor = new Color4(background.r, background.g, background.b, 1);
    if (themeMode === "dark") {
      if (ambientLight) {
        ambientLight.intensity = 0.55 * lightingIntensity;
        ambientLight.diffuse = new Color3(0.82, 0.88, 1);
        ambientLight.groundColor = new Color3(0.08, 0.1, 0.14);
      }
      if (keyLight) {
        keyLight.intensity = 0.65 * lightingIntensity;
        keyLight.diffuse = new Color3(1, 0.95, 0.86);
      }
      return;
    }

    if (ambientLight) {
      ambientLight.intensity = 0.65 * lightingIntensity;
      ambientLight.diffuse = new Color3(0.92, 0.95, 1);
      ambientLight.groundColor = new Color3(0.24, 0.27, 0.32);
    }
    if (keyLight) {
      keyLight.intensity = 0.75 * lightingIntensity;
      keyLight.diffuse = new Color3(1, 0.97, 0.9);
    }
  }

  function appearanceFromRecord(record: ViewportRecord): LayerAppearance {
    const display = record.payload?.display;
    return {
      color: display?.color ?? "#4c89c8",
      opacity: display?.opacity ?? 1,
      visible: display?.visible ?? true,
      zMin: display?.z_min ?? 0,
      zMax: display?.z_max ?? 1,
    };
  }

  function applyLayerAppearance(rendered: RenderedLayer) {
    const { color: colorValue, opacity, visible, zMin, zMax } = rendered.appearance;
    const color = Color3.FromHexString(colorValue);
    rendered.material.diffuseColor = color;
    rendered.material.emissiveColor = Color3.Black();
    rendered.material.alpha = Math.min(Math.max(opacity, 0), 1);
    rendered.material.transparencyMode =
      opacity < 1 ? Material.MATERIAL_ALPHABLEND : Material.MATERIAL_OPAQUE;
    rendered.material.needDepthPrePass = opacity < 1;
    rendered.mesh.renderingGroupId = opacity < 1 ? 1 : 0;
    rendered.mesh.setEnabled(visible);
    const depth = Math.max(0.001, zMax - zMin);
    rendered.mesh.scaling.y = depth / rendered.baseDepth;
    rendered.mesh.position.y = zMin;
  }

  function updateLayerAppearance(record: ViewportRecord, rendered: RenderedLayer) {
    rendered.appearance = appearanceFromRecord(record);
    applyLayerAppearance(rendered);
  }

  function updateLayerAppearanceFromEvent(event: Event) {
    const detail = (event as CustomEvent<ViewportDisplayEvent>).detail;
    const rendered = renderedLayers.get(detail.objectId);
    if (!rendered) return;
    const { update } = detail;
    rendered.appearance = {
      ...rendered.appearance,
      color: update.color ?? rendered.appearance.color,
      opacity: update.opacity ?? rendered.appearance.opacity,
      visible: update.visible ?? rendered.appearance.visible,
      zMin: update.z_min ?? rendered.appearance.zMin,
      zMax: update.z_max ?? rendered.appearance.zMax,
    };
    applyLayerAppearance(rendered);
  }

  function cancelActiveMeshBuild() {
    cancelMeshBuild?.();
    cancelMeshBuild = null;
    activeMeshWorker = null;
  }

  function buildMeshesInWorker(layers: WorkerLayerInput[]): Promise<WorkerLayerMesh[] | null> {
    cancelActiveMeshBuild();
    const worker = new Worker(new URL("./mesh.worker.ts", import.meta.url), { type: "module" });
    activeMeshWorker = worker;
    return new Promise((resolve, reject) => {
      let settled = false;
      const finish = (result: WorkerLayerMesh[] | null, reason?: unknown) => {
        if (settled) return;
        settled = true;
        worker.terminate();
        if (activeMeshWorker === worker) activeMeshWorker = null;
        if (cancelMeshBuild === cancel) cancelMeshBuild = null;
        if (reason !== undefined) reject(reason);
        else resolve(result);
      };
      const cancel = () => finish(null);
      cancelMeshBuild = cancel;
      worker.onmessage = ({ data }: MessageEvent<MeshWorkerResponse>) => {
        if (data.ok) finish(data.layers);
        else finish(null, new Error(data.message));
      };
      worker.onerror = (event) => finish(null, new Error(event.message || "mesh worker failed"));
      worker.postMessage({ layers });
    });
  }

  function createLayerMesh(data: WorkerLayerMesh) {
    if (!scene || data.positions.length === 0 || data.indices.length === 0) return null;
    const mesh = new Mesh(data.id, scene);
    const vertexData = new VertexData();
    vertexData.positions = data.positions;
    vertexData.normals = data.normals;
    vertexData.indices = data.indices;
    vertexData.applyToMesh(mesh, false);
    return mesh;
  }

  function clonePolygonsForWorker(
    polygons: { points: number[][]; holes?: number[][][] }[],
  ): { points: number[][]; holes?: number[][][] }[] {
    return polygons.map((polygon) => ({
      points: polygon.points.map((point) => [point[0], point[1]]),
      holes: polygon.holes?.map((hole) => hole.map((point) => [point[0], point[1]])),
    }));
  }

  async function synchronizeObjects(sceneObjects: unknown[], forceRebuild = false) {
    if (!scene) return;
    const records = sceneObjects
      .map((entry) => entry as ViewportRecord)
      .filter(
        (record) =>
          (record.kind === "GdsLayer" || record.kind === "Baseplate") && record.payload?.id,
      );
    const renderedIds = records.map((record) => record.payload?.id ?? "").join("|");
    const geometryChanged = forceRebuild || renderedIds !== renderedObjectIds;
    if (!geometryChanged) return;

    const generation = ++meshBuildGeneration;
    const buildResetKey = resetKey;
    cancelActiveMeshBuild();
    renderedObjectIds = renderedIds;
    if (forceRebuild) clearMeshes();
    const currentIds = new Set(records.map((record) => record.payload?.id ?? ""));
    for (const [id, rendered] of renderedLayers) {
      if (currentIds.has(id)) continue;
      rendered.mesh.dispose(false, true);
      renderedLayers.delete(id);
      meshes = meshes.filter((mesh) => mesh !== rendered.mesh);
    }

    const pendingRecords = records.filter(
      (record) => record.payload?.id && !renderedLayers.has(record.payload.id),
    );
    const workerLayers = pendingRecords.map((record) => {
      const payload = record.payload;
      const id = payload?.id ?? "";
      const polygons =
        payload?.polygons ??
        (payload?.bounds
          ? [
              {
                points: [
                  [payload.bounds.min_x, payload.bounds.min_y],
                  [payload.bounds.max_x, payload.bounds.min_y],
                  [payload.bounds.max_x, payload.bounds.max_y],
                  [payload.bounds.min_x, payload.bounds.max_y],
                ],
              },
            ]
          : []);
      const depth = Math.max(
        0.001,
        (payload?.display?.z_max ?? 1) - (payload?.display?.z_min ?? 0),
      );
      return { id, polygons: clonePolygonsForWorker(polygons), depth };
    });

    if (workerLayers.length === 0) {
      if (forceRebuild) fitCamera();
      onMeshesReady?.(buildResetKey, renderedIds);
      return;
    }

    let builtLayers: WorkerLayerMesh[] | null;
    try {
      builtLayers = await buildMeshesInWorker(workerLayers);
    } catch (reason) {
      if (generation === meshBuildGeneration) onMeshesError?.(buildResetKey, reason);
      return;
    }
    if (!builtLayers || generation !== meshBuildGeneration || !scene) return;

    const recordsById = new Map(
      pendingRecords
        .filter((record) => record.payload?.id)
        .map((record) => [record.payload?.id ?? "", record] as const),
    );
    const depthsById = new Map(workerLayers.map(({ id, depth }) => [id, depth] as const));
    for (const [layerIndex, layer] of builtLayers.entries()) {
      if (generation !== meshBuildGeneration || !scene) return;
      const record = recordsById.get(layer.id);
      if (!record) continue;
      const material = new StandardMaterial(`${layer.id}-material`, scene);
      material.specularColor = new Color3(0.06, 0.06, 0.06);
      material.alpha = 1;
      material.backFaceCulling = false;
      const mesh = createLayerMesh(layer);
      if (!mesh) {
        material.dispose();
        continue;
      }
      mesh.material = material;
      mesh.metadata = { objectId: layer.id };
      meshes.push(mesh);
      const rendered = {
        mesh,
        material,
        baseDepth: depthsById.get(layer.id) ?? 1,
        appearance: appearanceFromRecord(record),
      };
      renderedLayers.set(layer.id, rendered);
      updateLayerAppearance(record, rendered);
      if (layerIndex + 1 < builtLayers.length) {
        await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
      }
    }
    if (forceRebuild) fitCamera();
    onMeshesReady?.(buildResetKey, renderedIds);
  }

  function fitCamera() {
    if (!camera || meshes.length === 0) return;
    for (const mesh of meshes) mesh.computeWorldMatrix(true);
    const { min, max } = Mesh.MinMax(meshes);
    const target = min.add(max).scale(0.5);
    const radius = Math.max(max.subtract(min).length() * 1.25, 1);
    homeCameraState = { target, alpha: homeAlpha, beta: homeBeta, radius };
    resetCameraImmediately();
    camera.getViewMatrix(true);
    homeCameraState = {
      target: camera.target.clone(),
      alpha: camera.alpha,
      beta: camera.beta,
      radius: camera.radius,
    };
  }

  function resetCameraInertia() {
    if (!camera) return;
    camera.movement.resetPanVelocity();
    camera.movement.resetRotationVelocity();
    camera.movement.resetZoomVelocity();
    camera.inertialAlphaOffset = 0;
    camera.inertialBetaOffset = 0;
    camera.inertialRadiusOffset = 0;
    camera.inertialPanningX = 0;
    camera.inertialPanningY = 0;
  }

  function resetCameraImmediately() {
    if (!camera) return;
    stopCameraAnimation();
    resetCameraInertia();
    camera.setTarget(homeCameraState.target, false, true, true);
    camera.alpha = homeCameraState.alpha;
    camera.beta = homeCameraState.beta;
    camera.radius = homeCameraState.radius;
  }

  function shortestAngleDelta(start: number, end: number) {
    return ((((end - start + Math.PI) % (Math.PI * 2)) + Math.PI * 2) % (Math.PI * 2)) - Math.PI;
  }

  function hermite(
    progress: number,
    startTime: number,
    endTime: number,
    startValue: number,
    endValue: number,
    startSlope: number,
    endSlope: number,
  ) {
    const duration = endTime - startTime;
    const position = (progress - startTime) / duration;
    const position2 = position * position;
    const position3 = position2 * position;
    return (
      (2 * position3 - 3 * position2 + 1) * startValue +
      (position3 - 2 * position2 + position) * duration * startSlope +
      (-2 * position3 + 3 * position2) * endValue +
      (position3 - position2) * duration * endSlope
    );
  }

  function cameraResetProgress(progress: number) {
    const mirrored = progress > 0.5;
    const position = mirrored ? 1 - progress : progress;
    const eased =
      position < 1 / 3
        ? hermite(position, 0, 1 / 3, 0, 0.06, 0, 0.5)
        : hermite(position, 1 / 3, 0.5, 0.06, 0.5, 0.5, 4);
    return mirrored ? 1 - eased : eased;
  }

  function animateCameraHome() {
    if (!camera) return;
    stopCameraAnimation();
    resetCameraInertia();
    const activeCamera = camera;
    const startTarget = activeCamera.target.clone();
    const endState: CameraState = {
      ...homeCameraState,
      target: homeCameraState.target.clone(),
    };
    const startAlpha = activeCamera.alpha;
    const startBeta = activeCamera.beta;
    const startRadius = activeCamera.radius;
    const alphaDelta = shortestAngleDelta(startAlpha, endState.alpha);
    const startedAt = performance.now();

    const animate = (now: number) => {
      if (camera !== activeCamera) {
        cameraAnimationFrame = undefined;
        return;
      }
      const progress = Math.min((now - startedAt) / cameraResetDuration, 1);
      const eased = cameraResetProgress(progress);
      activeCamera.setTarget(Vector3.Lerp(startTarget, endState.target, eased), false, true, true);
      activeCamera.alpha = startAlpha + alphaDelta * eased;
      activeCamera.beta = startBeta + (endState.beta - startBeta) * eased;
      activeCamera.radius = startRadius + (endState.radius - startRadius) * eased;
      if (progress < 1) {
        cameraAnimationFrame = requestAnimationFrame(animate);
        return;
      }
      cameraAnimationFrame = undefined;
      resetCameraInertia();
      activeCamera.setTarget(endState.target, false, true, true);
      activeCamera.alpha = endState.alpha;
      activeCamera.beta = endState.beta;
      activeCamera.radius = endState.radius;
    };

    cameraAnimationFrame = requestAnimationFrame(animate);
  }

  onMount(() => {
    if (!canvas) return;
    const viewportCanvas = canvas;
    const viewportEngine = new Engine(
      viewportCanvas,
      true,
      { preserveDrawingBuffer: true, stencil: true },
      true,
    );
    scene = new Scene(viewportEngine);
    updateEnvironment();
    const activeCamera = new ArcRotateCamera(
      "camera",
      -Math.PI / 2,
      Math.PI / 3,
      100,
      Vector3.Zero(),
      scene,
    );
    camera = activeCamera;
    activeCamera.attachControl(false, false, 2);
    activeCamera.wheelDeltaPercentage = 0.01;
    activeCamera.panningSensibility = homePanningSensibility;
    activeCamera.lowerRadiusLimit = 0.01;
    activeCamera.upperRadiusLimit = Number.MAX_SAFE_INTEGER;
    viewportCanvas.addEventListener("contextmenu", preventContextMenu);
    viewportCanvas.addEventListener("wheel", preventPageZoom, { passive: false });
    viewportCanvas.addEventListener("pointerdown", stopCameraAnimation);
    window.addEventListener("gds3d-reset-camera", animateCameraHome);
    window.addEventListener("gds3d-viewport-display", updateLayerAppearanceFromEvent);
    configureLighting();
    const panningObserver = scene.onBeforeRenderObservable.add(() => {
      const radiusRatio = Math.max(activeCamera.radius / homeCameraState.radius, 0.0001);
      activeCamera.panningSensibility = homePanningSensibility / radiusRatio;
      activeCamera.minZ = Math.max(0.01, activeCamera.radius / 1_000);
      activeCamera.maxZ = Math.max(10_000, activeCamera.radius * 4);
      if (keyLight) {
        keyLight.direction
          .copyFrom(activeCamera.target)
          .subtractInPlace(activeCamera.position)
          .normalize();
      }
    });
    scene.onPointerObservable.add((event) => {
      if (event.type !== PointerEventTypes.POINTERPICK) return;
      const id = event.pickInfo?.hit
        ? (event.pickInfo.pickedMesh?.metadata?.objectId as string | undefined)
        : undefined;
      onSelect?.(id ?? null);
    });
    renderedResetKey = resetKey;
    void synchronizeObjects(objects, true);
    viewportEngine.runRenderLoop(() => {
      if (!resizePaused) scene?.render();
    });
    let resizeFrame: number | undefined;
    const resize = () => {
      if (resizePaused) return;
      if (resizeFrame !== undefined) cancelAnimationFrame(resizeFrame);
      resizeFrame = requestAnimationFrame(() => {
        resizeFrame = undefined;
        viewportEngine.resize();
      });
    };
    requestEngineResize = resize;
    const resizeObserver = new ResizeObserver(resize);
    resizeObserver.observe(viewportCanvas);
    window.addEventListener("resize", resize);
    resize();
    return () => {
      window.removeEventListener("resize", resize);
      window.removeEventListener("gds3d-reset-camera", animateCameraHome);
      window.removeEventListener("gds3d-viewport-display", updateLayerAppearanceFromEvent);
      viewportCanvas.removeEventListener("contextmenu", preventContextMenu);
      viewportCanvas.removeEventListener("wheel", preventPageZoom);
      viewportCanvas.removeEventListener("pointerdown", stopCameraAnimation);
      resizeObserver.disconnect();
      scene?.onBeforeRenderObservable.remove(panningObserver);
      if (resizeFrame !== undefined) cancelAnimationFrame(resizeFrame);
      stopCameraAnimation();
      meshBuildGeneration += 1;
      cancelActiveMeshBuild();
      requestEngineResize = null;
      clearMeshes();
      ambientLight?.dispose();
      keyLight?.dispose();
      ambientLight = null;
      keyLight = null;
      scene?.dispose();
      viewportEngine.dispose();
      scene = null;
      camera = null;
    };
  });

  $effect(() => {
    void objectIds;
    const forceFit = resetKey !== renderedResetKey;
    renderedResetKey = resetKey;
    void synchronizeObjects(
      untrack(() => objects),
      forceFit,
    );
  });

  $effect(() => {
    void themeMode;
    void lightingIntensity;
    updateEnvironment();
  });

  $effect(() => {
    if (!resizePaused) requestEngineResize?.();
  });
</script>

<div class="viewport-frame">
  <canvas bind:this={canvas} aria-label={t("gds.viewportLabel")}></canvas>
</div>

<style>
  .viewport-frame {
    position: relative;
    width: 100%;
    height: 100%;
    overflow: hidden;
  }
  canvas {
    display: block;
    inline-size: 100%;
    block-size: 100%;
    touch-action: none;
    cursor: grab;
  }
  canvas:active {
    cursor: grabbing;
  }
</style>
