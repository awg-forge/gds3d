<script lang="ts">
  import { onMount } from "svelte";
  import { ArcRotateCamera } from "@babylonjs/core/Cameras/arcRotateCamera";
  import { Color3 } from "@babylonjs/core/Maths/math.color";
  import { Color4 } from "@babylonjs/core/Maths/math.color";
  import { Engine } from "@babylonjs/core/Engines/engine";
  import { HemisphericLight } from "@babylonjs/core/Lights/hemisphericLight";
  import { Mesh } from "@babylonjs/core/Meshes/mesh";
  import { MeshBuilder } from "@babylonjs/core/Meshes/meshBuilder";
  import { Scene } from "@babylonjs/core/scene";
  import { StandardMaterial } from "@babylonjs/core/Materials/standardMaterial";
  import { Vector3 } from "@babylonjs/core/Maths/math.vector";
  import { PointerEventTypes } from "@babylonjs/core/Events/pointerEvents";
  import earcut from "earcut";

  interface Props {
    objects: unknown[];
    onSelect?: (id: string | null) => void;
  }
  let { objects, onSelect }: Props = $props();
  let canvas = $state<HTMLCanvasElement>();
  let scene: Scene | null = null;
  let camera: ArcRotateCamera | null = null;
  let meshes: Mesh[] = [];
  let renderedObjectIds = "";
  let homeTarget = Vector3.Zero();
  let homeRadius = 100;

  function preventContextMenu(event: MouseEvent) {
    event.preventDefault();
  }

  function preventPageZoom(event: WheelEvent) {
    event.preventDefault();
  }

  function clearMeshes() {
    for (const mesh of meshes) mesh.dispose();
    meshes = [];
  }

  function renderObjects(sceneObjects: unknown[]) {
    if (!scene) return;
    const objectIds = sceneObjects
      .map((entry) => (entry as { payload?: { id?: string } }).payload?.id ?? "")
      .join("|");
    const shouldFitCamera = objectIds !== renderedObjectIds;
    renderedObjectIds = objectIds;
    clearMeshes();
    for (const entry of sceneObjects) {
      const record = entry as {
        kind?: string;
        payload?: {
          id?: string;
          display?: {
            color?: string;
            brightness?: number;
            visible?: boolean;
            z_min?: number;
            z_max?: number;
          };
          polygons?: { points: number[][] }[];
        };
      };
      if (record.kind !== "GdsLayer" || !record.payload?.display?.visible) continue;
      const payload = record.payload;
      const layerMeshes: Mesh[] = [];
      const material = new StandardMaterial(`${payload.id ?? "layer"}-material`, scene);
      const color = Color3.FromHexString(payload.display?.color ?? "#4c89c8");
      const brightness = payload.display?.brightness ?? 1;
      material.diffuseColor = color.scale(brightness);
      material.alpha = 1;
      material.backFaceCulling = false;
      for (const [index, polygon] of (payload.polygons ?? []).entries()) {
        if (polygon.points.length < 3) continue;
        const shape = polygon.points.map(([x, y]) => new Vector3(x, 0, y));
        const depth = Math.max(
          0.001,
          (payload.display?.z_max ?? 1) - (payload.display?.z_min ?? 0),
        );
        const mesh = MeshBuilder.ExtrudePolygon(
          `${payload.id ?? "layer"}-${index}`,
          { shape, depth },
          scene,
          earcut,
        );
        mesh.position.y = payload.display?.z_min ?? 0;
        mesh.visibility = 1;
        mesh.material = material;
        layerMeshes.push(mesh);
      }
      if (layerMeshes.length > 0) {
        const merged = Mesh.MergeMeshes(layerMeshes, true, true);
        if (merged) {
          merged.name = payload.id ?? "layer";
          merged.material = material;
          merged.metadata = { objectId: payload.id };
          meshes.push(merged);
        }
      } else {
        material.dispose();
      }
    }
    if (shouldFitCamera) fitCamera();
  }

  function fitCamera() {
    if (!camera || meshes.length === 0) return;
    const { min, max } = Mesh.MinMax(meshes);
    const target = min.add(max).scale(0.5);
    const radius = Math.max(max.subtract(min).length() * 1.25, 1);
    camera.setTarget(target);
    camera.radius = radius;
    homeTarget = target.clone();
    homeRadius = radius;
  }

  onMount(() => {
    if (!canvas) return;
    const viewportCanvas = canvas;
    const engine = new Engine(
      viewportCanvas,
      true,
      { preserveDrawingBuffer: true, stencil: true },
      true,
    );
    scene = new Scene(engine);
    scene.clearColor = new Color4(0.95, 0.97, 0.98, 1);
    const activeCamera = new ArcRotateCamera(
      "camera",
      -Math.PI / 2,
      Math.PI / 3,
      100,
      Vector3.Zero(),
      scene,
    );
    camera = activeCamera;
    activeCamera.attachControl(viewportCanvas, true);
    activeCamera.wheelDeltaPercentage = 0.01;
    activeCamera.panningMouseButton = 2;
    activeCamera.panningSensibility = 60;
    activeCamera.lowerRadiusLimit = 0.01;
    activeCamera.upperRadiusLimit = Number.MAX_SAFE_INTEGER;
    const resetCamera = () => {
      activeCamera.alpha = -Math.PI / 2;
      activeCamera.beta = Math.PI / 3;
      activeCamera.radius = homeRadius;
      activeCamera.setTarget(homeTarget);
    };
    viewportCanvas.addEventListener("contextmenu", preventContextMenu);
    viewportCanvas.addEventListener("wheel", preventPageZoom, { passive: false });
    window.addEventListener("gds3d-reset-camera", resetCamera);
    new HemisphericLight("light", new Vector3(0, 1, 0), scene).intensity = 1.1;
    scene.onPointerObservable.add((event) => {
      if (event.type !== PointerEventTypes.POINTERPICK) return;
      const id = event.pickInfo?.hit
        ? (event.pickInfo.pickedMesh?.metadata?.objectId as string | undefined)
        : undefined;
      onSelect?.(id ?? null);
    });
    renderObjects(objects);
    engine.runRenderLoop(() => scene?.render());
    let resizeFrame: number | undefined;
    const resize = () => {
      if (resizeFrame !== undefined) cancelAnimationFrame(resizeFrame);
      resizeFrame = requestAnimationFrame(() => {
        resizeFrame = undefined;
        engine.resize();
      });
    };
    const resizeObserver = new ResizeObserver(resize);
    resizeObserver.observe(viewportCanvas);
    window.addEventListener("resize", resize);
    resize();
    return () => {
      window.removeEventListener("resize", resize);
      window.removeEventListener("gds3d-reset-camera", resetCamera);
      viewportCanvas.removeEventListener("contextmenu", preventContextMenu);
      viewportCanvas.removeEventListener("wheel", preventPageZoom);
      resizeObserver.disconnect();
      if (resizeFrame !== undefined) cancelAnimationFrame(resizeFrame);
      clearMeshes();
      scene?.dispose();
      engine.dispose();
      scene = null;
      camera = null;
    };
  });

  $effect(() => {
    renderObjects(objects);
  });
</script>

<div class="viewport-frame">
  <canvas bind:this={canvas} aria-label="GDS 3D viewport"></canvas>
</div>

<style>
  .viewport-frame {
    position: relative;
    width: 100%;
    height: 100%;
    overflow: hidden;
    border: 1px solid color-mix(in srgb, var(--border) 75%, transparent);
    border-radius: 10px;
    box-shadow: inset 0 1px 0 color-mix(in srgb, white 45%, transparent);
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
