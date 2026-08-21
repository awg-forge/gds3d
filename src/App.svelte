<script lang="ts">
  import { Camera, FolderOpen, Save, Upload } from "@lucide/svelte";
  import { onMount } from "svelte";
  import {
    chooseGdsPath,
    chooseProjectPath,
    chooseProjectSavePath,
    getSceneSnapshot,
    importGds,
    inspectGdsFile,
    loadProject,
    saveProject,
    updateObjectDisplay,
    type GdsFileInfo,
    type SceneSnapshot,
  } from "@api/gds";
  import Button from "./lib/components/ui/Button.svelte";
  import ColorPicker from "./lib/components/ui/ColorPicker.svelte";
  import Slider from "./lib/components/ui/Slider.svelte";
  import Viewport from "./lib/Viewport.svelte";

  type Entry = {
    kind?: string;
    payload?: {
      id?: string;
      display?: {
        name?: string;
        color?: string;
        brightness?: number;
        visible?: boolean;
        z_min?: number;
        z_max?: number;
      };
    };
  };
  let fileInfo = $state<GdsFileInfo | null>(null);
  let scene = $state<SceneSnapshot | null>(null);
  let selectedPath = $state<string | null>(null);
  let selectedId = $state<string | null>(null);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let selected = $derived(
    scene?.objects.find((item) => (item as Entry).payload?.id === selectedId) as Entry | undefined,
  );
  let objects = $derived(
    (scene?.objects ?? []).map((item) => item as Entry).filter((item) => item.kind === "GdsLayer"),
  );

  onMount(() => {
    void getSceneSnapshot().then((snapshot) => (scene = snapshot));
  });

  async function chooseGds() {
    const path = await chooseGdsPath();
    if (!path) return;
    busy = true;
    error = null;
    selectedPath = path;
    selectedId = null;
    try {
      fileInfo = await inspectGdsFile(path);
      scene = await importGds(path);
    } catch (reason) {
      error = reason instanceof Error ? reason.message : String(reason);
    } finally {
      busy = false;
    }
  }

  async function openProject() {
    const path = await chooseProjectPath();
    if (!path) return;
    busy = true;
    error = null;
    try {
      scene = await loadProject(path);
      selectedPath = path;
      selectedId = null;
      fileInfo = null;
    } catch (reason) {
      error = reason instanceof Error ? reason.message : String(reason);
    } finally {
      busy = false;
    }
  }

  async function saveCurrentProject() {
    if (!scene) return;
    const path = await chooseProjectSavePath();
    if (!path) return;
    try {
      await saveProject(path);
    } catch (reason) {
      error = reason instanceof Error ? reason.message : String(reason);
    }
  }

  async function updateSelected(update: {
    color?: string;
    brightness?: number;
    visible?: boolean;
  }) {
    if (!selectedId) return;
    try {
      scene = await updateObjectDisplay({ objectId: selectedId, ...update });
    } catch (reason) {
      error = reason instanceof Error ? reason.message : String(reason);
    }
  }

  async function updateVisibility(id: string, visible: boolean) {
    try {
      scene = await updateObjectDisplay({ objectId: id, visible });
    } catch (reason) {
      error = reason instanceof Error ? reason.message : String(reason);
    }
  }

  function resetCamera() {
    window.dispatchEvent(new CustomEvent("gds3d-reset-camera"));
  }
</script>

<svelte:head><title>gds3d</title></svelte:head>

