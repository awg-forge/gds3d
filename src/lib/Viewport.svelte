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

  interface Props { objects: unknown[]; onSelect?: (id: string | null) => void }
  let { objects, onSelect }: Props = $props();
  let canvas = $state<HTMLCanvasElement>();
  let scene: Scene | null = null;
  let meshes: Mesh[] = [];

  function clearMeshes() {
    for (const mesh of meshes) mesh.dispose();
    meshes = [];
  }

  function renderObjects() {
    if (!scene) return;
    clearMeshes();
    for (const entry of objects) {
      const record = entry as { kind?: string; payload?: { id?: string; display?: { color?: string; brightness?: number; visible?: boolean; z_min?: number; z_max?: number }; polygons?: { points: number[][] }[] } };
      if (record.kind !== "GdsLayer" || !record.payload?.display?.visible) continue;
      const payload = record.payload;
      const layerMeshes: Mesh[] = [];
      const material = new StandardMaterial(`${payload.id ?? "layer"}-material`, scene);
      const color = Color3.FromHexString(payload.display?.color ?? "#4c89c8");
      const brightness = payload.display?.brightness ?? 1;
      material.diffuseColor = color.scale(brightness);
      material.alpha = Math.min(1, Math.max(0.05, brightness));
      material.backFaceCulling = false;
      if (material.alpha < 1) material.needDepthPrePass = true;
      for (const [index, polygon] of (payload.polygons ?? []).entries()) {
        if (polygon.points.length < 3) continue;
        const shape = polygon.points.map(([x, y]) => new Vector3(x, 0, y));
        const depth = Math.max(0.001, (payload.display?.z_max ?? 1) - (payload.display?.z_min ?? 0));
        const mesh = MeshBuilder.ExtrudePolygon(`${payload.id ?? "layer"}-${index}`, { shape, depth }, scene);
        mesh.position.y = payload.display?.z_min ?? 0;
        mesh.visibility = material.alpha;
        mesh.material = material;
        layerMeshes.push(mesh);
      }
      if (layerMeshes.length > 0) {
        const merged = Mesh.MergeMeshes(layerMeshes, true, true);
        if (merged) { merged.name = payload.id ?? "layer"; merged.material = material; merged.metadata = { objectId: payload.id }; meshes.push(merged); }
      } else {
        material.dispose();
      }
    }
  }

  onMount(() => {
    if (!canvas) return;
    const engine = new Engine(canvas, true, { preserveDrawingBuffer: true, stencil: true });
    scene = new Scene(engine);
    scene.clearColor = new Color4(0.95, 0.97, 0.98, 1);
    const camera = new ArcRotateCamera("camera", -Math.PI / 2, Math.PI / 3, 100, Vector3.Zero(), scene);
    camera.attachControl(canvas, true);
    camera.wheelPrecision = 30;
    const resetCamera = () => { camera.alpha = -Math.PI / 2; camera.beta = Math.PI / 3; camera.radius = 100; camera.setTarget(Vector3.Zero()); };
    window.addEventListener("gds3d-reset-camera", resetCamera);
    new HemisphericLight("light", new Vector3(0, 1, 0), scene).intensity = 1.1;
    scene.onPointerObservable.add((event) => {
      if (event.type !== PointerEventTypes.POINTERPICK) return;
      const id = event.pickInfo?.hit ? (event.pickInfo.pickedMesh?.metadata?.objectId as string | undefined) : undefined;
      onSelect?.(id ?? null);
    });
    renderObjects();
    engine.runRenderLoop(() => scene?.render());
    const resize = () => engine.resize();
    window.addEventListener("resize", resize);
    return () => { window.removeEventListener("resize", resize); window.removeEventListener("gds3d-reset-camera", resetCamera); clearMeshes(); scene?.dispose(); engine.dispose(); scene = null; };
  });

  $effect(() => { objects; renderObjects(); });
</script>

<canvas bind:this={canvas} aria-label="GDS 3D viewport"></canvas>

<style>
  canvas { width: 100%; height: 100%; display: block; touch-action: none; }
</style>
