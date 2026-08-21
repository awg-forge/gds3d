<script lang="ts">
  import { onMount } from "svelte";
  import {
    ArcRotateCamera, Color3, Color4, Engine, HemisphericLight, MeshBuilder, Scene, StandardMaterial, Vector3,
  } from "@babylonjs/core";

  interface Props { objects: unknown[] }
  let { objects }: Props = $props();
  let canvas = $state<HTMLCanvasElement>();
  let scene: Scene | null = null;
  let meshes: ReturnType<typeof MeshBuilder.CreatePolygon>[] = [];

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
      for (const [index, polygon] of (payload.polygons ?? []).entries()) {
        if (polygon.points.length < 3) continue;
        const shape = polygon.points.map(([x, y]) => new Vector3(x, 0, y));
        const depth = Math.max(0.001, (payload.display?.z_max ?? 1) - (payload.display?.z_min ?? 0));
        const mesh = MeshBuilder.ExtrudePolygon(`${payload.id ?? "layer"}-${index}`, { shape, depth }, scene);
        mesh.position.y = payload.display?.z_min ?? 0;
        const material = new StandardMaterial(`${mesh.name}-material`, scene);
        const color = Color3.FromHexString(payload.display?.color ?? "#4c89c8");
        const brightness = payload.display?.brightness ?? 1;
        material.diffuseColor = color.scale(brightness);
        material.alpha = Math.min(1, Math.max(0.05, brightness));
        material.backFaceCulling = false;
        if (material.alpha < 1) { material.needDepthPrePass = true; mesh.visibility = material.alpha; }
        mesh.material = material;
        meshes.push(mesh);
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
    new HemisphericLight("light", new Vector3(0, 1, 0), scene).intensity = 1.1;
    renderObjects();
    engine.runRenderLoop(() => scene?.render());
    const resize = () => engine.resize();
    window.addEventListener("resize", resize);
    return () => { window.removeEventListener("resize", resize); clearMeshes(); scene?.dispose(); engine.dispose(); scene = null; };
  });

  $effect(() => { objects; renderObjects(); });
</script>

<canvas bind:this={canvas} aria-label="GDS 3D viewport"></canvas>

<style>
  canvas { width: 100%; height: 100%; display: block; touch-action: none; }
</style>
