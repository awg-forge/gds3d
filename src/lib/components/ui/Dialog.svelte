<script lang="ts">
  import { X } from "@lucide/svelte";
  import { Dialog } from "bits-ui";
  let {
    open = $bindable(false),
    title,
    closeLabel = "Close dialog",
    width = "480px",
    children,
    footer,
  } = $props<{
    open?: boolean;
    title: string;
    closeLabel?: string;
    width?: string;
    children: import("svelte").Snippet;
    footer?: import("svelte").Snippet;
  }>();

  function preventImplicitClose(event: { preventDefault: () => void }): void {
    event.preventDefault();
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Portal>
    <Dialog.Overlay class="ui-dialog-overlay" />
    <Dialog.Content
      class="ui-dialog-content"
      style={`--ui-dialog-width: ${width}`}
      onEscapeKeydown={preventImplicitClose}
      onInteractOutside={preventImplicitClose}
    >
      <div class="ui-dialog-header">
        <Dialog.Title class="ui-dialog-title">{title}</Dialog.Title>
        <Dialog.Close class="ui-dialog-close" aria-label={closeLabel}>
          <X size={18} />
        </Dialog.Close>
      </div>
      <div class="ui-dialog-body">{@render children()}</div>
      {#if footer}<div class="ui-dialog-footer">{@render footer()}</div>{/if}
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>

<style>
  :global(.ui-dialog-overlay) {
    position: fixed;
    inset: 0;
    z-index: 800;
    display: flex;
    align-items: center;
    justify-content: center;
    background: color-mix(in srgb, #000 40%, transparent);
    backdrop-filter: blur(8px);
    animation: ui-dialog-fade-in 0.2s ease;
  }
  :global(.ui-dialog-content) {
    position: fixed;
    z-index: 801;
    top: 50%;
    left: 50%;
    width: min(var(--ui-dialog-width, 480px), calc(100vw - 32px));
    max-height: 90vh;
    transform: translate(-50%, -50%);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    color: var(--text);
    background: color-mix(in srgb, var(--overlay-surface) 82%, transparent);
    border: 1px solid var(--border);
    border-radius: 16px;
    box-shadow: 0 16px 48px color-mix(in srgb, #000 40%, transparent);
    backdrop-filter: blur(24px) saturate(180%);
    animation: ui-dialog-scale-in 0.25s cubic-bezier(0.34, 1.56, 0.64, 1);
  }
  :global(.ui-dialog-header) {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 16px 24px;
    border-bottom: 1px solid var(--border);
  }
  :global(.ui-dialog-title) {
    margin: 0;
    color: var(--text);
    font-size: 1.125rem;
    font-weight: 600;
  }
  :global(.ui-dialog-close) {
    width: 32px;
    height: 32px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: 0 0 auto;
    padding: 0;
    color: var(--muted);
    background: var(--surface-strong);
    border: 1px solid var(--border);
    border-radius: 10px;
    cursor: pointer;
    transition:
      color 0.15s ease,
      background-color 0.15s ease,
      border-color 0.15s ease;
  }
  :global(.ui-dialog-close svg) {
    transition: transform 0.2s ease;
  }
  :global(.ui-dialog-close:hover) {
    color: var(--danger);
    background: color-mix(in srgb, var(--danger) 12%, var(--surface-strong));
    border-color: color-mix(in srgb, var(--danger) 60%, var(--border));
  }
  :global(.ui-dialog-close:hover svg) {
    transform: rotate(90deg);
  }
  :global(.ui-dialog-body) {
    flex: 1;
    overflow-y: auto;
    padding: 24px;
    color: var(--muted);
    line-height: 1.55;
  }
  :global(.ui-dialog-footer) {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 8px;
    padding: 16px 24px;
    border-top: 1px solid var(--border);
  }
  @keyframes ui-dialog-fade-in {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }
  @keyframes ui-dialog-scale-in {
    from {
      opacity: 0;
      transform: translate(-50%, -50%) scale(0.96);
    }
    to {
      opacity: 1;
      transform: translate(-50%, -50%) scale(1);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    :global(.ui-dialog-overlay),
    :global(.ui-dialog-content) {
      animation: none;
    }
  }
</style>
