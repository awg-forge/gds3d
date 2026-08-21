<script lang="ts">
  import { onMount } from "svelte";
  import { getSystemFonts, openTextFile, saveTextFile, supportsLiquidGlass } from "@api/settings";
  import { t } from "@i18n";
  import type { ColorThemeId } from "@models/preferences";
  import type { Preferences, ThemeColors } from "@models/preferences";
  import { getThemeOptions } from "@themes";
  import { DEFAULT_CUSTOM_THEME } from "@themes/custom";
  import { MAX_FONT_SIZE, MIN_FONT_SIZE } from "@themes/typography";
  import { showToast } from "../state";
  import Button from "./ui/Button.svelte";
  import ColorPicker from "./ui/ColorPicker.svelte";
  import Select, { type Option, type PointerOrigin } from "./ui/Select.svelte";
  import Slider from "./ui/Slider.svelte";
  import Toggle from "./ui/Toggle.svelte";
  import { Download, RotateCcw, Upload, X } from "@lucide/svelte";

  let { value, onupdate, onthemechange } = $props<{
    value: Preferences;
    onupdate: (value: Partial<Preferences>) => void;
    onthemechange?: (theme: Preferences["theme"], origin: PointerOrigin) => void;
  }>();
  let fonts = $state<string[]>([]);
  let liquidGlassSupported = $state(false);
  let platform = $state<"macos" | "windows" | "other">("other");
  let activePlan = $state<"light" | "dark">("light");
  let backgroundFileInput = $state<HTMLInputElement>();
  const customColorFields: Array<{ key: keyof ThemeColors; label: string }> = [
    { key: "bg", label: "background" },
    { key: "bgSecondary", label: "surface" },
    { key: "bgTertiary", label: "surfaceStrong" },
    { key: "primary", label: "primary" },
    { key: "primarySolid", label: "primarySolid" },
    { key: "primarySolidHover", label: "primarySolidHover" },
    { key: "secondary", label: "secondary" },
    { key: "textPrimary", label: "textPrimary" },
    { key: "textSecondary", label: "textSecondary" },
    { key: "border", label: "border" },
  ];
  const themeOptions = $derived([
    { label: t("personalization.followSystem"), value: "system" },
    { label: t("personalization.light"), value: "light" },
    { label: t("personalization.dark"), value: "dark" },
  ]);
  const materialOptions = $derived<Option[]>(
    platform === "macos"
      ? [
          { label: t("personalization.windowMaterials.solid"), value: "solid" },
          { label: t("personalization.windowMaterials.vibrancy"), value: "vibrancy" },
          ...(liquidGlassSupported
            ? [
                {
                  label: t("personalization.windowMaterials.liquidGlass"),
                  value: "liquid_glass",
                },
              ]
            : []),
        ]
      : platform === "windows"
        ? [
            { label: t("personalization.windowMaterials.solid"), value: "solid" },
            { label: t("personalization.windowMaterials.mica"), value: "mica" },
            { label: t("personalization.windowMaterials.acrylic"), value: "acrylic" },
          ]
        : [{ label: t("personalization.windowMaterials.solid"), value: "solid" }],
  );
  const glassSelects = $derived(
    ["vibrancy", "liquid_glass", "mica", "acrylic"].includes(value.windowMaterial),
  );
  const colorOptions = $derived<Option[]>(
    getThemeOptions().map((option) => ({
      ...option,
      label: t(`personalization.colorThemes.${option.value}`),
    })),
  );
  const customColors = $derived(value.customTheme[activePlan]);
  const fontOptions = $derived([
    { label: t("personalization.systemFont"), value: "" },
    ...(value.fontFamily && !fonts.includes(value.fontFamily)
      ? [{ label: value.fontFamily, value: value.fontFamily, fontFamily: value.fontFamily }]
      : []),
    ...fonts.map((font) => ({ label: font, value: font, fontFamily: font })),
  ]);

  function updateCustomColor(field: keyof ThemeColors, color: string): void {
    onupdate({
      customTheme: {
        ...value.customTheme,
        [activePlan]: { ...value.customTheme[activePlan], [field]: color },
      },
    });
  }

  function syncActivePlan(): void {
    activePlan = document.documentElement.dataset.theme === "dark" ? "dark" : "light";
  }

  function resetCustomPalette(): void {
    onupdate({
      customTheme: {
        ...value.customTheme,
        [activePlan]: { ...DEFAULT_CUSTOM_THEME[activePlan] },
      },
    });
  }

  function customPaletteCss(): string {
    return [
      ":root {",
      "  /* SeaLantern Connect custom theme */",
      ...(["light", "dark"] as const).flatMap((plan) => [
        `  /* ${plan} */`,
        ...customColorFields.map(
          ({ key }) => `  --sl-custom-${plan}-${key}: ${value.customTheme[plan][key]};`,
        ),
      ]),
      "}",
      "",
    ].join("\n");
  }

  async function exportCustomPalette(): Promise<void> {
    const css = customPaletteCss();
    try {
      const saved = await saveTextFile(css, "sl-connect-custom.css");
      if (saved === true) {
        showToast(t("personalization.exportCustomThemeSuccess"), "success");
        return;
      }
      if (saved === null) return;
    } catch {
      showToast(t("personalization.customThemeFileError"), "error");
    }
  }

  async function importCustomPalette(): Promise<void> {
    try {
      const css = await openTextFile();
      if (css !== null) applyImportedPalette(css);
    } catch {
      showToast(t("personalization.customThemeFileError"), "error");
    }
  }

  function applyImportedPalette(css: string): void {
    const expected = new Set(
      (["light", "dark"] as const).flatMap((plan) =>
        customColorFields.map(({ key }) => `${plan}-${key}`),
      ),
    );
    const parsed = new Map<string, string>();
    let hasExtraField = false;
    let hasInvalidValue = false;
    try {
      const stylesheet = new CSSStyleSheet();
      stylesheet.replaceSync(css);
      const visitRules = (rules: CSSRuleList): void => {
        for (const rule of Array.from(rules)) {
          if ("style" in rule && rule.style instanceof CSSStyleDeclaration) {
            for (const property of Array.from(rule.style)) {
              if (!property.startsWith("--sl-custom-")) continue;
              const token = property.slice("--sl-custom-".length);
              if (!expected.has(token)) {
                hasExtraField = true;
                continue;
              }
              const color = rule.style.getPropertyValue(property).trim();
              if (!/^#[\da-f]{6}$/i.test(color)) {
                hasInvalidValue = true;
                continue;
              }
              parsed.set(token, color);
            }
          }
          if ("cssRules" in rule && rule.cssRules) visitRules(rule.cssRules as CSSRuleList);
        }
      };
      visitRules(stylesheet.cssRules);
    } catch {
      showToast(t("personalization.invalidCustomTheme"), "error");
      return;
    }
    const hasMissingField = [...expected].some((key) => !parsed.has(key));
    if (hasMissingField || hasExtraField || hasInvalidValue) {
      showToast(t("personalization.invalidCustomTheme"), "error");
      return;
    }
    const imported = {
      light: { ...value.customTheme.light },
      dark: { ...value.customTheme.dark },
    };
    for (const plan of ["light", "dark"] as const) {
      for (const { key } of customColorFields) {
        imported[plan][key] = parsed.get(`${plan}-${key}`)!;
      }
    }
    onupdate({ customTheme: imported });
    showToast(t("personalization.importCustomThemeSuccess"), "success");
  }

  function chooseBackground(): void {
    backgroundFileInput?.click();
  }

  function handleBackgroundFile(event: Event): void {
    const file = (event.currentTarget as HTMLInputElement).files?.[0];
    if (!file) return;
    if (!file.type.startsWith("image/")) {
      showToast(t("personalization.invalidBackground"), "error");
      return;
    }
    const reader = new FileReader();
    reader.addEventListener(
      "load",
      () => {
        if (typeof reader.result === "string") {
          onupdate({ backgroundImage: reader.result, backgroundEnabled: true });
          showToast(t("personalization.backgroundSelected"), "success");
        }
      },
      { once: true },
    );
    reader.addEventListener(
      "error",
      () => showToast(t("personalization.invalidBackground"), "error"),
      { once: true },
    );
    reader.readAsDataURL(file);
  }

  onMount(async () => {
    platform = /Macintosh|Mac OS X/i.test(navigator.userAgent)
      ? "macos"
      : /Windows/i.test(navigator.userAgent)
        ? "windows"
        : "other";
    try {
      fonts = await getSystemFonts();
    } catch (error) {
      console.error("Failed to load fonts", error);
    }
    if (platform === "macos") {
      try {
        liquidGlassSupported = await supportsLiquidGlass();
      } catch {
        liquidGlassSupported = false;
      }
    }
  });

  onMount(() => {
    syncActivePlan();
    const themeObserver = new MutationObserver(syncActivePlan);
    themeObserver.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["data-theme"],
    });
    return () => themeObserver.disconnect();
  });
