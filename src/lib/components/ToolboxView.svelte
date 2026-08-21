<script lang="ts">
  import { LoaderCircle, RefreshCw, ShieldAlert, ShieldCheck } from "@lucide/svelte";
  import {
    runNetworkDiagnostics,
    runRelayDiagnostics,
    type NetworkDiagnostics,
    type RelayDiagnostics,
  } from "@api/toolbox";
  import { t } from "@i18n";
  import type { Preferences } from "@models/preferences";
  import Button from "./ui/Button.svelte";
  import Dialog from "./ui/Dialog.svelte";

  let { value } = $props<{ value: Preferences }>();
  let network = $state<NetworkDiagnostics | null>(null);
  let relay = $state<RelayDiagnostics | null>(null);
  let networkChecking = $state(false);
  let relayChecking = $state(false);
  let error = $state("");
  let activeTool = $state<"network" | "relay" | null>(null);
  let dialogOpen = $state(false);
  let relayTimer: number | null = null;
  const directStatus = $derived(
    !network
      ? "unknown"
      : !network.udpAvailable || network.mappingVariesByDestination === true
        ? "limited"
        : network.mappingVariesByDestination === false
          ? "available"
          : "unknown",
  );

  async function checkNetwork(): Promise<void> {
    if (networkChecking) return;
    networkChecking = true;
    error = "";
    try {
      network = await runNetworkDiagnostics();
    } catch (reason) {
      showError(reason);
    } finally {
      networkChecking = false;
    }
  }

  async function checkRelay(): Promise<void> {
    if (relayChecking) return;
    relayChecking = true;
    error = "";
    try {
      relay = await runRelayDiagnostics(value.relayCustom ? value.relayUrl : null);
    } catch (reason) {
      showError(reason);
    } finally {
      relayChecking = false;
    }
  }

  function showError(reason: unknown): void {
    const code = reason instanceof Error ? reason.message : "";
    error = t(
      code === "toolbox_network_timeout"
        ? "toolbox.errors.timeout"
        : code === "toolbox_relay_invalid_url"
          ? "toolbox.errors.invalidRelay"
          : code === "toolbox_network_start_failed"
            ? "toolbox.errors.startFailed"
            : "toolbox.errors.failed",
    );
  }

  function resetTool(): void {
    activeTool = null;
    network = null;
    relay = null;
    error = "";
    if (relayTimer != null) window.clearInterval(relayTimer);
    relayTimer = null;
  }

  function openTool(tool: "network" | "relay"): void {
    resetTool();
    activeTool = tool;
    dialogOpen = true;
    if (tool === "network") void checkNetwork();
    if (tool === "relay") {
      void checkRelay();
      relayTimer = window.setInterval(() => void checkRelay(), 1000);
    }
  }

  $effect(() => {
    if (!dialogOpen && activeTool !== null) resetTool();
  });
</script>

