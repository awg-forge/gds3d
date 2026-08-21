<script lang="ts">
  let {
    value,
    min = 0,
    max = 100,
    step = 1,
    id,
    ariaLabel,
    ariaValueText,
    onvaluechange,
  } = $props<{
    value: number;
    min?: number;
    max?: number;
    step?: number;
    id?: string;
    ariaLabel?: string;
    ariaValueText?: string;
    onvaluechange?: (value: number) => void;
  }>();

  const progress = $derived(`${Math.max(0, Math.min(100, ((value - min) / (max - min)) * 100))}%`);

  function handleInput(event: Event): void {
    onvaluechange?.(Number((event.currentTarget as HTMLInputElement).value));
  }
</script>

<input
  {id}
  class="settings-slider"
  type="range"
  {min}
  {max}
  {step}
  {value}
  style={`--slider-progress: ${progress}`}
  aria-label={ariaLabel}
  aria-valuetext={ariaValueText}
  oninput={handleInput}
/>
