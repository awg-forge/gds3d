<script lang="ts">
  import { Toaster } from "svelte-sonner";
</script>

<Toaster
  position="bottom-right"
  theme="system"
  richColors={false}
  visibleToasts={3}
  toastOptions={{ class: "gds-toast" }}
/>

<style>
  :global([data-sonner-toaster]) {
    z-index: 1200;
    --width: min(360px, calc(100vw - 32px));
    font-family: var(--app-font-family) !important;
    --normal-border: var(--border);
    --normal-text: var(--primary);
  }
  :global([data-sonner-toast].gds-toast) {
    width: fit-content !important;
    min-width: min(240px, calc(100vw - 32px));
    max-width: min(360px, calc(100vw - 32px));
    white-space: normal;
    overflow-wrap: anywhere;
    --normal-bg: var(--surface) !important;
    --normal-bg-hover: var(--surface-strong) !important;
    --normal-border: var(--border) !important;
    --normal-text: var(--primary) !important;
    background: var(--surface) !important;
    border-color: var(--border) !important;
    color: var(--primary) !important;
    font-family: var(--app-font-family) !important;
    user-select: none;
    -webkit-user-select: none;
    backdrop-filter: blur(18px) saturate(1.08);
    box-shadow: var(--shadow);
  }
  :global([data-sonner-toast].gds-toast:hover) {
    background: var(--surface-strong) !important;
  }
  :global([data-sonner-toast].gds-toast [data-icon]) {
    color: var(--primary) !important;
  }
</style>
