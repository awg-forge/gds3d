<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { disableAutostart, enableAutostart, getAutostartEnabled } from "@api/autostart";
  import { t } from "@i18n";
  import type { Preferences } from "@models/preferences";
  import { showToast, updateApplication, updateConnection, updateLightweight } from "../state";
  import Input from "./ui/Input.svelte";
  import Select, { type Option } from "./ui/Select.svelte";
  import Toggle from "./ui/Toggle.svelte";

  let { value } = $props<{ value: Preferences }>();
  let autostartEnabled = $state(false);
  let autostartLoading = $state(true);
  let autostartUpdating = $state(false);
  let autoLightweight = $state(false);
  let autoLightweightMinutes = $state("3");
  let reconnectUnlimited = $state(true);
  let relayCustom = $state(untrack(() => value.relayCustom));
  let relayUrl = $state(untrack(() => value.relayUrl));
  const splashOptions = $derived<Option[]>([
    { label: t("connectionSettings.disabled"), value: 0 },
    ...[500, 1000, 1500, 2000].map((durationMs) => ({
      label: t("connectionSettings.seconds", { value: durationMs / 1000 }),
      value: durationMs,
    })),
  ]);
  const relayOptions = $derived<Option[]>([
    { label: t("connectionSettings.defaultRelay"), value: "default" },
    { label: t("connectionSettings.customRelay"), value: "custom" },
  ]);
  const reconnectOptions = $derived<Option[]>([
    { label: t("connectionSettings.unlimited"), value: "unlimited" },
    { label: t("connectionSettings.limited"), value: "limited" },
  ]);
  const timeoutOptions = $derived<Option[]>(
    [10, 15, 20, 30, 60].map((timeoutSeconds) => ({
      label: t("connectionSettings.seconds", { value: timeoutSeconds }),
      value: timeoutSeconds,
    })),
  );

  $effect(() => {
    if (value.autoLightweightMinutes != null) {
      autoLightweight = true;
      autoLightweightMinutes = String(value.autoLightweightMinutes);
    }
    reconnectUnlimited = value.reconnectTimeoutSecs == null;
  });

  onMount(async () => {
    try {
      autostartEnabled = await getAutostartEnabled();
    } catch (error) {
      console.error("Failed to load autostart", error);
    } finally {
      autostartLoading = false;
    }
  });
  async function updateAutostart(next: boolean): Promise<void> {
    if (autostartLoading || autostartUpdating) return;
    const previous = autostartEnabled;
    autostartEnabled = next;
    autostartUpdating = true;
    try {
      if (next) await enableAutostart();
      else await disableAutostart();
    } catch (error) {
      autostartEnabled = previous;
      console.error("Failed to update autostart", error);
      showToast(t("connectionSettings.autostartError"), "error");
    } finally {
      autostartUpdating = false;
    }
  }
  function persistConnection(
    update: Partial<Pick<Preferences, "relayCustom" | "relayUrl" | "reconnectTimeoutSecs">> = {},
  ): void {
    const next = {
      relayCustom,
      relayUrl,
      reconnectTimeoutSecs: reconnectUnlimited ? null : (value.reconnectTimeoutSecs ?? 30),
      ...update,
    };
    if (next.relayCustom && !isValidRelayUrl(next.relayUrl)) return;
    updateConnection(next);
  }
  function isValidRelayUrl(relayEndpoint: string): boolean {
    try {
      const url = new URL(relayEndpoint.trim());
      return url.protocol === "http:" || url.protocol === "https:";
    } catch {
      return false;
    }
  }
  function saveLightweight(): void {
    const minutes = Number(autoLightweightMinutes);
    if (!autoLightweight) {
      updateLightweight({ autoLightweightMinutes: null });
      return;
    }
    if (!isValidLightweightMinutes()) return;
    updateLightweight({ autoLightweightMinutes: minutes });
  }
  function isValidLightweightMinutes(): boolean {
    const minutes = Number(autoLightweightMinutes);
    return Number.isInteger(minutes) && minutes >= 1 && minutes <= 1440;
  }
</script>

