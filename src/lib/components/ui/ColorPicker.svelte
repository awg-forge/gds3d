<script lang="ts">
  import AwesomeColorPicker from "svelte-awesome-color-picker";

  let {
    value,
    label,
    disabled = false,
    onvaluechange,
  } = $props<{
    value: string;
    label: string;
    disabled?: boolean;
    onvaluechange: (value: string) => void;
  }>();

  const swatches = [
    "#ffffff",
    "#f1f5f9",
    "#94a3b8",
    "#475569",
    "#0f172a",
    "#0ea5e9",
    "#22c55e",
    "#f59e0b",
    "#ef4444",
    "#a855f7",
  ];

  function updateColor(hex: string | null): void {
    if (!hex || hex.toUpperCase() === value.toUpperCase()) return;
    onvaluechange(hex.toUpperCase());
  }
</script>

{#if disabled}
  <span class="ui-color-picker-swatch" style={`background: ${value}`} aria-label={label}></span>
{:else}
  <div class="ui-color-picker">
    <AwesomeColorPicker
      hex={value}
      {label}
      isAlpha={false}
      isTextInput
      textInputModes={["hex"]}
      {swatches}
      onInput={({ hex }) => updateColor(hex)}
    />
  </div>
{/if}

<style>
  :global(.ui-color-picker) {
    position: relative;
    display: inline-flex;
    --cp-bg-color: var(--overlay-surface);
    --cp-border-color: var(--border);
    --cp-input-color: var(--surface-soft);
    --cp-text-color: var(--text);
    --cp-button-hover-color: var(--surface-strong);
    --focus-color: var(--primary);
    --input-size: 28px;
    --picker-width: 196px;
    --picker-height: 164px;
    --picker-radius: 6px;
    --picker-z-index: 300;
  }
  :global(.ui-color-picker .color-picker > label) {
    gap: 0;
    margin: 0;
    font-size: 0;
  }
  :global(.ui-color-picker .color-picker > label .color) {
    border: 1px solid color-mix(in srgb, var(--text) 20%, var(--border));
    border-radius: 6px;
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--surface) 55%, transparent);
  }
  :global(.ui-color-picker .color-picker > .wrapper) {
    margin: 8px 0 0;
    border-color: var(--border);
    border-radius: 8px;
    box-shadow: 0 10px 24px rgba(0, 0, 0, 0.16);
  }
  :global(.ui-color-picker .color-picker .text-input input),
  :global(.ui-color-picker .color-picker .text-input .button-like) {
    border: 1px solid var(--border);
    color: var(--text);
  }
  .ui-color-picker-swatch {
    width: 28px;
    height: 28px;
    display: inline-block;
    border: 1px solid color-mix(in srgb, var(--text) 20%, var(--border));
    border-radius: 6px;
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--surface) 55%, transparent);
  }
</style>
