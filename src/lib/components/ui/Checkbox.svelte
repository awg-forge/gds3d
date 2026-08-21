<script lang="ts">
  import { Check, Minus } from "@lucide/svelte";

  let {
    checked = $bindable(false),
    indeterminate = false,
    disabled = false,
    ariaLabel,
    oncheckedchange,
  } = $props<{
    checked?: boolean;
    indeterminate?: boolean;
    disabled?: boolean;
    ariaLabel?: string;
    oncheckedchange?: (checked: boolean) => void;
  }>();

  let input = $state<HTMLInputElement>();

  $effect(() => {
    if (input) input.indeterminate = indeterminate;
  });

  function handleChange(event: Event & { currentTarget: HTMLInputElement }) {
    checked = event.currentTarget.checked;
    oncheckedchange?.(checked);
  }
</script>

<span class:disabled class="ui-checkbox">
  <input
    bind:this={input}
    type="checkbox"
    {checked}
    {disabled}
    aria-label={ariaLabel}
    onchange={handleChange}
  />
  <span class="ui-checkbox-box" aria-hidden="true">
    {#if indeterminate}<Minus size={13} strokeWidth={2.8} />{:else if checked}<Check
        size={13}
        strokeWidth={2.8}
      />{/if}
  </span>
</span>

<style>
  .ui-checkbox {
    position: relative;
    width: 18px;
    height: 18px;
    display: inline-grid;
    flex: 0 0 auto;
    place-items: center;
    vertical-align: middle;
  }
  input {
    position: absolute;
    z-index: 2;
    inset: 0;
    width: 100%;
    height: 100%;
    margin: 0;
    opacity: 0;
    cursor: pointer;
  }
  .ui-checkbox-box {
    width: 18px;
    height: 18px;
    display: grid;
    place-items: center;
    border: 1px solid color-mix(in srgb, var(--muted) 70%, var(--border));
    border-radius: 5px;
    color: #fff;
    background: var(--surface);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--surface) 45%, transparent);
    transition:
      border-color 0.14s ease,
      background-color 0.14s ease,
      box-shadow 0.14s ease,
      transform 0.1s ease;
  }
  input:hover + .ui-checkbox-box {
    border-color: var(--primary);
    background: color-mix(in srgb, var(--primary) 7%, var(--surface));
  }
  input:checked + .ui-checkbox-box,
  input:indeterminate + .ui-checkbox-box {
    border-color: var(--primary-solid);
    background: var(--primary-solid);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, #fff 16%, transparent);
  }
  input:active + .ui-checkbox-box {
    transform: scale(0.9);
  }
  input:focus-visible + .ui-checkbox-box {
    outline: none;
    box-shadow: var(--gds-shadow-focus);
  }
  .disabled {
    opacity: 0.45;
  }
  .disabled input {
    cursor: not-allowed;
  }
</style>
