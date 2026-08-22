<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { ArcRotateCamera } from "@babylonjs/core/Cameras/arcRotateCamera";
  import { ArcRotateCameraPointersInput } from "@babylonjs/core/Cameras/Inputs/arcRotateCameraPointersInput";
  import { Color3 } from "@babylonjs/core/Maths/math.color";
  import { Color4 } from "@babylonjs/core/Maths/math.color";
  // oxlint-disable-next-line import/no-unassigned-import -- Babylon registers scene picking through this module.
  import "@babylonjs/core/Culling/ray";
  import { Engine } from "@babylonjs/core/Engines/engine";
  import { HemisphericLight } from "@babylonjs/core/Lights/hemisphericLight";
  import { DirectionalLight } from "@babylonjs/core/Lights/directionalLight";
  import { Mesh } from "@babylonjs/core/Meshes/mesh";
  import { AbstractMesh } from "@babylonjs/core/Meshes/abstractMesh";
  import { VertexData } from "@babylonjs/core/Meshes/mesh.vertexData";
  import { Scene } from "@babylonjs/core/scene";
  import { CreateScreenshotUsingRenderTarget } from "@babylonjs/core/Misc/screenshotTools";
  import { StandardMaterial } from "@babylonjs/core/Materials/standardMaterial";
  import { Material } from "@babylonjs/core/Materials/material";
  import { Vector3 } from "@babylonjs/core/Maths/math.vector";
  import type { Occurrence, RenderObjectOccurrences, ViewCapture } from "@api/gds";
  import { t } from "@i18n";
  import defaultTheme from "../themes/default";

  interface Props {
    objects: unknown[];
    occurrences?: RenderObjectOccurrences[];
    objectIds: string;
    themeMode: "light" | "dark";
    lightingIntensity: number;
    resetKey: number;
    resizePaused?: boolean;
    onPick?: (pick: ViewportPick | null) => void;
    onMeshesReady?: (resetKey: number, objectIds: string) => void;
    onMeshesError?: (resetKey: number, reason: unknown) => void;
    onCaptureReady?: (
      capture: ((width: number, height: number) => Promise<ViewCapture>) | null,
    ) => void;
    onModelExportReady?: (exporter: ((format: "glb" | "stl") => Promise<string>) | null) => void;
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
  type ViewportPick = {
    objectId: string;
    occurrence: Occurrence | null;
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
    triangleRanges: {
      startFaceId: number;
      endFaceId: number;
      polygonIndex: number;
    }[];
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
  type CameraView = "top" | "front" | "left" | "right" | "back" | "bottom";
  const homeAlpha = -Math.PI / 2;
  const homeBeta = Math.PI / 3;
  const cameraPoleMargin = 0.01;
  const cameraResetDuration = 1_500;
  let {
    objects,
    occurrences = [],
    objectIds,
    themeMode,
    lightingIntensity,
    resetKey,
    resizePaused = false,
    onPick,
    onMeshesReady,
    onMeshesError,
    onCaptureReady,
    onModelExportReady,
  }: Props = $props();
  let canvas = $state<HTMLCanvasElement>();
  let scene: Scene | null = null;
  let camera: ArcRotateCamera | null = null;
  let meshes: Mesh[] = [];
  let renderedLayers = new Map<string, RenderedLayer>();
  let renderedObjectIds = "";
  let renderedResetKey = -1;
  let selectionPointerStart: { x: number; y: number } | null = null;
  let panDragging = $state(false);
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

  function resetPointerState() {
    panDragging = false;
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
        keyLight.diffuse = new Color3(1, 1, 1);
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
      keyLight.diffuse = new Color3(1, 1, 1);
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
    // Worker meshes extend down from local Y=0, so their top belongs at zMax.
    rendered.mesh.position.y = zMax;
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

  function reportPickedMesh(mesh: AbstractMesh | null, faceId: number | undefined) {
    const metadata = mesh?.metadata;
    const id = metadata?.objectId as string | undefined;
    if (!id) return;

    const triangleRange =
      typeof faceId === "number"
        ? (
            metadata?.triangleRanges as
              | { startFaceId: number; endFaceId: number; polygonIndex: number }[]
              | undefined
          )?.find((range) => faceId >= range.startFaceId && faceId < range.endFaceId)
        : undefined;
    const occurrence = triangleRange
      ? ((metadata?.occurrences as Occurrence[] | undefined)?.[triangleRange.polygonIndex] ?? null)
      : null;
    onPick?.({ objectId: id, occurrence });
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
      worker.addEventListener("message", ({ data }: MessageEvent<MeshWorkerResponse>) => {
        if (data.ok) finish(data.layers);
        else finish(null, new Error(data.message));
      });
      worker.addEventListener("error", (event) =>
        finish(null, new Error(event.message || "mesh worker failed")),
      );
      // oxlint-disable-next-line unicorn/require-post-message-target-origin -- Worker.postMessage has no targetOrigin parameter.
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
    const occurrencesByObjectId = new Map(
      occurrences.map((entry) => [entry.objectId, entry.occurrences] as const),
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
      mesh.isPickable = true;
      mesh.alwaysSelectAsActiveMesh = true;
      mesh.metadata = {
        objectId: layer.id,
        occurrences: occurrencesByObjectId.get(layer.id) ?? [],
        triangleRanges: layer.triangleRanges,
      };
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
        // oxlint-disable-next-line eslint/no-await-in-loop -- Yielding between GPU uploads keeps the UI responsive.
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

  function animateCameraTo(endState: CameraState, preferPositiveHalfTurn = false) {
    if (!camera) return;
    stopCameraAnimation();
    resetCameraInertia();
    const activeCamera = camera;
    const startTarget = activeCamera.target.clone();
    const startAlpha = activeCamera.alpha;
    const startBeta = activeCamera.beta;
    const startRadius = activeCamera.radius;
    const shortestAlphaDelta = shortestAngleDelta(startAlpha, endState.alpha);
    const alphaDelta =
      preferPositiveHalfTurn && Math.abs(Math.abs(shortestAlphaDelta) - Math.PI) < 0.0001
        ? Math.PI
        : shortestAlphaDelta;
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

  function cameraViewState(view: CameraView): CameraState {
    const direction = {
      top: { alpha: homeAlpha, beta: cameraPoleMargin },
      front: { alpha: -Math.PI / 2, beta: Math.PI / 2 },
      left: { alpha: Math.PI, beta: Math.PI / 2 },
      right: { alpha: 0, beta: Math.PI / 2 },
      back: { alpha: Math.PI / 2, beta: Math.PI / 2 },
      bottom: { alpha: homeAlpha + Math.PI, beta: Math.PI - cameraPoleMargin },
    }[view];
    return {
      target: homeCameraState.target.clone(),
      alpha: direction.alpha,
      beta: direction.beta,
      radius: homeCameraState.radius,
    };
  }

  function animateCameraHome(event: Event) {
    const view = (event as CustomEvent<CameraView | undefined>).detail;
    const endState = view
      ? cameraViewState(view)
      : {
          ...homeCameraState,
          target: homeCameraState.target.clone(),
        };
    animateCameraTo(endState, view === "bottom");
  }

  function blobDataUrl(blob: Blob): Promise<string> {
    return new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.addEventListener("load", () => {
        if (typeof reader.result === "string") resolve(reader.result);
        else reject(new Error(t("gds.exportEncodeFailed")));
      });
      reader.addEventListener("error", () =>
        reject(reader.error ?? new Error(t("gds.exportEncodeFailed"))),
      );
      reader.readAsDataURL(blob);
    });
  }

  function encodePng(width: number, height: number, pixels: ArrayBufferView): Promise<string> {
    const expectedLength = width * height * 4;
    if (pixels.byteLength !== expectedLength) {
      return Promise.reject(new Error(t("gds.exportEncodeFailed")));
    }
    const source = new Uint8ClampedArray(pixels.buffer, pixels.byteOffset, pixels.byteLength);
    const target = document.createElement("canvas");
    target.width = width;
    target.height = height;
    const context = target.getContext("2d");
    if (!context) return Promise.reject(new Error(t("gds.exportEncodeFailed")));
    const image = context.createImageData(width, height);
    const rowLength = width * 4;
    for (let targetRow = 0; targetRow < height; targetRow += 1) {
      const sourceStart = (height - targetRow - 1) * rowLength;
      image.data.set(source.subarray(sourceStart, sourceStart + rowLength), targetRow * rowLength);
    }
    context.putImageData(image, 0, 0);
    return new Promise((resolve, reject) => {
      target.toBlob((blob) => {
        if (!blob) {
          reject(new Error(t("gds.exportEncodeFailed")));
          return;
        }
        void blobDataUrl(blob).then(resolve, reject);
      }, "image/png");
    });
  }

  async function exportTimeout<T>(operation: Promise<T>): Promise<T> {
    let timeout: number | undefined;
    try {
      return await Promise.race([
        operation,
        new Promise<never>((_, reject) => {
          timeout = window.setTimeout(() => reject(new Error(t("gds.exportTimedOut"))), 35_000);
        }),
      ]);
    } finally {
      if (timeout !== undefined) window.clearTimeout(timeout);
    }
  }

  onMount(() => {
    if (!canvas) return;
    const viewportCanvas = canvas;
    const viewportEngine = new Engine(
      viewportCanvas,
      true,
      { preserveDrawingBuffer: false, stencil: true },
      true,
    );
    const activeScene = new Scene(viewportEngine);
    scene = activeScene;
    // Transparent meshes must retain the depth written by opaque meshes in group 0.
    activeScene.setRenderingAutoClearDepthStencil(1, false);
    updateEnvironment();
    const activeCamera = new ArcRotateCamera(
      "camera",
      -Math.PI / 2,
      Math.PI / 3,
      100,
      Vector3.Zero(),
      activeScene,
    );
    camera = activeCamera;
    const renderFrame = () => {
      if (!resizePaused) activeScene.render();
    };
    onCaptureReady?.(async (width, height) => {
      const maximumSize = viewportEngine.getCaps().maxTextureSize;
      if (width > maximumSize || height > maximumSize) {
        throw new Error(t("gds.exportTextureLimit", { width, height, maximum: maximumSize }));
      }
      try {
        const dataUrl = await exportTimeout(
          new Promise<string>((resolve, reject) => {
            CreateScreenshotUsingRenderTarget(
              viewportEngine,
              activeCamera,
              { width, height },
              resolve,
              "image/png",
              1,
              false,
              undefined,
              false,
              false,
              true,
              undefined,
              (texture) => {
                texture.readPixels = () => {
                  // WebKitGTK can leave Babylon's asynchronous pixel read pending indefinitely.
                  // eslint-disable-next-line no-underscore-dangle
                  const pixels = texture._readPixelsSync(0, 0, null, true, false);
                  return pixels
                    ? Promise.resolve(pixels)
                    : Promise.reject(new Error(t("gds.exportEncodeFailed")));
                };
              },
              (dumpWidth, dumpHeight, pixels, successCallback) => {
                const encode = async () => {
                  try {
                    const result = await encodePng(dumpWidth, dumpHeight, pixels);
                    successCallback?.(result);
                  } catch (reason) {
                    reject(reason);
                  }
                };
                void encode();
              },
              30_000,
              () => reject(new Error(t("gds.exportTimedOut"))),
            );
          }),
        );
        return { dataUrl, width, height };
      } catch (reason) {
        if (
          reason instanceof Error &&
          (reason.message === t("gds.exportTimedOut") ||
            reason.message === t("gds.exportEncodeFailed"))
        ) {
          throw reason;
        }
        throw new Error(t("gds.exportRetry"), { cause: reason });
      }
    });
    onModelExportReady?.(async (format) => {
      const exportMeshes = meshes.filter((mesh) => mesh.isEnabled() && mesh.isVisible);
      if (format === "glb") {
        const { GLTF2Export } = await import("@babylonjs/serializers/glTF");
        const included = new Set(exportMeshes);
        const data = await GLTF2Export.GLBAsync(scene!, "gds3d-model", {
          shouldExportNode: (node) => node instanceof Mesh && included.has(node),
        });
        const file = data.files["gds3d-model.glb"];
        if (!(file instanceof Blob)) throw new Error(t("gds.exportEncodeFailed"));
        return blobDataUrl(file);
      }
      const { STLExport } = await import("@babylonjs/serializers/stl");
      const clones = exportMeshes.map((mesh) => {
        const clone = mesh.clone(`${mesh.name}-stl-export`, null, true)!;
        clone.makeGeometryUnique();
        clone.bakeCurrentTransformIntoVertices();
        return clone;
      });
      try {
        const data = STLExport.CreateSTL(clones, false, "gds3d-model", true, true, true);
        return blobDataUrl(new Blob([data], { type: "model/stl" }));
      } finally {
        for (const clone of clones) clone.dispose(false, false);
      }
    });
    activeCamera.inputs.removeByType("ArcRotateCameraPointersInput");
    const pointerInput = new ArcRotateCameraPointersInput();
    pointerInput.buttons = [1, 2];
    activeCamera.inputs.add(pointerInput);
    activeCamera.attachControl(false, false, -1);
    activeCamera.movement.input.setInteraction("pointer", { button: 1 }, "pan");
    activeCamera.movement.input.setInteraction("pointer", { button: 2 }, "rotate");
    activeCamera.wheelDeltaPercentage = 0.01;
    activeCamera.panningSensibility = homePanningSensibility;
    activeCamera.lowerRadiusLimit = 0.01;
    activeCamera.upperRadiusLimit = Number.MAX_SAFE_INTEGER;
    viewportCanvas.addEventListener("contextmenu", preventContextMenu);
    viewportCanvas.addEventListener("wheel", preventPageZoom, { passive: false });
    viewportCanvas.addEventListener("pointerdown", stopCameraAnimation);
    const startSelection = (event: PointerEvent) => {
      if (event.button === 1) panDragging = true;
      if (event.button !== 0) return;
      selectionPointerStart = { x: event.clientX, y: event.clientY };
    };
    const finishSelection = (event: PointerEvent) => {
      if (event.button === 1) panDragging = false;
      if (event.button !== 0 || !selectionPointerStart) return;
      const movement = Math.hypot(
        event.clientX - selectionPointerStart.x,
        event.clientY - selectionPointerStart.y,
      );
      selectionPointerStart = null;
      if (movement > 4) return;

      const bounds = viewportCanvas.getBoundingClientRect();
      const pick = activeScene.pick(
        event.clientX - bounds.left,
        event.clientY - bounds.top,
        (mesh) => Boolean(mesh.metadata?.objectId),
      );
      if (pick.hit) reportPickedMesh(pick.pickedMesh ?? null, pick.faceId);
    };
    viewportCanvas.addEventListener("pointerdown", startSelection);
    viewportCanvas.addEventListener("pointerup", finishSelection);
    viewportCanvas.addEventListener("pointercancel", finishSelection);
    window.addEventListener("blur", resetPointerState);
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
    renderedResetKey = resetKey;
    void synchronizeObjects(objects, true);
    viewportEngine.runRenderLoop(renderFrame);
    let resizeFrame: number | undefined;
    const resize = () => {
      if (resizePaused) return;
      if (resizeFrame !== undefined) cancelAnimationFrame(resizeFrame);
      resizeFrame = requestAnimationFrame(() => {
        resizeFrame = undefined;
        if (resizePaused) return;
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
      viewportCanvas.removeEventListener("pointerdown", startSelection);
      viewportCanvas.removeEventListener("pointerup", finishSelection);
      viewportCanvas.removeEventListener("pointercancel", finishSelection);
      window.removeEventListener("blur", resetPointerState);
      resizeObserver.disconnect();
      scene?.onBeforeRenderObservable.remove(panningObserver);
      if (resizeFrame !== undefined) cancelAnimationFrame(resizeFrame);
      stopCameraAnimation();
      meshBuildGeneration += 1;
      cancelActiveMeshBuild();
      requestEngineResize = null;
      onCaptureReady?.(null);
      onModelExportReady?.(null);
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
  <canvas class:pan-dragging={panDragging} bind:this={canvas} aria-label={t("gds.viewportLabel")}
  ></canvas>
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
    outline: none;
    touch-action: none;
    cursor: default;
  }
  canvas.pan-dragging {
    cursor: grabbing;
  }
</style>
