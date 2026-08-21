<script lang="ts">
  import { ArrowRight, Check, Copy, Radio, RotateCcw, Unplug } from "@lucide/svelte";
  import { saveJoinPort, startJoin, stopJoin, stopTunnel, validateInvite } from "@api/p2p";
  import { backendMessage, p2pErrorMessage, t } from "@i18n";
  import {
    isSameInvite,
    normalizeInvite,
    toWebInvite,
    type IncomingInvite,
  } from "@domain/invitations";
  import type { P2pStatus } from "@models/p2p";
  import { consumeInvite } from "../state";
  import Button from "./ui/Button.svelte";
  import Dialog from "./ui/Dialog.svelte";
  import Input from "./ui/Input.svelte";
  import Select, { type Option } from "./ui/Select.svelte";
  import LatencyChart from "./shared/LatencyChart.svelte";

  let { status, savedInvite, savedPort, request } = $props<{
    status: P2pStatus;
    savedInvite: string;
    savedPort: number;
    request: IncomingInvite | null;
  }>();
  let invite = $state("");
  let inviteCleared = $state(false);
  let validationError = $state("");
  let commandError = $state("");
  let confirming = $state(false);
  let portMode = $state("auto");
  let manualPort = $state("");
  let copied = $state(false);
  let stopPending = $state(false);
  let handledRequestId = $state<number | null>(null);
  const portOptions = $derived<Option[]>([
    { label: t("join.automatic"), value: "auto" },
    { label: t("join.manual"), value: "manual" },
  ]);
  let joining = $derived(status.mode === "join" && status.phase !== "idle");
  let occupied = $derived(status.phase !== "idle" && status.mode !== "join");
  let busy = $derived(joining && (status.phase === "starting" || status.phase === "stopping"));
  let connected = $derived(joining && status.phase === "active");
  let stopDisabled = $derived(status.phase === "stopping" || stopPending);
  let replacingConnection = $derived(status.phase !== "idle");
  let canJoin = $derived(invite.trim().length > 0 && status.phase === "idle");
  let validManualPort = $derived(
    Number.isInteger(Number(manualPort)) && Number(manualPort) >= 1 && Number(manualPort) <= 65535,
  );
  let phaseLabel = $derived(
    status.phase === "starting"
      ? t("join.starting")
      : status.phase === "active"
        ? t("join.active")
        : status.phase === "stopping"
          ? t("join.stopping")
          : t("join.idle"),
  );
  let stopLabel = $derived(
    status.phase === "starting"
      ? t("join.cancelConnection")
      : status.phase === "stopping"
        ? t("join.cancelling")
        : t("join.disconnect"),
  );

  $effect(() => {
    if (!invite && !inviteCleared) invite = toWebInvite(savedInvite);
  });
  $effect(() => {
    if (Number(manualPort) !== savedPort) manualPort = String(savedPort);
  });
  $effect(() => {
    const currentRequest = request;
    if (!currentRequest || currentRequest.id === handledRequestId) return;
    handledRequestId = currentRequest.id;
    void importIncomingInvite(currentRequest.uri).finally(() => consumeInvite(currentRequest.id));
  });

  function setPortMode(value: string): void {
    if (value === "auto" || value === "manual") portMode = value;
  }
  function resetInvite(): void {
    inviteCleared = true;
    invite = "";
    validationError = "";
    commandError = "";
  }
  function rejectOwnInvite(uri: string): boolean {
    const isOwnInvite =
      status.mode === "host" && status.phase !== "idle" && isSameInvite(status.shareUri, uri);
    if (!isOwnInvite) return false;
    confirming = false;
    commandError = t("join.ownInvite");
    return true;
  }
  async function submitInvite(): Promise<void> {
    validationError = "";
    commandError = "";
    try {
      const normalized = normalizeInvite(invite);
      await validateInvite(normalized);
      invite = toWebInvite(normalized);
      await join();
    } catch {
      validationError = t("join.invalidInvite");
    }
  }
  async function importIncomingInvite(uri: string): Promise<void> {
    const normalized = normalizeInvite(uri);
    validationError = "";
    commandError = "";
    invite = toWebInvite(normalized);
    if (rejectOwnInvite(normalized)) return;
    if (joining && isSameInvite(status.shareUri, normalized)) return;
    confirming = true;
    try {
      await validateInvite(normalized);
    } catch {
      confirming = false;
      if (status.phase === "idle") validationError = t("join.invalidInvite");
      else commandError = t("join.invalidInvite");
    }
  }
  function waitForIdle(timeoutMs = 20_000): Promise<void> {
    if (status.phase === "idle") return Promise.resolve();
    return new Promise((resolve, reject) => {
      const checkId = window.setInterval(() => {
        if (status.phase === "idle") finish();
      }, 100);
      const timeoutId = window.setTimeout(
        () => finish(new Error("timed out while stopping the current connection")),
        timeoutMs,
      );
      function finish(error?: Error): void {
        window.clearInterval(checkId);
        window.clearTimeout(timeoutId);
        if (error) reject(error);
        else resolve();
      }
    });
  }
  async function join(): Promise<void> {
    if (portMode === "manual" && !validManualPort) return;
    if (rejectOwnInvite(invite)) return;
    confirming = false;
    commandError = "";
    try {
      if (status.phase !== "idle") {
        await stopTunnel();
        await waitForIdle();
      }
      await startJoin(normalizeInvite(invite), portMode === "auto" ? null : Number(manualPort));
    } catch (error) {
      commandError = backendMessage(error);
    }
  }
  async function savePort(): Promise<void> {
    if (!validManualPort) return;
    try {
      await saveJoinPort(Number(manualPort));
    } catch (error) {
      console.error("Failed to save join port", error);
    }
  }
  async function stop(): Promise<void> {
    if (stopDisabled) return;
    commandError = "";
    stopPending = true;
    try {
      await stopJoin();
    } catch (error) {
      commandError = backendMessage(error);
    } finally {
      stopPending = false;
    }
  }
  async function copyAddress(): Promise<void> {
    if (!status.localAddress) return;
    await navigator.clipboard.writeText(status.localAddress);
    copied = true;
    window.setTimeout(() => (copied = false), 1600);
  }
  function formatBytes(value: number): string {
    return value < 1024
      ? `${value} B`
      : value < 1024 ** 2
        ? `${(value / 1024).toFixed(1)} KB`
        : `${(value / 1024 ** 2).toFixed(1)} MB`;
  }