<div class="workspace settings-workspace">
  <section class="settings-section">
    <div class="settings-section-heading"><h2>{t("connectionSettings.startup")}</h2></div>
    <div class="preference-row switch-row">
      <span>{t("connectionSettings.autostart")}</span><Toggle
        label={t("connectionSettings.autostart")}
        checked={autostartEnabled}
        disabled={autostartLoading || autostartUpdating}
        oncheckedchange={updateAutostart}
      />
    </div>
    {#if autostartEnabled}<div class="preference-row switch-row">
        <span>{t("connectionSettings.silentStart")}</span><Toggle
          label={t("connectionSettings.silentStart")}
          checked={value.silentStart}
          oncheckedchange={(checked) =>
            updateApplication({
              splashDurationMs: value.splashDurationMs,
              silentStart: checked,
              autoUpdate: value.autoUpdate,
              rememberWindowState: value.rememberWindowState,
            })}
        />
      </div>{/if}
    <div class="preference-row switch-row">
      <span>{t("connectionSettings.autoUpdate")}</span><Toggle
        label={t("connectionSettings.autoUpdate")}
        checked={value.autoUpdate}
        oncheckedchange={(checked) =>
          updateApplication({
            splashDurationMs: value.splashDurationMs,
            silentStart: value.silentStart,
            autoUpdate: checked,
            rememberWindowState: value.rememberWindowState,
          })}
      />
    </div>
    <div class="preference-row">
      <span>{t("connectionSettings.splashDuration")}</span><Select
        class="settings-select"
        value={value.splashDurationMs}
        options={splashOptions}
        onValueChange={(next) =>
          updateApplication({
            splashDurationMs: Number(next) as Preferences["splashDurationMs"],
            silentStart: value.silentStart,
            autoUpdate: value.autoUpdate,
            rememberWindowState: value.rememberWindowState,
          })}
      />
    </div>
  </section>

  <section class="settings-section">
    <div class="settings-section-heading"><h2>{t("connectionSettings.windowBehavior")}</h2></div>
    <div class="preference-row switch-row">
      <span>{t("connectionSettings.rememberWindowState")}</span><Toggle
        label={t("connectionSettings.rememberWindowState")}
        checked={value.rememberWindowState}
        oncheckedchange={(checked) =>
          updateApplication({
            splashDurationMs: value.splashDurationMs,
            silentStart: value.silentStart,
            autoUpdate: value.autoUpdate,
            rememberWindowState: checked,
          })}
      />
    </div>
  </section>

  <section class="settings-section">
    <div class="settings-section-heading">
      <h2>{t("connectionSettings.lightweightSection")}</h2>
    </div>
    <div class="preference-row switch-row">
      <span>{t("connectionSettings.autoLightweight")}</span><Toggle
        label={t("connectionSettings.autoLightweight")}
        checked={autoLightweight}
        oncheckedchange={(checked) => {
          autoLightweight = checked;
          if (checked && !isValidLightweightMinutes()) autoLightweightMinutes = "3";
          saveLightweight();
        }}
      />
    </div>
    {#if autoLightweight}<label class="preference-row settings-input-row"
        ><span>{t("connectionSettings.lightweightDelay")}</span>
        <div class="settings-input settings-number-input sl-input-wrapper">
          <Input
            bind:value={autoLightweightMinutes}
            type="number"
            min={1}
            max={1440}
            onchange={saveLightweight}
            hideNumberControls
          /><span>{t("connectionSettings.minutes")}</span>
        </div></label
      >{#if !isValidLightweightMinutes()}<p class="field-error relay-error">
          {t("connectionSettings.invalidLightweightDelay")}
        </p>{/if}{/if}
  </section>

  <section class="settings-section">
    <div class="settings-section-heading"><h2>{t("connectionSettings.relaySection")}</h2></div>
    <div class="preference-row">
      <span>{t("connectionSettings.relayNode")}</span><Select
        class="settings-select"
        value={relayCustom ? "custom" : "default"}
        options={relayOptions}
        onValueChange={(next) => {
          relayCustom = next === "custom";
          persistConnection();
        }}
      />
    </div>
    {#if relayCustom}<label class="preference-row settings-input-row"
        ><span>{t("connectionSettings.customRelayUrl")}</span><Input
          class="settings-input"
          bind:value={relayUrl}
          type="url"
          placeholder={t("connectionSettings.relayPlaceholder")}
          onchange={() => persistConnection()}
        /></label
      >{#if !isValidRelayUrl(relayUrl)}<p class="field-error relay-error">
          {t("connectionSettings.invalidRelay")}
        </p>{/if}{/if}
  </section>

  <section class="settings-section">
    <div class="settings-section-heading"><h2>{t("connectionSettings.reconnectSection")}</h2></div>
    <div class="preference-row">
      <span>{t("connectionSettings.reconnectPolicy")}</span><Select
        class="settings-select"
        value={reconnectUnlimited ? "unlimited" : "limited"}
        options={reconnectOptions}
        onValueChange={(next) => {
          reconnectUnlimited = next === "unlimited";
          persistConnection();
        }}
      />
    </div>
    {#if !reconnectUnlimited}<label class="preference-row settings-input-row"
        ><span>{t("connectionSettings.timeout")}</span><Select
          class="settings-select"
          value={value.reconnectTimeoutSecs ?? 30}
          options={timeoutOptions}
          onValueChange={(next) => persistConnection({ reconnectTimeoutSecs: Number(next) })}
        /></label
      >{/if}
  </section>
</div>

<style>
  .settings-number-input {
    position: relative;
  }
  .settings-number-input :global(.ui-input) {
    padding-right: 52px;
  }
  .settings-number-input > span {
    position: absolute;
    top: 50%;
    right: 12px;
    color: var(--muted);
    font-size: 0.8571rem;
    pointer-events: none;
    transform: translateY(-50%);
  }
</style>
