<script lang="ts">
  import type { ViewExportFormat, ViewExportQuality, ViewExportSettings } from "@api/gds";
  import { t } from "@i18n";
  import Button from "./ui/Button.svelte";
  import Dialog from "./ui/Dialog.svelte";
  import Select from "./ui/Select.svelte";

  let { busy, onexport, oncancel } = $props<{
    busy: boolean;
    onexport: (settings: ViewExportSettings) => Promise<void>;
    oncancel: () => void;
  }>();

  let open = $state(true);
  let format = $state<ViewExportFormat>("png");
  let quality = $state<ViewExportQuality>("standard");
  const dimensionsByQuality: Record<ViewExportQuality, { width: number; height: number }> = {
    low: { width: 2400, height: 1800 },
    standard: { width: 4000, height: 3000 },
    high: { width: 6000, height: 4500 },
    ultra: { width: 8000, height: 6000 },
  };
  let dimensions = $derived(dimensionsByQuality[quality]);
  const formatOptions = $derived([
    { label: t("gds.exportFormatPng"), value: "png" },
    { label: t("gds.exportFormatGlb"), value: "glb" },
    { label: t("gds.exportFormatStl"), value: "stl" },
  ]);
  const qualityOptions = $derived([
    { label: t("gds.exportQualityLow"), value: "low" },
    { label: t("gds.exportQualityStandard"), value: "standard" },
    { label: t("gds.exportQualityHigh"), value: "high" },
    { label: t("gds.exportQualityUltra"), value: "ultra" },
  ]);

  $effect(() => {
    if (!open && !busy) oncancel();
  });

  function submit() {
    void onexport({ format, ...dimensions, quality: format === "png" ? quality : undefined });
  }
</script>

<Dialog bind:open title={t("gds.saveAs")} closeLabel={t("gds.closeDialog")} width="440px">
  <div class="export-settings">
    <label
      ><span>{t("gds.exportFormat")}</span><Select
        value={format}
        options={formatOptions}
        onValueChange={(value) => (format = value as ViewExportFormat)}
      /></label
    >
    {#if format === "png"}
      <label
        ><span>{t("gds.exportQuality")}</span><Select
          value={quality}
          options={qualityOptions}
          onValueChange={(value) => (quality = value as ViewExportQuality)}
        /></label
      >
      <output>{dimensions.width} × {dimensions.height} px</output>
    {/if}
  </div>
  {#snippet footer()}
    <Button variant="ghost" disabled={busy} onclick={oncancel}>{t("gds.cancel")}</Button>
    <Button loading={busy} onclick={submit}>{t("gds.exportAction")}</Button>
  {/snippet}
</Dialog>

<style>
  .export-settings {
    display: grid;
    gap: 10px;
  }
  :global(.ui-dialog-content:has(.export-settings) .ui-dialog-body) {
    padding-block: 16px;
  }
  :global(.ui-dialog-content:has(.export-settings) .ui-dialog-header),
  :global(.ui-dialog-content:has(.export-settings) .ui-dialog-footer) {
    padding-block: 12px;
  }
  label {
    display: grid;
    grid-template-columns: 88px minmax(0, 1fr);
    align-items: center;
    gap: 14px;
  }
  label > span {
    color: var(--text);
    font-weight: 500;
  }
  output {
    padding-top: 2px;
    color: var(--muted);
    text-align: center;
    font-variant-numeric: tabular-nums;
  }
</style>
