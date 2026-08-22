<script lang="ts">
  import type { ViewExportFormat, ViewExportSettings } from "@api/gds";
  import { t } from "@i18n";
  import Button from "./ui/Button.svelte";
  import Dialog from "./ui/Dialog.svelte";
  import Select from "./ui/Select.svelte";

  type Quality = "low" | "standard" | "high";
  type SizePreset = "4:3" | "3:2" | "16:9" | "1:1";

  let { busy, onexport, oncancel } = $props<{
    busy: boolean;
    onexport: (settings: ViewExportSettings) => Promise<void>;
    oncancel: () => void;
  }>();

  let open = $state(true);
  let format = $state<ViewExportFormat>("png");
  let quality = $state<Quality>("standard");
  let sizePreset = $state<SizePreset>("4:3");
  const widths: Record<Quality, number> = { low: 2000, standard: 4000, high: 6000 };
  const ratios: Record<SizePreset, [number, number]> = {
    "4:3": [4, 3],
    "3:2": [3, 2],
    "16:9": [16, 9],
    "1:1": [1, 1],
  };
  let dimensions = $derived.by(() => {
    const width = widths[quality];
    const [ratioWidth, ratioHeight] = ratios[sizePreset];
    return { width, height: Math.floor((width * ratioHeight) / ratioWidth) };
  });
  const formatOptions = $derived([
    { label: t("gds.exportFormatPng"), value: "png" },
    { label: t("gds.exportFormatSvg"), value: "svg" },
    { label: t("gds.exportFormatGlb"), value: "glb" },
    { label: t("gds.exportFormatStl"), value: "stl" },
  ]);
  const sizeOptions = $derived([
    { label: t("gds.exportSize4x3"), value: "4:3" },
    { label: t("gds.exportSize3x2"), value: "3:2" },
    { label: t("gds.exportSize16x9"), value: "16:9" },
    { label: t("gds.exportSize1x1"), value: "1:1" },
  ]);
  const qualityOptions = $derived([
    { label: t("gds.exportQualityLow"), value: "low" },
    { label: t("gds.exportQualityStandard"), value: "standard" },
    { label: t("gds.exportQualityHigh"), value: "high" },
  ]);

  $effect(() => {
    if (!open && !busy) oncancel();
  });

  function submit() {
    void onexport({ format, ...dimensions });
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
    {#if format === "png" || format === "svg"}
      <label
        ><span>{t("gds.exportSize")}</span><Select
          value={sizePreset}
          options={sizeOptions}
          onValueChange={(value) => (sizePreset = value as SizePreset)}
        /></label
      >
      <label
        ><span>{t("gds.exportQuality")}</span><Select
          value={quality}
          options={qualityOptions}
          onValueChange={(value) => (quality = value as Quality)}
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
