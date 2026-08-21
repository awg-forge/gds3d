<script lang="ts">
  import { onMount } from "svelte";
  import { Check, CircleAlert, Copy, HousePlus, LoaderCircle, Square } from "@lucide/svelte";
  import {
    getLanScan,
    probeHostPort,
    startHost,
    startLanScan,
    stopLanScan,
    stopTunnel,
  } from "@api/p2p";
  import { backendMessage, t } from "@i18n";
  import type { HostUriLifetime } from "@models/preferences";
  import type { P2pStatus } from "@models/p2p";
  import Button from "./ui/Button.svelte";
  import Input from "./ui/Input.svelte";
  import Select, { type Option } from "./ui/Select.svelte";

  let { status, uriLifetime, onLifetime } = $props<{
    status: P2pStatus;
    uriLifetime: HostUriLifetime;
    onLifetime: (value: HostUriLifetime) => void;
  }>();
  let portMode = $state("auto");
  let manualPort = $state("25565");
  let maxPlayers = $state("");
  let scan = $state({ scanning: false, port: null as number | null });
  let scanError = $state("");
  let commandError = $state("");
  let pending = $state(false);
  let copied = $state(false);
  let scanTimer: number | null = null;
  let monitorTimer: number | null = null;
  let previousPhase = $state<P2pStatus["phase"] | null>(null);
  const portOptions = $derived<Option[]>([
    { label: t("create.automaticDiscovery"), value: "auto" },
    { label: t("create.manual"), value: "manual" },
  ]);
  const lifetimeOptions = $derived<Option[]>(
    ["always", "never", "1h", "3h", "6h", "12h", "24h"].map((value) => ({
      label: t(
        `create.lifetime${value === "always" ? "Always" : value === "never" ? "Never" : value}`,
      ),
      value,
    })),
  );
  let hosting = $derived(status.mode === "host" && status.phase !== "idle");
  let occupied = $derived(status.phase !== "idle" && status.mode !== "host");
  let selectedPort = $derived(portMode === "auto" ? scan.port : Number(manualPort));
  let validPort = $derived(
    Number.isInteger(selectedPort) &&
      selectedPort != null &&
      selectedPort >= 1 &&
      selectedPort <= 65535,
  );
  let validMaxPlayers = $derived(
    maxPlayers.trim() === "" ||
      (Number.isInteger(Number(maxPlayers)) &&
        Number(maxPlayers) >= 1 &&
        Number(maxPlayers) <= 1000),
  );
  let canCreate = $derived(!pending && !occupied && validPort && validMaxPlayers);

  onMount(() => {
    if (status.phase === "idle") void beginScan();
    return () => {
      stopPolling();
      stopMonitoring();
      void stopLanScan().catch((error) => console.error("Failed to stop LAN scan", error));
    };
  });

  $effect(() => {
    const phase = status.phase;
    if (previousPhase === null) {
      previousPhase = phase;
      return;
    }
    if (phase === previousPhase) return;

    const wasIdle = previousPhase === "idle";
    previousPhase = phase;
    if (phase === "idle" && !wasIdle && portMode === "auto") {
      void beginScan(true);
    } else if (phase !== "idle") {
      void stopScan();
    }
  });

  function stopPolling(): void {
    if (scanTimer != null) {
      window.clearInterval(scanTimer);
      scanTimer = null;
    }
  }
  function stopMonitoring(): void {
    if (monitorTimer != null) {
      window.clearInterval(monitorTimer);
      monitorTimer = null;
    }
  }
  async function beginScan(restart = false): Promise<void> {
    stopPolling();
    stopMonitoring();
    scanError = "";
    try {
      scan = await startLanScan(restart);
      scanTimer = window.setInterval(pollScan, 800);
    } catch (error) {
      scanError = backendMessage(error);
    }
  }
  async function pollScan(): Promise<void> {
    try {
      scan = await getLanScan();
      if (scan.port != null || !scan.scanning) {
        stopPolling();
        if (scan.port != null) startMonitoring();
      }
    } catch (error) {
      scanError = backendMessage(error);
      stopPolling();
    }
  }
  function startMonitoring(): void {
    if (monitorTimer != null) return;
    monitorTimer = window.setInterval(async () => {
      if (scan.port == null) return;
      try {
        if (!(await probeHostPort(scan.port))) void beginScan(true);
      } catch (error) {
        scanError = backendMessage(error);
      }
    }, 5000);
  }
  async function stopScan(): Promise<void> {
    stopPolling();
    stopMonitoring();
    scan = { scanning: false, port: null };
    try {
      await stopLanScan();
    } catch (error) {
      scanError = backendMessage(error);
    }
  }
  function setPortMode(value: string): void {
    if (value !== "auto" && value !== "manual") return;

    portMode = value;
    if (value === "auto") void beginScan(true);
    else void stopScan();
  }
  async function createRoom(): Promise<void> {
    if (!canCreate || selectedPort == null) return;
    pending = true;
    commandError = "";
    try {
      await startHost(selectedPort, maxPlayers.trim() ? Number(maxPlayers) : null, uriLifetime);
    } catch (error) {
      commandError = backendMessage(error);
    } finally {
      pending = false;
    }
  }
  async function stopRoom(): Promise<void> {
    pending = true;
    try {
      await stopTunnel();
    } catch (error) {
      commandError = backendMessage(error);
    } finally {
      pending = false;
    }
  }
  async function copyInvite(): Promise<void> {
    if (!status.shareUri) return;
    await navigator.clipboard.writeText(status.shareUri);
    copied = true;
    window.setTimeout(() => (copied = false), 1600);
  }
