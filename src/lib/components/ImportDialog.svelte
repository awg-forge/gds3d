<script lang="ts">
  import { onMount, tick } from "svelte";
  import type { GdsFileInfo, GdsLayerSelection } from "@api/gds";
  import { t } from "@i18n";
  import Button from "./ui/Button.svelte";
  import Checkbox from "./ui/Checkbox.svelte";
  import Dialog from "./ui/Dialog.svelte";

  let {
    info,
    busy,
    onimport,
    oncancel,
  }: {
    info: GdsFileInfo;
    busy: boolean;
    onimport: (selections: GdsLayerSelection[]) => Promise<void>;
    oncancel: () => void;
  } = $props();

  let allLayers = $derived(info.cells.flatMap((cell) => cell.layers));
  let selectedKeys = $state<string[]>([]);
  let warning = $state<string | null>(null);
  let dialogOpen = $state(true);
  let submitting = $state(false);
  let loading = $derived(busy || submitting);

  onMount(() => {
    selectedKeys = initialSelection(info);
  });

  $effect(() => {
    if (!dialogOpen) {
      if (loading) dialogOpen = true;
      else oncancel();
    }
  });

  function selectionKey(selection: GdsLayerSelection): string {
    return `${selection.cell_name}\u0000${selection.layer}\u0000${selection.datatype}`;
  }

  function initialSelection(file: GdsFileInfo): string[] {
    const layers = file.cells.flatMap((cell) => cell.layers);
    return layers.length === 1 ? [selectionKey(layers[0].selection)] : [];
  }

  function isSelected(selection: GdsLayerSelection): boolean {
    return selectedKeys.includes(selectionKey(selection));
  }

  function toggleLayer(selection: GdsLayerSelection, checked: boolean) {
    const key = selectionKey(selection);
    selectedKeys = checked
      ? Array.from(new Set([...selectedKeys, key]))
      : selectedKeys.filter((selectedKey) => selectedKey !== key);
    warning = null;
  }

  function toggleCell(cellIndex: number, checked: boolean) {
    const cellKeys = info.cells[cellIndex].layers.map((layer) => selectionKey(layer.selection));
    if (checked) {
      selectedKeys = Array.from(new Set([...selectedKeys, ...cellKeys]));
    } else {
      const removedKeys = new Set(cellKeys);
      selectedKeys = selectedKeys.filter((key) => !removedKeys.has(key));
    }
    warning = null;
  }

  async function confirmImport() {
    if (loading) return;
    const selections = allLayers
      .filter((layer) => isSelected(layer.selection))
      .map((layer) => layer.selection);
    if (selections.length === 0) {
      warning = t("gds.selectLayerWarning");
      return;
    }
    submitting = true;
    await tick();
    await nextPaint();
    try {
      await onimport(selections);
    } finally {
      submitting = false;
    }
  }

  function nextPaint(): Promise<void> {
    return new Promise((resolve) =>
      requestAnimationFrame(() => requestAnimationFrame(() => window.setTimeout(resolve, 0))),
    );
  }

  function formatBounds(bounds: { min_x: number; min_y: number; max_x: number; max_y: number }) {
    return `X ${bounds.min_x.toFixed(2)}–${bounds.max_x.toFixed(2)}, Y ${bounds.min_y.toFixed(2)}–${bounds.max_y.toFixed(2)}`;
  }
</script>

<Dialog
  bind:open={dialogOpen}
  title={t("gds.importGdsTitle")}
  closeLabel={t("gds.closeDialog")}
  width="640px"
>
  <div class="import-content">
    <p class="import-path" title={info.file_path}><span>{t("gds.path")}</span>{info.file_path}</p>
    <div class="import-tree">
      {#each info.cells as cell, cellIndex}
        {@const checkedCount = cell.layers.filter((layer) => isSelected(layer.selection)).length}
        {@const cellChecked = cell.layers.length > 0 && checkedCount === cell.layers.length}
        <section class="import-cell">
          <label class="cell-option">
            <Checkbox
              checked={cellChecked}
              indeterminate={checkedCount > 0 && !cellChecked}
              ariaLabel={cell.name}
              oncheckedchange={(checked) => toggleCell(cellIndex, checked)}
            />
            <strong>{cell.name}</strong>
            {#if checkedCount > 0 && !cellChecked}<small>{checkedCount}/{cell.layers.length}</small
              >{/if}
          </label>
          <div class="cell-layers">
            {#each cell.layers as layer}
              <label class="layer-option">
                <Checkbox
                  checked={isSelected(layer.selection)}
                  ariaLabel={`L${layer.selection.layer}/D${layer.selection.datatype}`}
                  oncheckedchange={(checked) => toggleLayer(layer.selection, checked)}
                />
                <span class="layer-name">L{layer.selection.layer}/D{layer.selection.datatype}</span>
                <small>{t("gds.polygonCount", { count: layer.polygon_count })}</small>
                <small class="layer-bounds" title={formatBounds(layer.bounds)}
                  >{formatBounds(layer.bounds)}</small
                >
              </label>
            {/each}
          </div>
        </section>
      {/each}
    </div>

    {#if warning}<p class="import-warning">{warning}</p>{/if}
  </div>
  {#snippet footer()}
    <Button variant="ghost" disabled={loading} onclick={oncancel}>{t("gds.cancel")}</Button>
    <Button {loading} onclick={() => void confirmImport()}>{t("gds.import")}</Button>
  {/snippet}
</Dialog>

<style>
  .import-content {
    display: grid;
    gap: 10px;
  }
  .import-path {
    margin: 0;
    overflow: hidden;
    color: var(--muted);
    font-size: 0.82rem;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .import-tree {
    min-height: 112px;
    max-height: min(240px, calc(100vh - 260px));
    overflow: auto;
    padding: 2px;
  }
  .import-cell + .import-cell {
    margin-top: 8px;
    padding-top: 8px;
    border-top: 1px solid var(--border);
  }
  .cell-option,
  .layer-option {
    display: grid;
    align-items: center;
    cursor: pointer;
  }
  .cell-option {
    grid-template-columns: 20px minmax(0, 1fr) auto;
    gap: 7px;
    min-height: 34px;
    padding: 0 7px;
  }
  .cell-option small,
  .layer-option small {
    color: var(--muted);
  }
  .cell-layers {
    display: grid;
    gap: 2px;
    margin-left: 20px;
  }
  .layer-option {
    grid-template-columns: 20px 72px 104px minmax(0, 1fr);
    gap: 7px;
    min-height: 34px;
    padding: 0 7px;
    border-radius: var(--gds-radius-xs);
  }
  .layer-option:hover {
    background: var(--surface-soft);
  }
  .layer-name {
    font-weight: 500;
  }
  .layer-bounds {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .import-warning {
    margin: 0;
    padding: 8px 2px 0;
    color: var(--danger);
    font-size: 0.86rem;
  }
</style>
