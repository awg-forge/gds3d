<script lang="ts">
  import { onMount } from "svelte";
  import { t } from "@i18n";
  import logoUrl from "../../../assets/logo.png";

  let { loading, durationMs, onReady } = $props<{
    loading: boolean;
    durationMs: number;
    onReady: () => void;
  }>();

  let logoScale = $state(0);
  let contentVisible = $state(false);
  let animationComplete = $state(false);
  let startedAt = 0;
  let logoTimer: number | null = null;
  let contentTimer: number | null = null;
  let completionTimer: number | null = null;

  function finishWhenReady(): void {
    if (animationComplete && !loading) onReady();
  }

  function scheduleCompletion(): void {
    if (completionTimer != null) window.clearTimeout(completionTimer);
    const elapsedMs = performance.now() - startedAt;
    completionTimer = window.setTimeout(
      () => {
        animationComplete = true;
        finishWhenReady();
      },
      Math.max(0, durationMs - elapsedMs),
    );
  }

  $effect(() => {
    if (startedAt === 0) return;
    scheduleCompletion();
    finishWhenReady();
  });

  onMount(() => {
    startedAt = performance.now();
    logoTimer = window.setTimeout(() => (logoScale = 1), 50);
    contentTimer = window.setTimeout(() => (contentVisible = true), 200);
    scheduleCompletion();
    return () => {
      if (logoTimer != null) window.clearTimeout(logoTimer);
      if (contentTimer != null) window.clearTimeout(contentTimer);
      if (completionTimer != null) window.clearTimeout(completionTimer);
    };
  });
</script>

<div class="splash-screen">
  <div class="splash-content">
    <div class="splash-logo" style={`transform: scale(${logoScale})`}>
      <img src={logoUrl} alt="SeaLantern Connect" width="96" height="96" />
    </div>
    <div class:visible={contentVisible} class="splash-text">
      <h1>SeaLantern Connect</h1>
      <p>{t("splash.subtitle")}</p>
    </div>
    <div class:visible={contentVisible} class="splash-loader" aria-label={t("splash.starting")}>
      <span></span><span></span><span></span>
    </div>
  </div>
</div>

<style>
  .splash-screen {
    position: fixed;
    inset: 0;
    z-index: 1000;
    display: grid;
    place-items: center;
    background: var(--surface-soft);
  }
  .splash-content {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 20px;
  }
  .splash-logo {
    transition: transform 0.5s cubic-bezier(0.34, 1.56, 0.64, 1);
  }
  .splash-logo img {
    display: block;
    width: 96px;
    height: 96px;
    border-radius: var(--sl-radius-xl);
    box-shadow: 0 10px 24px color-mix(in srgb, var(--primary) 12%, transparent);
  }
  .splash-text,
  .splash-loader {
    opacity: 0;
    transition: opacity 0.4s ease;
  }
  .splash-text.visible,
  .splash-loader.visible {
    opacity: 1;
  }
  .splash-text {
    text-align: center;
  }
  .splash-text h1 {
    margin: 0 0 7px;
    font-size: 2rem;
    font-weight: 600;
    line-height: 1.2;
  }
  .splash-text p {
    margin: 0;
    color: var(--muted);
    font-size: 1rem;
  }
  .splash-loader {
    display: flex;
    gap: 8px;
    margin-top: 4px;
  }
  .splash-loader span {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--primary);
    animation: splash-bounce 1.4s infinite ease-in-out;
  }
  .splash-loader span:nth-child(1) {
    animation-delay: -0.32s;
  }
  .splash-loader span:nth-child(2) {
    animation-delay: -0.16s;
  }
  @keyframes splash-bounce {
    0%,
    80%,
    100% {
      transform: scale(0.8);
      opacity: 0.5;
    }
    40% {
      transform: scale(1.2);
      opacity: 1;
    }
  }
</style>
