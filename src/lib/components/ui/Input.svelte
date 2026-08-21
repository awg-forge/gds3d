<script lang="ts">
  let {
    value = $bindable(""),
    type = "text",
    placeholder = "",
    disabled = false,
    hideNumberControls = false,
    class: className = "",
    onchange,
    onkeydown,
    ...rest
  } = $props<{
    value?: string | number;
    type?: string;
    placeholder?: string;
    disabled?: boolean;
    hideNumberControls?: boolean;
    class?: string;
    onchange?: (event: Event & { currentTarget: HTMLInputElement }) => void;
    onkeydown?: (event: KeyboardEvent) => void;
    [key: string]: unknown;
  }>();
</script>

<input
  class:hide-number-controls={hideNumberControls}
  class={`ui-input ${className}`}
  bind:value
  {type}
  {placeholder}
  {disabled}
  {onchange}
  {onkeydown}
  {...rest}
/>

<style>
  .ui-input {
    width: 100%;
    height: 38px;
    min-height: 38px;
    padding: 0 12px;
    color: var(--text);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 8px;
    outline: 0;
    transition:
      border-color 0.15s ease,
      box-shadow 0.15s ease;
  }
  .ui-input:focus {
    border-color: var(--primary);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--primary) 15%, transparent);
  }
  .ui-input:disabled {
    opacity: 0.55;
  }
  .ui-input.invalid {
    border-color: var(--danger);
    box-shadow: none;
  }
  .ui-input.hide-number-controls::-webkit-inner-spin-button,
  .ui-input.hide-number-controls::-webkit-outer-spin-button {
    margin: 0;
    appearance: none;
  }
  .ui-input.hide-number-controls {
    appearance: textfield;
    -moz-appearance: textfield;
  }
</style>