<div class="workspace toolbox-workspace">
  <div class="toolbox-grid">
    <section class="settings-section tool-card network-card">
      <div class="tool-card-heading">
        <div class="tool-card-title"><h2>{t("toolbox.networkTitle")}</h2></div>
        <Button class="tool-action" onclick={() => openTool("network")}
          ><RefreshCw size={15} />{t("toolbox.check")}</Button
        >
      </div>
      <p class="tool-card-description">{t("toolbox.networkDescription")}</p>
    </section>
    <section class="settings-section tool-card">
      <div class="tool-card-heading">
        <div class="tool-card-title"><h2>{t("toolbox.relayTitle")}</h2></div>
        <Button class="tool-action" onclick={() => openTool("relay")}
          ><RefreshCw size={15} />{t("toolbox.check")}</Button
        >
      </div>
      <p class="tool-card-description">
        {value.relayCustom ? t("toolbox.customRelayHint") : t("toolbox.defaultRelayHint")}
      </p>
    </section>
  </div>
  {#if error}<p class="field-error toolbox-error">{error}</p>{/if}
</div>

<Dialog bind:open={dialogOpen} title={t(`toolbox.${activeTool ?? "network"}Title`)} width="480px">
  {#if activeTool === "network"}
    <div class="tool-dialog-content tool-network-dialog">
      <div
        class="toolbox-status"
        class:available={network && directStatus === "available"}
        class:limited={network && directStatus === "limited"}
      >
        {#if network}{#if directStatus === "available"}<ShieldCheck size={19} />{:else}<ShieldAlert
              size={19}
            />{/if}{:else}<LoaderCircle class="spin" size={19} />{/if}
        <div>
          <strong
            >{network ? t(`toolbox.direct.${directStatus}.title`) : t("toolbox.checking")}</strong
          >
          {#if network}<p>{t(`toolbox.direct.${directStatus}.hint`)}</p>{/if}
        </div>
      </div>
      <div class="toolbox-mini-results">
        <span
          >{t("toolbox.publicIpv4")}{#if network}<strong
              >{network.publicIpv4 ?? t("toolbox.unavailable")}</strong
            >{:else}<LoaderCircle class="spin" size={16} />{/if}</span
        >
        <span
          >{t("toolbox.publicIpv6")}{#if network}<strong
              >{network.publicIpv6 ?? t("toolbox.unavailable")}</strong
            >{:else}<LoaderCircle class="spin" size={16} />{/if}</span
        >
        <span
          >{t("toolbox.udp")}{#if network}<strong
              >{network.udpAvailable ? t("toolbox.available") : t("toolbox.unavailable")}</strong
            >{:else}<LoaderCircle class="spin" size={16} />{/if}</span
        >
        <span
          >{t("toolbox.natMapping")}{#if network}<strong
              >{network.mappingVariesByDestination === null
                ? t("toolbox.unknown")
                : network.mappingVariesByDestination
                  ? t("toolbox.varies")
                  : t("toolbox.stable")}</strong
            >{:else}<LoaderCircle class="spin" size={16} />{/if}</span
        >
        <span
          >{t("toolbox.relay")}{#if network}<strong
              >{network.relayAvailable ? t("toolbox.available") : t("toolbox.unavailable")}</strong
            >{:else}<LoaderCircle class="spin" size={16} />{/if}</span
        >
      </div>
    </div>
  {:else if activeTool === "relay"}
    <div class="tool-dialog-content tool-relay-dialog">
      <div class="tool-value-list">
        <span
          >{t("toolbox.relayAddress")}{#if relay}<strong
              >{relay.relayUrl ?? t("toolbox.unavailable")}</strong
            >{:else}<LoaderCircle class="spin" size={16} />{/if}</span
        >
        <span
          >{t("toolbox.latency")}{#if relay}<strong
              >{relay.latencyMs == null
                ? t("toolbox.unavailable")
                : `${relay.latencyMs} ms`}</strong
            >{:else}<LoaderCircle class="spin" size={16} />{/if}</span
        >
      </div>
    </div>
  {/if}
</Dialog>

<style>
  .toolbox-workspace {
    width: min(760px, calc(100% - 48px));
    min-height: 100%;
    padding-bottom: 32px;
    align-content: start;
  }
  :global(.tool-action) {
    min-width: 158px;
  }
  .toolbox-grid {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 16px;
  }
  .tool-card {
    position: relative;
    display: grid;
    align-content: start;
    gap: 13px;
    min-width: 0;
    height: 156px;
    box-sizing: border-box;
    padding: 20px;
    border: 1px solid var(--border);
    border-radius: var(--sl-radius-lg);
    background: var(--surface-soft);
    box-shadow: var(--card-shadow);
  }
  .tool-card-heading {
    display: block;
  }
  .tool-card-heading :global(.tool-action) {
    position: absolute;
    right: 20px;
    bottom: 20px;
  }
  .tool-card-title {
    display: flex;
    align-items: center;
    gap: 9px;
    min-width: 0;
  }
  .tool-card-title h2 {
    margin: 0;
    font-size: 1rem;
  }
  .tool-card-description {
    min-height: 42px;
    margin: 0;
    color: var(--muted);
    line-height: 1.55;
  }
  .toolbox-status {
    min-height: 80px;
    box-sizing: border-box;
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 11px;
    border: 1px solid var(--border);
    border-radius: var(--sl-radius-sm);
    color: var(--muted);
  }
  .toolbox-status.available {
    color: var(--success);
  }
  .toolbox-status.limited {
    color: var(--warning);
  }
  .toolbox-status strong {
    color: var(--text);
  }
  .toolbox-status p {
    margin: 4px 0 0;
    color: var(--muted);
    font-size: 0.8571rem;
    line-height: 1.45;
  }
  .toolbox-mini-results,
  .tool-value-list {
    display: grid;
    gap: 7px;
  }
  .toolbox-mini-results {
    margin-top: 10px;
  }
  .toolbox-mini-results span,
  .tool-value-list span {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    color: var(--muted);
    font-size: 0.8571rem;
  }
  .toolbox-mini-results strong,
  .tool-value-list strong {
    color: var(--text);
    font-family: inherit;
    font-weight: 500;
    text-align: right;
    overflow-wrap: anywhere;
  }
  .toolbox-error {
    margin: 0;
  }
  .tool-dialog-content {
    display: grid;
    align-content: start;
    gap: 16px;
  }
  .tool-network-dialog {
    min-height: 292px;
  }
  .tool-relay-dialog {
    min-height: 86px;
  }
  :global(.tool-dialog-action) {
    justify-self: end;
  }
  @media (max-width: 760px) {
    .toolbox-workspace {
      width: calc(100% - 28px);
    }
    .toolbox-grid {
      grid-template-columns: 1fr;
    }
  }
</style>
