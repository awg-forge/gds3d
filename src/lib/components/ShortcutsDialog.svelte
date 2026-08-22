<script lang="ts">
  import { t } from "@i18n";
  import Dialog from "./ui/Dialog.svelte";

  let { open = $bindable(false) }: { open?: boolean } = $props();

  const mouseControls = [
    { action: "gds.mouseSelect", input: "gds.mouseLeftClick" },
    { action: "gds.mouseRotate", input: "gds.mouseLeftDrag" },
    { action: "gds.mousePan", input: "gds.mouseRightDrag" },
    { action: "gds.mouseZoom", input: "gds.mouseWheel" },
  ] as const;
</script>

<Dialog bind:open title={t("gds.shortcuts")} closeLabel={t("gds.closeDialog")} width="460px">
  <section class="shortcut-content">
    <h3>{t("gds.mouseControls")}</h3>
    <div class="shortcut-list">
      {#each mouseControls as control}
        <div class="shortcut-row">
          <span>{t(control.action)}</span>
          <kbd>{t(control.input)}</kbd>
        </div>
      {/each}
    </div>
  </section>
</Dialog>

<style>
  .shortcut-content {
    display: grid;
    gap: 12px;
  }
  h3 {
    margin: 0;
    color: var(--text);
    font-size: 0.85rem;
    font-weight: 600;
  }
  .shortcut-list {
    display: grid;
    overflow: hidden;
    background: var(--surface-soft);
    border: 1px solid var(--border);
    border-radius: 10px;
  }
  .shortcut-row {
    min-height: 44px;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    gap: 16px;
    padding: 0 14px;
    color: var(--text);
  }
  .shortcut-row + .shortcut-row {
    border-top: 1px solid var(--border);
  }
  kbd {
    min-width: 92px;
    padding: 3px 8px;
    color: var(--muted);
    background: var(--surface-strong);
    border: 1px solid var(--border);
    border-radius: 6px;
    font: inherit;
    font-size: 0.8rem;
    line-height: 1.35;
    text-align: center;
    white-space: nowrap;
  }
</style>
