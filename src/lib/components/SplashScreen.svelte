<script lang="ts">
  import { onMount } from "svelte";
  import { t } from "@i18n";

  let { onready } = $props<{ onready: () => void }>();

  const durationMs = 2000;
  const exitDurationMs = 280;
  let visible = $state(false);
  let leaving = $state(false);

  onMount(() => {
    const enterTimer = window.setTimeout(() => (visible = true), 40);
    const exitTimer = window.setTimeout(() => (leaving = true), durationMs - exitDurationMs);
    const readyTimer = window.setTimeout(onready, durationMs);
    return () => {
      window.clearTimeout(enterTimer);
      window.clearTimeout(exitTimer);
      window.clearTimeout(readyTimer);
    };
  });
</script>

<div class:visible class:leaving class="splash-screen">
  <main class="splash-content" aria-label={t("gds.splashLoading")} aria-live="polite">
    <h1 aria-label="gds3d"><span>gds</span><strong>3d</strong></h1>
    <div class="splash-rule" aria-hidden="true"><span></span></div>
  </main>
</div>

<style>
  .splash-screen {
    position: fixed;
    z-index: 1000;
    inset: 0;
    display: grid;
    place-items: center;
    color: var(--text);
    background: var(--surface-soft);
    opacity: 1;
    transition: opacity 240ms ease;
  }

  .splash-screen.visible {
    opacity: 1;
  }

  .splash-screen.leaving {
    opacity: 0;
  }

  .splash-content {
    width: min(420px, calc(100vw - 64px));
    display: grid;
    justify-items: center;
    text-align: center;
    transform: translateY(8px);
    opacity: 0;
    transition:
      opacity 420ms ease 80ms,
      transform 560ms cubic-bezier(0.22, 1, 0.36, 1) 80ms;
  }

  .visible .splash-content {
    transform: translateY(0);
    opacity: 1;
  }

  h1 {
    margin: 0;
    font-family: "Gds3d Display", serif;
    font-size: clamp(4.25rem, 11vw, 6.2rem);
    font-style: italic;
    font-weight: 500;
    line-height: 1;
    letter-spacing: -0.055em;
  }

  h1 span,
  h1 strong {
    display: inline-block;
  }

  h1 strong {
    margin-left: 0.08em;
    color: var(--primary);
    font-weight: 500;
    letter-spacing: -0.035em;
    transform: translateY(-0.04em) rotate(-2deg);
  }

  .splash-rule {
    width: min(390px, calc(100vw - 72px));
    height: 3px;
    margin-top: 28px;
    overflow: hidden;
    border-radius: 999px;
    background: color-mix(in srgb, var(--primary) 16%, var(--border));
  }

  .splash-rule span {
    display: block;
    width: 100%;
    height: 100%;
    border-radius: inherit;
    background: var(--primary);
    transform: scaleX(0);
    transform-origin: left;
  }

  .visible .splash-rule span {
    animation: splash-progress 1.5s cubic-bezier(0.22, 0.7, 0.2, 1) 180ms forwards;
  }

  @keyframes splash-progress {
    to {
      transform: scaleX(1);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .splash-screen,
    .splash-content {
      transition: none;
    }

    .visible .splash-rule span {
      animation: none;
      transform: scaleX(1);
    }
  }
</style>
