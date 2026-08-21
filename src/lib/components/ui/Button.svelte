<script lang="ts">
  import { LoaderCircle } from "@lucide/svelte";
  let {
    variant = "solid",
    size = "md",
    loading = false,
    disabled = false,
    type = "button",
    class: className = "",
    onclick,
    title,
    children,
  } = $props<{
    variant?: "solid" | "outline" | "ghost" | "danger";
    size?: "sm" | "md" | "lg";
    loading?: boolean;
    disabled?: boolean;
    type?: "button" | "submit" | "reset";
    class?: string;
    onclick?: (event: MouseEvent) => void;
    title?: string;
    children: import("svelte").Snippet;
  }>();
</script>

<button
  class={`ui-button ui-button-${variant} ui-button-${size} ${className}`}
  disabled={disabled || loading}
  {title}
  aria-busy={loading}
  {type}
  {onclick}
>
  {#if loading}<LoaderCircle class="spin" size={16} />{:else}{@render children()}{/if}
</button>

<style>
  .ui-button {
    min-height: 38px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 7px;
    padding: 0 18px;
    border: 1px solid transparent;
    border-radius: 8px;
    font-weight: 600;
    cursor: pointer;
    transform: translateY(0);
    transform-origin: center;
    will-change: transform;
    transition:
      transform 0.15s ease,
      background-color 0.15s ease,
      border-color 0.15s ease;
  }
  .ui-button:hover:not(:disabled) {
    transform: translateY(-1px);
  }
  .ui-button:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
  .ui-button[aria-busy="true"] :global(svg:not(.spin)) {
    display: none;
  }
  .ui-button-solid {
    color: #fff;
    background: var(--primary-solid, var(--primary));
  }
  .ui-button-solid:hover:not(:disabled) {
    background: var(--primary-solid-hover, var(--primary-hover));
  }
  .ui-button-outline {
    color: var(--primary);
    background: transparent;
    border-color: var(--primary);
  }
  .ui-button-outline:hover:not(:disabled) {
    background: color-mix(in srgb, var(--primary) 10%, transparent);
  }
  .ui-button-ghost {
    color: var(--muted);
    background: transparent;
  }
  .ui-button-ghost:hover:not(:disabled) {
    color: var(--text);
    background: color-mix(in srgb, var(--text) 8%, transparent);
  }
  .ui-button-danger {
    color: var(--danger);
    background: transparent;
    border-color: color-mix(in srgb, var(--danger) 70%, transparent);
  }
  .ui-button-danger:hover:not(:disabled) {
    background: color-mix(in srgb, var(--danger) 10%, transparent);
  }
  .ui-button-sm {
    min-height: 30px;
    padding: 0 11px;
    font-size: 0.86rem;
  }
  .ui-button-lg {
    min-height: 44px;
    padding: 0 22px;
  }
</style>