<main class="gds-shell">
  <header class="gds-toolbar">
    <strong class="gds-brand">gds3d</strong>
    <div class="gds-actions">
      <Button size="sm" loading={busy} onclick={chooseGds}><FolderOpen size={16} />Open GDS</Button>
      <Button size="sm" variant="outline" onclick={openProject}
        ><Upload size={16} />Open project</Button
      >
      <Button size="sm" variant="outline" disabled={!scene} onclick={saveCurrentProject}
        ><Save size={16} />Save project</Button
      >
      <Button size="sm" variant="ghost" onclick={resetCamera}
        ><Camera size={16} />Reset camera</Button
      >
    </div>
    <span class="gds-path" title={selectedPath ?? undefined}
      >{selectedPath ?? "No file loaded"}</span
    >
  </header>

  <section class="gds-workbench">
    <aside class="gds-panel gds-layers">
      <div class="gds-heading">
        <h2>Layers</h2>
        <span>{objects.length}</span>
      </div>
      {#if fileInfo}
        {#each fileInfo.cells as cell}
          <section class="gds-cell">
            <h3>{cell.name}</h3>
            {#each cell.layers as layer}<div class="gds-layer">
                <span>L{layer.selection.layer}/D{layer.selection.datatype}</span><small
                  >{layer.polygon_count} polygons</small
                >
              </div>{/each}
          </section>
        {/each}
      {/if}
      {#if objects.length}
        <div class="gds-object-list">
          {#each objects as item}
            {@const id = item.payload?.id ?? ""}
            <div class:selected={id === selectedId} class="gds-object">
              <Button
                class="gds-object-button"
                variant="ghost"
                size="sm"
                onclick={() => (selectedId = id)}>{item.payload?.display?.name ?? "Layer"}</Button
              >
              <input
                aria-label="Toggle layer visibility"
                type="checkbox"
                checked={item.payload?.display?.visible ?? true}
                onchange={(event) =>
                  id && updateVisibility(id, (event.currentTarget as HTMLInputElement).checked)}
              />
            </div>
          {/each}
        </div>
      {:else}<p class="gds-empty">Open a GDS file to inspect its layers.</p>{/if}
    </aside>

    <section class="gds-viewport">
      {#if scene}<Viewport
          objects={scene.objects}
          onSelect={(id) => (selectedId = id)}
        />{:else}<div class="gds-viewport-empty">
          <FolderOpen size={32} /><strong>Open a GDS layout</strong><span
            >Its 3D scene will appear here.</span
          >
        </div>{/if}
    </section>

    <aside class="gds-panel gds-properties">
      <div class="gds-heading"><h2>Properties</h2></div>
      {#if selected}
        <div class="gds-property-grid">
          <h3>{selected.payload?.display?.name ?? selected.payload?.id}</h3>
          <div class="gds-property">
            <span>Visible</span><input
              type="checkbox"
              checked={selected.payload?.display?.visible ?? true}
              onchange={(event) =>
                updateSelected({ visible: (event.currentTarget as HTMLInputElement).checked })}
            />
          </div>
          <div class="gds-property">
            <span>Color</span><ColorPicker
              label="Layer color"
              value={selected.payload?.display?.color ?? "#2D6CDF"}
              onvaluechange={(color) => updateSelected({ color })}
            />
          </div>
          <div class="gds-property-stack">
            <span>Brightness</span><Slider
              value={selected.payload?.display?.brightness ?? 1}
              min={0.05}
              max={2}
              step={0.05}
              ariaLabel="Layer brightness"
              onvaluechange={(brightness) => updateSelected({ brightness })}
            />
          </div>
          <div class="gds-property">
            <span>Z range</span><strong
              >{selected.payload?.display?.z_min} — {selected.payload?.display?.z_max}</strong
            >
          </div>
        </div>
      {:else if scene}<p class="gds-empty">
          Select a layer in the viewport or layer panel to edit it.
        </p>{:else}<p class="gds-empty">No scene loaded.</p>{/if}
    </aside>
  </section>
  {#if error}<div class="gds-error">{error}</div>{/if}
</main>

<style>
  .gds-shell {
    width: 100%;
    height: 100%;
    min-height: 0;
    display: grid;
    grid-template-rows: 56px minmax(0, 1fr);
    color: var(--text);
    background: var(--background);
  }
  .gds-toolbar {
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 18px;
    padding: 0 18px;
    border-bottom: 1px solid var(--border);
    background: var(--surface);
  }
  .gds-brand {
    font-size: 1.08rem;
    letter-spacing: 0.04em;
  }
  .gds-actions {
    display: flex;
    gap: 8px;
  }
  .gds-path {
    min-width: 0;
    overflow: hidden;
    color: var(--muted);
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .gds-workbench {
    min-height: 0;
    display: grid;
    grid-template-columns: 264px minmax(0, 1fr) 300px;
  }
  .gds-panel {
    min-width: 0;
    overflow: auto;
    padding: 18px;
    background: var(--surface);
  }
  .gds-layers {
    border-right: 1px solid var(--border);
  }
  .gds-properties {
    border-left: 1px solid var(--border);
  }
  .gds-heading {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 16px;
  }
  .gds-heading h2 {
    margin: 0;
    font-size: 1rem;
  }
  .gds-heading span {
    min-width: 22px;
    padding: 2px 7px;
    border-radius: 999px;
    color: var(--muted);
    background: var(--surface-soft);
    font-size: 0.78rem;
    text-align: center;
  }
  .gds-cell {
    margin-bottom: 18px;
  }
  .gds-cell h3 {
    margin: 0 0 7px;
    color: var(--muted);
    font-size: 0.9rem;
  }
  .gds-layer {
    display: flex;
    justify-content: space-between;
    padding: 8px 4px;
    border-bottom: 1px solid var(--border);
  }
  .gds-layer small {
    color: var(--muted);
  }
  .gds-object-list {
    display: grid;
    gap: 3px;
    padding-top: 8px;
    border-top: 1px solid var(--border);
  }
  .gds-object {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 24px;
    align-items: center;
    gap: 4px;
    border-radius: 8px;
  }
  .gds-object.selected {
    background: color-mix(in srgb, var(--primary) 12%, transparent);
  }
  .gds-object-button {
    width: 100%;
    justify-content: flex-start;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .gds-object input,
  .gds-property input {
    accent-color: var(--primary);
  }
  .gds-viewport {
    min-width: 0;
    min-height: 0;
    background: var(--surface-soft);
  }
  .gds-viewport-empty {
    width: 100%;
    height: 100%;
    display: grid;
    place-content: center;
    justify-items: center;
    gap: 10px;
    color: var(--muted);
  }
  .gds-property-grid {
    display: grid;
    gap: 16px;
  }
  .gds-property-grid h3 {
    margin: 0;
    overflow-wrap: anywhere;
  }
  .gds-property {
    min-height: 34px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    color: var(--muted);
  }
  .gds-property strong {
    color: var(--text);
    font-size: 0.86rem;
  }
  .gds-property-stack {
    display: grid;
    gap: 8px;
    color: var(--muted);
  }
  .gds-empty {
    color: var(--muted);
    line-height: 1.55;
  }
  .gds-error {
    position: fixed;
    right: 18px;
    bottom: 18px;
    max-width: 520px;
    padding: 12px 16px;
    border-radius: 8px;
    color: white;
    background: var(--danger);
    box-shadow: var(--shadow);
  }
  @media (max-width: 960px) {
    .gds-workbench {
      grid-template-columns: 220px minmax(0, 1fr);
    }
    .gds-properties {
      display: none;
    }
    .gds-actions :global(.ui-button:nth-child(3)) {
      display: none;
    }
  }
</style>