</script>

<div class="workspace">
  <section class="intro">
    <div>
      <h1>{phaseLabel}</h1>
      <p>{connected ? t("join.activeHint") : t("join.idleHint")}</p>
    </div>
    <span class:active={connected} class="phase-pill"
      >{connected
        ? t("join.connected")
        : busy
          ? t("join.processing")
          : t("join.disconnected")}</span
    >
  </section>
  {#if !joining}
    <section class="join-panel">
      <label for="invite">{t("join.invite")}</label>
      <div class="invite-field">
        <Input
          id="invite"
          class={validationError ? "invalid" : ""}
          bind:value={invite}
          placeholder="https://ideaflash.cn/#v1/..."
          onkeydown={(event) => {
            if (event.key === "Enter" && canJoin) void submitInvite();
          }}
        /><Button
          variant="ghost"
          size="sm"
          title={t("join.clearInput")}
          disabled={!invite}
          onclick={resetInvite}><RotateCcw size={16} /></Button
        >
      </div>
      {#if validationError}<p class="field-error">{validationError}</p>{/if}
      <div class="join-actions">
        <div class="privacy-note">{t("join.privacy")}</div>
        <Button class="primary-button" disabled={!canJoin} onclick={submitInvite}
          >{t("join.continue")}<ArrowRight size={17} /></Button
        >
      </div>
    </section>
  {:else}
    <section class="connection-panel">
      <div class="address-block">
        <span>{t("join.minecraftAddress")}</span><strong
          >{status.localAddress ?? t("join.allocatingPort")}</strong
        ><Button
          class="copy-button"
          variant="outline"
          disabled={!status.localAddress}
          onclick={copyAddress}
          >{#if copied}<Check size={16} />{t("join.copied")}{:else}<Copy size={16} />{t(
              "join.copyAddress",
            )}{/if}</Button
        >
      </div>
      <div class="metrics">
        <div>
          <span>{t("join.route")}</span><strong
            >{status.route === "direct"
              ? t("join.direct")
              : status.route === "relay"
                ? t("join.relay")
                : t("join.detecting")}</strong
          >
        </div>
        <div>
          <span>{t("join.latency")}</span><strong
            >{status.rttMs == null ? "--" : `${status.rttMs} ms`}</strong
          >
        </div>
        <div><span>{t("join.sent")}</span><strong>{formatBytes(status.txBytes)}</strong></div>
        <div><span>{t("join.received")}</span><strong>{formatBytes(status.rxBytes)}</strong></div>
      </div>
      {#if connected}<LatencyChart rttMs={status.rttMs} />{/if}
      <div class="connection-footer">
        <p>{status.message ? backendMessage(status.message) : t("join.syncing")}</p>
        <Button variant="danger" disabled={stopDisabled} loading={stopPending} onclick={stop}
          >{#if !stopPending}<Unplug size={16} />{/if}{stopLabel}</Button
        >
      </div>
    </section>
  {/if}
  {#if commandError || status.error || occupied}<p class="error-banner">
      {commandError ||
        (occupied
          ? t("join.occupied")
          : status.message
            ? backendMessage(status.message)
            : status.error
              ? p2pErrorMessage(status.error)
              : t("join.retryHint"))}
    </p>{/if}
</div>

<Dialog bind:open={confirming} title={t("join.confirmTitle")} width="560px">
  <p class="modal-copy">{replacingConnection ? t("join.replaceHint") : t("join.confirmHint")}</p>
  <div class="invite-summary">
    <span>{t("join.inviteProtocol")}</span><strong>sculk / v1</strong>
  </div>
  <div class="join-port-setting">
    <div class="join-port-heading">
      <span>{t("join.localPort")}</span><Select
        class="mode-tabs compact"
        value={portMode}
        options={portOptions}
        onValueChange={setPortMode}
      />
    </div>
    {#if portMode === "auto"}<div class="join-port-detail">
        {t("join.automaticPort")}
      </div>{:else}<Input
        class={`join-port-input ${validManualPort ? "" : "invalid"}`}
        bind:value={manualPort}
        type="number"
        min={1}
        max={65535}
        aria-label={t("join.localPortNumber")}
        onchange={savePort}
        hideNumberControls
      />{/if}
  </div>
  {#snippet footer()}<Button variant="outline" onclick={() => (confirming = false)}
      >{t("join.cancel")}</Button
    ><Button
      class="primary-button"
      disabled={portMode === "manual" && !validManualPort}
      onclick={join}
      ><Radio size={17} />{replacingConnection
        ? t("join.confirmReplace")
        : t("join.confirm")}</Button
    >{/snippet}
</Dialog>

<style>
  :global(.invite-field) {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    gap: 8px;
    min-height: 38px;
    padding: 0 4px 0 0;
    color: var(--muted);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--sl-radius-md);
  }
  :global(.invite-field:focus-within) {
    border-color: var(--primary);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--primary) 14%, transparent);
  }
  :global(.invite-field .ui-input) {
    min-width: 0;
    border: 0;
    box-shadow: none;
  }
  :global(.invite-field .ui-button) {
    width: 30px;
    padding: 0;
  }
</style>
