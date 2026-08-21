<script lang="ts">
  import { onMount } from "svelte";
  import { getDesktopPreferences, getSystemFonts, updateDesktopPreferences } from "@api/desktop";
  import { t } from "@i18n";
  import { MAX_FONT_SIZE, MIN_FONT_SIZE } from "../../themes/typography";
  import Select from "./ui/Select.svelte";
  import Slider from "./ui/Slider.svelte";
  import Toggle from "./ui/Toggle.svelte";

  let { fontSize, fontFamily, lightingIntensity, ontypographychange, onlightingchange } = $props<{
    fontSize: number;
    fontFamily: string;
    lightingIntensity: number;
    ontypographychange: (value: { fontSize?: number; fontFamily?: string }) => void;
    onlightingchange: (value: number) => void;
  }>();

  let fonts = $state<string[]>([]);
  const fontOptions = $derived([
    { label: t("gds.systemFont"), value: "" },
    ...(fontFamily && !fonts.includes(fontFamily)
      ? [{ label: fontFamily, value: fontFamily, fontFamily }]
      : []),
    ...fonts.map((font) => ({ label: font, value: font, fontFamily: font })),
  ]);

  let rememberWindowState = $state(true);
  let closeToTray = $state(false);
  let savingWindowPreferences = $state(false);

  onMount(() => {
    void getDesktopPreferences()
      .then((preferences) => {
        rememberWindowState = preferences.rememberWindowState;
        closeToTray = preferences.closeToTray;
        return undefined;
      })
      .catch(() => undefined);
    void getSystemFonts()
      .then((systemFonts) => {
        fonts = systemFonts;
        return undefined;
      })
      .catch(() => undefined);
  });

  function updateFontFamily(next: string): void {
    ontypographychange({ fontFamily: next });
  }

  function updateFontSize(next: number): void {
    ontypographychange({ fontSize: next });
  }

  function saveWindowPreferences(
    next: Partial<{ rememberWindowState: boolean; closeToTray: boolean }>,
  ): void {
    const preferences = {
      rememberWindowState: next.rememberWindowState ?? rememberWindowState,
      closeToTray: next.closeToTray ?? closeToTray,
    };
    rememberWindowState = preferences.rememberWindowState;
    closeToTray = preferences.closeToTray;
    savingWindowPreferences = true;
    void updateDesktopPreferences(preferences).finally(() => (savingWindowPreferences = false));
  }
</script>

<div class="workspace settings-workspace">
  <section class="settings-section">
    <div class="settings-section-heading"><h2>{t("gds.appearance")}</h2></div>
    <div class="preference-row">
      <span>{t("gds.fontFamily")}</span>
      <Select
        class="settings-select font-family-select"
        value={fontFamily}
        options={fontOptions}
        searchable
        searchPlaceholder={t("gds.searchFont")}
        emptyLabel={t("gds.noResults")}
        onValueChange={updateFontFamily}
      />
    </div>
    <div class="preference-row">
      <span>{t("gds.fontSize")}</span>
      <div class="font-size-control">
        <Slider
          id="font-size-slider"
          min={MIN_FONT_SIZE}
          max={MAX_FONT_SIZE}
          value={fontSize}
          ariaLabel={t("gds.fontSize")}
          ariaValueText={`${fontSize}px`}
          onvaluechange={updateFontSize}
        />
        <output for="font-size-slider">{fontSize}px</output>
      </div>
    </div>
  </section>

  <section class="settings-section">
    <div class="settings-section-heading"><h2>{t("gds.viewport")}</h2></div>
    <div class="preference-row">
      <span>{t("gds.globalLighting")}</span>
      <div class="font-size-control">
        <Slider
          id="global-lighting-slider"
          min={0.1}
          max={2}
          step={0.05}
          value={lightingIntensity}
          ariaLabel={t("gds.globalLighting")}
          ariaValueText={`${Math.round(lightingIntensity * 100)}%`}
          onvaluechange={onlightingchange}
        />
        <output for="global-lighting-slider">{Math.round(lightingIntensity * 100)}%</output>
      </div>
    </div>
  </section>

  <section class="settings-section">
    <div class="settings-section-heading"><h2>{t("gds.window")}</h2></div>
    <div class="preference-row switch-row">
      <span>{t("gds.rememberWindowState")}</span>
      <Toggle
        checked={rememberWindowState}
        disabled={savingWindowPreferences}
        label={t("gds.rememberWindowState")}
        oncheckedchange={(checked) => saveWindowPreferences({ rememberWindowState: checked })}
      />
    </div>
    <div class="preference-row switch-row">
      <span>{t("gds.closeToTray")}</span>
      <Toggle
        checked={closeToTray}
        disabled={savingWindowPreferences}
        label={t("gds.closeToTray")}
        oncheckedchange={(checked) => saveWindowPreferences({ closeToTray: checked })}
      />
    </div>
  </section>
</div>

<style>
  .settings-workspace {
    width: min(760px, calc(100% - 48px));
    min-height: 100%;
    margin: 0 auto;
    padding: 24px 0 32px;
    align-content: start;
    gap: 16px;
  }

  .preference-row {
    min-height: 38px;
    display: grid;
    grid-template-columns: minmax(150px, 1fr) var(--settings-control-width);
    align-items: center;
    gap: 20px;
  }

  .preference-row > span {
    font-weight: 500;
  }

  :global(.settings-select),
  .font-size-control,
  .switch-row :global(.gds-toggle) {
    width: var(--settings-control-width);
    justify-self: end;
  }

  .font-size-control {
    max-width: var(--settings-control-width);
    display: grid;
    grid-template-columns: minmax(0, 1fr) 40px;
    align-items: center;
    gap: 10px;
  }

  output {
    color: var(--muted);
    font-variant-numeric: tabular-nums;
    text-align: right;
  }

  .switch-row :global(.gds-toggle) {
    width: 38px;
  }

  @media (max-width: 680px) {
    .settings-workspace {
      width: calc(100% - 32px);
      padding-top: 16px;
    }

    .preference-row {
      grid-template-columns: minmax(0, 1fr);
      gap: 8px;
    }

    :global(.settings-select),
    .font-size-control,
    .switch-row :global(.gds-toggle) {
      justify-self: stretch;
    }
  }
</style>
