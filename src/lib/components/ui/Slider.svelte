<script lang="ts">
  let {
    value,
    min = 0,
    max = 100,
    step = 1,
    id,
    ariaLabel,
    ariaValueText,
    disabled = false,
    onvaluechange,
  }: {
    value: number;
    min?: number;
    max?: number;
    step?: number;
    id?: string;
    ariaLabel?: string;
    ariaValueText?: string;
    disabled?: boolean;
    onvaluechange?: (value: number) => number | void;
  } = $props();

  let input = $state<HTMLInputElement>();
  const progress = $derived(`${Math.max(0, Math.min(100, ((value - min) / (max - min)) * 100))}%`);

  $effect(() => {
    if (input && Number(input.value) !== value) input.value = String(value);
  });

  function handleInput(event: Event): void {
    const nextValue = onvaluechange?.(Number((event.currentTarget as HTMLInputElement).value));
    if (typeof nextValue === "number" && input) input.value = String(nextValue);
  }
</script>

<input
  bind:this={input}
  {id}
  class="settings-slider"
  type="range"
  {min}
  {max}
  {step}
  {value}
  {disabled}
  style={`--slider-progress: ${progress}`}
  aria-label={ariaLabel}
  aria-valuetext={ariaValueText}
  oninput={handleInput}
/>