</script>

<div class="workspace settings-workspace">
  <section class="settings-section">
    <div class="settings-section-heading"><h2>{t("personalization.themeSection")}</h2></div>
    <div class="preference-row">
      <span>{t("personalization.windowMaterial")}</span><Select
        class="settings-select"
        glass={glassSelects}
        value={value.windowMaterial}
        options={materialOptions}
        onValueChange={(next) =>
          onupdate({ windowMaterial: next as Preferences["windowMaterial"] })}
      />
    </div>
    <div class="preference-row">
      <span>{t("personalization.themeMode")}</span><Select
        class="settings-select"
        glass={glassSelects}
        value={value.theme}
        options={themeOptions}
        onValueChange={(next, origin) => {
          const theme = next as Preferences["theme"];
          if (origin) onthemechange?.(theme, origin);
          else onupdate({ theme });
        }}
      />
    </div>
    <div class="preference-row">
      <span>{t("personalization.colorTheme")}</span><Select
        class="settings-select"
        glass={glassSelects}
        value={value.colorTheme}
        options={colorOptions}
        onValueChange={(next) => onupdate({ colorTheme: next as ColorThemeId })}
      />
    </div>
    <div class="preference-row">
      <span>{t("personalization.fontFamily")}</span><Select
        class="settings-select font-family-select"
        glass={glassSelects}
        value={value.fontFamily}
        options={fontOptions}
        searchable
        searchPlaceholder={t("personalization.searchFont")}
        emptyLabel={t("common.noResults")}
        onValueChange={(next) => onupdate({ fontFamily: next })}
      />
    </div>
    <div class="preference-row">
      <span>{t("personalization.fontSize")}</span>
      <div class="font-size-control">
        <Slider
          id="font-size-slider"
          min={MIN_FONT_SIZE}
          max={MAX_FONT_SIZE}
          value={value.fontSize}
          ariaLabel={t("personalization.fontSize")}
          ariaValueText={`${value.fontSize}px`}
          onvaluechange={(next) => onupdate({ fontSize: next })}
        /><output for="font-size-slider">{value.fontSize}px</output>
      </div>
    </div>
  </section>
  <section class="settings-section background-section">
    <div class="settings-section-heading"><h2>{t("personalization.backgroundSection")}</h2></div>
    <div class="preference-row switch-row">
      <span>{t("personalization.backgroundEnabled")}</span><Toggle
        checked={value.backgroundEnabled}
        label={t("personalization.backgroundEnabled")}
        oncheckedchange={(checked) => onupdate({ backgroundEnabled: checked })}
      />
    </div>
    {#if value.backgroundEnabled}<div class="background-settings">
        <input
          bind:this={backgroundFileInput}
          class="background-file-input"
          type="file"
          accept="image/*"
          onchange={handleBackgroundFile}
        />
        <div class="preference-row background-preview-row">
          <span>{t("personalization.backgroundPreview")}</span>
          <div class="background-picker">
            <div
              class:has-image={Boolean(value.backgroundImage)}
              class="background-preview"
              style={value.backgroundImage
                ? `background-image: url(${JSON.stringify(value.backgroundImage)})`
                : ""}
              role="button"
              tabindex="0"
              aria-label={t("personalization.chooseBackground")}
              onclick={() => !value.backgroundImage && chooseBackground()}
              onkeydown={(event) => {
                if (!value.backgroundImage && (event.key === "Enter" || event.key === " "))
                  chooseBackground();
              }}
            >
              {#if value.backgroundImage}<button
                  class="background-remove-button"
                  type="button"
                  aria-label={t("personalization.clearBackground")}
                  onclick={(event) => {
                    event.stopPropagation();
                    onupdate({ backgroundImage: "" });
                    showToast(t("personalization.backgroundRemoved"), "success");
                  }}><X size={11} /></button
                >{/if}
              {#if !value.backgroundImage}<span>{t("personalization.chooseBackground")}</span>{/if}
            </div>
          </div>
        </div>
        <div class="preference-row">
          <span>{t("personalization.backgroundOpacity")}</span>
          <div class="font-size-control">
            <Slider
              min={0}
              max={1}
              step={0.05}
              value={value.backgroundOpacity}
              ariaLabel={t("personalization.backgroundOpacity")}
              ariaValueText={`${Math.round(value.backgroundOpacity * 100)}%`}
              onvaluechange={(next) => onupdate({ backgroundOpacity: next })}
            /><output>{Math.round(value.backgroundOpacity * 100)}%</output>
          </div>
        </div>
        <div class="preference-row">
          <span>{t("personalization.backgroundBlur")}</span>
          <div class="font-size-control">
            <Slider
              min={0}
              max={20}
              value={value.backgroundBlur}
              ariaLabel={t("personalization.backgroundBlur")}
              ariaValueText={`${value.backgroundBlur}px`}
              onvaluechange={(next) => onupdate({ backgroundBlur: next })}
            /><output>{value.backgroundBlur}px</output>
          </div>
        </div>
        <div class="preference-row">
          <span>{t("personalization.backgroundBrightness")}</span>
          <div class="font-size-control">
            <Slider
              min={0.5}
              max={1.5}
              step={0.1}
              value={value.backgroundBrightness}
              ariaLabel={t("personalization.backgroundBrightness")}
              ariaValueText={value.backgroundBrightness.toFixed(1)}
              onvaluechange={(next) => onupdate({ backgroundBrightness: next })}
            /><output>{value.backgroundBrightness.toFixed(1)}</output>
          </div>
        </div>
        <div class="preference-row">
          <span>{t("personalization.backgroundCardBlur")}</span>
          <div class="font-size-control">
            <Slider
              min={8}
              max={30}
              value={value.backgroundCardBlur}
              ariaLabel={t("personalization.backgroundCardBlur")}
              ariaValueText={`${value.backgroundCardBlur}px`}
              onvaluechange={(next) => onupdate({ backgroundCardBlur: next })}
            /><output>{value.backgroundCardBlur}px</output>
          </div>
        </div>
      </div>{/if}
  </section>
  {#if value.colorTheme === "custom"}<section class="settings-section custom-theme-section">
      <div class="settings-section-heading custom-theme-heading">
        <h2>{t("personalization.customTheme")}</h2>
        <div class="custom-theme-actions">
          <Button
            variant="ghost"
            size="sm"
            title={t("personalization.importCustomTheme")}
            onclick={importCustomPalette}
            ><Upload size={15} />{t("personalization.importCustomTheme")}</Button
          >
          <Button
            variant="ghost"
            size="sm"
            title={t("personalization.exportCustomTheme")}
            onclick={exportCustomPalette}
            ><Download size={15} />{t("personalization.exportCustomTheme")}</Button
          >
          <Button
            variant="ghost"
            size="sm"
            title={t("personalization.resetCustomTheme")}
            onclick={resetCustomPalette}
            ><RotateCcw size={15} />{t("personalization.resetCustomTheme")}</Button
          >
        </div>
      </div>
      <div class="custom-theme-grid">
        {#each customColorFields as field (field.key)}<label class="custom-color-field">
            <span>{t(`personalization.customColors.${field.label}`)}</span>
            <span class="custom-color-control">
              <ColorPicker
                value={customColors[field.key]}
                label={t(`personalization.customColors.${field.label}`)}
                onvaluechange={(color) => updateCustomColor(field.key, color)}
              />
              <code>{customColors[field.key].toUpperCase()}</code>
            </span>
          </label>{/each}
      </div>
    </section>{/if}
</div>
