<script lang="ts">
  let {
    checked = $bindable(false),
    disabled = false,
    oncheckedchange,
    label = "Toggle",
  } = $props<{
    checked?: boolean;
    disabled?: boolean;
    oncheckedchange?: (checked: boolean) => void;
    label?: string;
  }>();
  function toggle(): void {
    checked = !checked;
    oncheckedchange?.(checked);
  }
</script>

<button
  class:checked
  class="ui-toggle sl-toggle"
  type="button"
  role="switch"
  aria-label={label}
  aria-checked={checked}
  {disabled}
  onclick={toggle}
>
  <span></span>
</button>

<style>
  .ui-toggle {
    width: 38px;
    height: 22px;
    padding: 2px;
    border: 0;
    border-radius: 999px;
    background: var(--surface-strong);
    cursor: pointer;
    transition: background 0.2s ease;
  }
  .ui-toggle span {
    display: block;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: var(--surface);
    box-shadow: 0 1px 4px #0003;
    transition: transform 0.2s ease;
  }
  .ui-toggle.checked {
    background: var(--primary);
  }
  .ui-toggle.checked span {
    transform: translateX(16px);
    background: var(--overlay-surface);
    box-shadow:
      0 1px 4px rgba(0, 0, 0, 0.28),
      0 0 0 1px color-mix(in srgb, var(--text) 12%, transparent);
  }
  :global(:root[data-theme="light"]) .ui-toggle:not(.checked) {
    background: color-mix(in srgb, var(--text) 4%, var(--surface));
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--text) 12%, transparent);
  }
  :global(:root[data-theme="light"]) .ui-toggle:not(.checked) span {
    background: var(--surface);
    box-shadow:
      0 1px 4px #0003,
      0 0 0 1px color-mix(in srgb, var(--text) 6%, transparent);
  }
  :global(:root[data-theme="dark"]) .ui-toggle:not(.checked) {
    background: color-mix(in srgb, var(--text) 12%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--text) 32%, transparent);
  }
  :global(:root[data-theme="dark"]) .ui-toggle:not(.checked) span {
    background: color-mix(in srgb, var(--text) 76%, var(--surface));
  }
</style>
