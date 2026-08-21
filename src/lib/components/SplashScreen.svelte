<script lang="ts">
  import { onMount } from "svelte";
  import { t } from "@i18n";
  import logo from "../../assets/logo.png";

  let { onready } = $props<{ onready: () => void }>();

  const durationMs = 1000;
  let logoVisible = $state(false);
  let contentVisible = $state(false);

  onMount(() => {
    const logoTimer = window.setTimeout(() => (logoVisible = true), 24);
    const contentTimer = window.setTimeout(() => (contentVisible = true), 120);
    const readyTimer = window.setTimeout(onready, durationMs);
    return () => {
      window.clearTimeout(logoTimer);
      window.clearTimeout(contentTimer);
      window.clearTimeout(readyTimer);
    };
  });
</script>

<div class="splash-screen">
  <div class="splash-content">
    <div class:visible={logoVisible} class="splash-logo">
      <img src={logo} alt="gds3d" width="88" height="88" />
    </div>
    <div class:visible={contentVisible} class="splash-text">
      <h1>gds3d</h1>
      <p>{t("gds.splashSubtitle")}</p>
    </div>
    <div class:visible={contentVisible} class="splash-loader" aria-label={t("gds.splashLoading")}>
      <span></span><span></span><span></span>
    </div>
  </div>
</div>

<style>
  .splash-screen {
    position: fixed;
    z-index: 1000;
    inset: 0;
    display: grid;
    place-items: center;
    background: var(--surface-soft);
  }

  .splash-content {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 15px;
  }

  .splash-logo {
    opacity: 0;
    transform: scale(0.72);
    transition:
      opacity 0.2s ease,
      transform 0.38s cubic-bezier(0.34, 1.56, 0.64, 1);
  }

  .splash-logo.visible {
    opacity: 1;
    transform: scale(1);
  }

  .splash-logo img {
    display: block;
    width: 88px;
    height: 88px;
    border-radius: var(--gds-radius-xl);
  }

  .splash-text,
  .splash-loader {
    opacity: 0;
    transition: opacity 0.2s ease;
  }

  .splash-text.visible,
  .splash-loader.visible {
    opacity: 1;
  }

  .splash-text {
    text-align: center;
  }

  .splash-text h1 {
    margin: 0 0 4px;
    color: var(--text);
    font-size: 1.7rem;
    font-weight: 650;
    line-height: 1.2;
  }

  .splash-text p {
    margin: 0;
    color: var(--muted);
    font-size: 0.86rem;
  }

  .splash-loader {
    display: flex;
    gap: 6px;
  }

  .splash-loader span {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--primary);
    animation: splash-bounce 1.2s infinite ease-in-out;
  }

  .splash-loader span:nth-child(1) {
    animation-delay: -0.28s;
  }

  .splash-loader span:nth-child(2) {
    animation-delay: -0.14s;
  }

  @keyframes splash-bounce {
    0%,
    80%,
    100% {
      opacity: 0.46;
      transform: scale(0.75);
    }
    40% {
      opacity: 1;
      transform: scale(1.15);
    }
  }
</style>
