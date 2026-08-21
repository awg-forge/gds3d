<script lang="ts">
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { onMount } from "svelte";
  import { getSceneSnapshot, importGds, inspectGdsFile, saveProject, updateObjectDisplay, type GdsFileInfo, type SceneSnapshot } from "@api/gds";
  import Viewport from "./lib/Viewport.svelte";

  let fileInfo = $state<GdsFileInfo | null>(null);
  let scene = $state<SceneSnapshot | null>(null);
  let selectedPath = $state<string | null>(null);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let selectedId = $state<string | null>(null);

  onMount(() => { void getSceneSnapshot().then((snapshot) => (scene = snapshot)); });

  async function chooseGds() {
    const path = await open({ multiple: false, directory: false, filters: [{ name: "GDSII layout", extensions: ["gds", "gdsii"] }] });
    if (typeof path !== "string") return;
    selectedPath = path; fileInfo = null; scene = null; error = null; busy = true;
    try { fileInfo = await inspectGdsFile(path); scene = await importGds(path); }
    catch (reason) { error = reason instanceof Error ? reason.message : String(reason); }
    finally { busy = false; }
  }

  function selectedObject() {
    return scene?.objects.find((entry) => (entry as { payload?: { id?: string } }).payload?.id === selectedId) as { payload?: { id?: string; display?: { name?: string; color?: string; brightness?: number; visible?: boolean; z_min?: number; z_max?: number } } } | undefined;
  }

  let selected = $derived(selectedObject());

  async function saveCurrentProject() {
    if (!scene) return;
    const path = await save({ defaultPath: "gds3d-project.gds3d", filters: [{ name: "gds3d project", extensions: ["gds3d"] }] });
    if (!path) return;
    try { await saveProject(path); } catch (reason) { error = reason instanceof Error ? reason.message : String(reason); }
  }

  async function updateSelected(update: { color?: string; brightness?: number }) {
    if (!selectedId) return;
    try { scene = await updateObjectDisplay({ objectId: selectedId, ...update }); }
    catch (reason) { error = reason instanceof Error ? reason.message : String(reason); }
  }

  function resetCamera() { window.dispatchEvent(new CustomEvent("gds3d-reset-camera")); }
</script>

<svelte:head><title>gds3d</title></svelte:head>

<main class="app-shell">
  <header class="toolbar"><div class="brand">gds3d</div><button onclick={chooseGds} disabled={busy}>{busy ? "Loading…" : "Open GDS"}</button><button class="secondary" onclick={saveCurrentProject} disabled={!scene}>Save project</button><button class="secondary" onclick={resetCamera}>Reset camera</button>{#if selectedPath}<span class="path">{selectedPath}</span>{/if}</header>
  <section class="workspace">
    <aside class="sidebar"><h2>Layers</h2>
      {#if fileInfo}{#each fileInfo.cells as cell}<h3>{cell.name}</h3>{#each cell.layers as layer}<div class="layer-row"><span>L{layer.selection.layer}/D{layer.selection.datatype}</span><small>{layer.polygon_count} polygons</small></div>{/each}{/each}{:else}<p class="muted">Open a GDS file to inspect its layers.</p>{/if}
    </aside>
    <section class="viewport">{#if scene}<Viewport objects={scene.objects} onSelect={(id) => (selectedId = id)} />{:else}<div class="viewport-placeholder"><div class="cube">◇</div><strong>Babylon viewport</strong><span>Waiting for a GDS scene</span></div>{/if}</section>
    <aside class="properties"><h2>Properties</h2>{#if selected}<p class="selected-name">{selected.payload?.display?.name ?? selected.payload?.id}</p><label>Color<input type="color" value={selected.payload?.display?.color ?? "#4c89c8"} onchange={(event) => updateSelected({ color: (event.currentTarget as HTMLInputElement).value })} /></label><label>Brightness<input type="range" min="0.1" max="2" step="0.05" value={selected.payload?.display?.brightness ?? 1} oninput={(event) => updateSelected({ brightness: Number((event.currentTarget as HTMLInputElement).value) })} /></label><p>Z: {selected.payload?.display?.z_min} → {selected.payload?.display?.z_max}</p>{:else if scene}<p>{scene.objects.length} objects</p><p class="muted">Click a layer in the viewport to inspect it.</p>{:else}<p class="muted">Nothing selected</p>{/if}</aside>
  </section>
  {#if error}<div class="error">{error}</div>{/if}
</main>

<style>
  :global(*) { box-sizing: border-box; }
  :global(body) { margin: 0; background: #edf1f5; color: #1c2630; font: 14px system-ui, sans-serif; }
  .app-shell { height: 100vh; display: flex; flex-direction: column; }
  .toolbar { height: 52px; display: flex; align-items: center; gap: 16px; padding: 0 18px; background: #243447; color: white; }
  .brand { font-weight: 700; letter-spacing: .08em; } button { border: 0; border-radius: 4px; padding: 7px 14px; background: #4c89c8; color: white; cursor: pointer; } button:disabled { opacity: .6; cursor: wait; }
  .path, .muted { color: #82909d; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; } .path { color: #cbd7e3; } .secondary { background: #506274; }
  .workspace { min-height: 0; flex: 1; display: grid; grid-template-columns: 240px 1fr 260px; } aside { padding: 16px; background: #f7f9fb; overflow: auto; } .sidebar { border-right: 1px solid #d9e0e7; } .properties { border-left: 1px solid #d9e0e7; }
  h2 { margin: 0 0 18px; font-size: 15px; } h3 { margin: 16px 0 6px; font-size: 12px; color: #647484; } .layer-row { display: flex; justify-content: space-between; padding: 7px 4px; border-bottom: 1px solid #e5eaf0; } small { color: #82909d; } label { display: grid; gap: 6px; margin: 14px 0; color: #647484; } input[type="range"] { width: 100%; } .selected-name { font-weight: 600; }
  .viewport { display: grid; place-items: center; background: #f1f4f7; } .viewport-placeholder { display: grid; place-items: center; gap: 8px; color: #627487; } .cube { font-size: 64px; color: #4c89c8; } .error { position: fixed; right: 18px; bottom: 18px; max-width: 520px; padding: 12px 16px; background: #9b3d3d; color: white; border-radius: 5px; }
</style>