</script>

<div class="workspace create-workspace">
  <section class="intro">
    <div>
      <h1>{hosting ? t("create.running") : t("create.title")}</h1>
      <p>{hosting ? t("create.runningHint") : t("create.idleHint")}</p>
    </div>
    <span class:active={hosting && status.phase === "active"} class="phase-pill"
      >{hosting
        ? status.phase === "active"
          ? t("create.created")
          : t("create.processing")
        : t("create.notCreated")}</span
    >
  </section>
  {#if hosting}
    <section class="connection-panel host-panel">
      <div class="share-block">
        <span>{t("create.invite")}</span><strong
          >{status.shareUri ?? t("create.generatingInvite")}</strong
        ><Button
          variant="outline"
          class="copy-button"
          disabled={!status.shareUri}
          onclick={copyInvite}
          >{#if copied}<Check size={16} />{t("create.copied")}{:else}<Copy size={16} />{t(
              "create.copyInvite",
            )}{/if}</Button
        >
      </div>
      <div class="host-summary">
        <div><span>{t("create.players")}</span><strong>{status.playerCount}</strong></div>
        <div><span>{t("create.targetPort")}</span><strong>{status.hostPort ?? "--"}</strong></div>
      </div>
      {#if status.hostPeers.length}<section class="host-peer-section">
          <div class="host-peer-heading">
            <span>{t("create.playerConnections")}</span><strong>{status.hostPeers.length}</strong>
          </div>
          {#each status.hostPeers as peer (peer.id)}<div class="host-peer-row">
              <code
                >{peer.id.length > 18
                  ? `${peer.id.slice(0, 10)}...${peer.id.slice(-6)}`
                  : peer.id}</code
              ><span
                >{peer.route === "direct"
                  ? t("join.direct")
                  : peer.route === "relay"
                    ? t("join.relay")
                    : t("join.detecting")}</span
              ><strong>{peer.rttMs == null ? "--" : `${peer.rttMs} ms`}</strong>
            </div>{/each}
        </section>{/if}
      <div class="connection-footer">
        <p>{status.message ? backendMessage(status.message) : t("create.ready")}</p>
        <Button variant="danger" disabled={pending} loading={pending} onclick={stopRoom}
          >{#if !pending}<Square size={15} />{/if}{t("create.stop")}</Button
        >
      </div>
    </section>
  {:else}
    <section class="create-panel">
      <div class="form-field">
        <span class="field-label">{t("create.minecraftPort")}</span><Select
          class="settings-select"
          value={portMode}
          options={portOptions}
          onValueChange={setPortMode}
        />
      </div>
      <div class="form-field port-field">
        <label for="host-port" class="field-label">{t("create.port")}</label>
        <div
          class:detecting={portMode === "auto" && scan.port == null && !scanError}
          class="port-input-wrap"
        >
          <Input
            id="host-port"
            class={`room-input ${portMode === "auto" && scanError ? "invalid" : ""}`}
            value={portMode === "auto" ? (scan.port?.toString() ?? "") : manualPort}
            type={portMode === "auto" ? "text" : "number"}
            min={1}
            max={65535}
            disabled={portMode === "auto"}
            placeholder={portMode === "auto"
              ? scanError
                ? t("create.discoveryFailed")
                : t("create.detectingPort")
              : ""}
            hideNumberControls
            onchange={(event) => {
              if (portMode === "manual") manualPort = event.currentTarget.value;
            }}
          />
          {#if portMode === "auto" && scan.port == null}<span
              class:error={!!scanError}
              class="port-input-status"
              >{#if scanError}<CircleAlert size={18} />{:else}<LoaderCircle
                  class="spin"
                  size={18}
                />{/if}</span
            >{/if}
        </div>
      </div>
      <div class="form-field settings-field">
        <label for="max-players" class="field-label">{t("create.maxPlayers")}</label><Input
          id="max-players"
          class="room-input"
          bind:value={maxPlayers}
          type="number"
          min={1}
          max={1000}
          placeholder={t("create.unlimited")}
          hideNumberControls
        />
      </div>
      <div class="form-field">
        <span class="field-label">{t("create.uriLifetime")}</span><Select
          class="settings-select"
          value={uriLifetime}
          options={lifetimeOptions}
          onValueChange={(value) => onLifetime(value as HostUriLifetime)}
        />
      </div>
      <div class="create-actions">
        {#if commandError || occupied || status.message}<p class="field-error">
            {occupied ? t("create.occupied") : commandError || backendMessage(status.message)}
          </p>{/if}<Button
          class="primary-button"
          disabled={!canCreate}
          loading={pending}
          onclick={createRoom}><HousePlus size={17} />{t("create.create")}</Button
        >
      </div>
    </section>
  {/if}
</div>
