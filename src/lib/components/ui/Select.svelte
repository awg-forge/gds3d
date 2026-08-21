<script lang="ts">
  import { Check, ChevronDown, Search } from "@lucide/svelte";
  import { Select } from "bits-ui";
  export interface Option {
    label: string;
    value: string | number;
    fontFamily?: string;
  }
  export interface PointerOrigin {
    x: number;
    y: number;
  }
  interface NormalizedOption {
    label: string;
    value: string;
    fontFamily?: string;
  }
  let {
    value = $bindable<string | number>(),
    options,
    placeholder = "Select",
    disabled = false,
    searchable = false,
    searchPlaceholder = "Search",
    emptyLabel = "No results",
    class: className = "",
    portal = true,
    glass = false,
    onValueChange,
  } = $props<{
    value?: string | number;
    options: Option[];
    placeholder?: string;
    disabled?: boolean;
    searchable?: boolean;
    searchPlaceholder?: string;
    emptyLabel?: string;
    class?: string;
    portal?: boolean;
    glass?: boolean;
    onValueChange?: (value: string, origin?: PointerOrigin) => void;
  }>();
  const normalized: NormalizedOption[] = $derived(
    options.map((item: Option) => ({ ...item, value: String(item.value) })),
  );
  function matchesFuzzy(label: string, query: string): boolean {
    let queryIndex = 0;
    const normalizedLabel = label.toLocaleLowerCase();
    const normalizedQuery = query.toLocaleLowerCase();
    for (const character of normalizedLabel) {
      if (character === normalizedQuery[queryIndex]) queryIndex += 1;
      if (queryIndex === normalizedQuery.length) return true;
    }
    return normalizedQuery.length === 0;
  }
  let selected = $state<string | undefined>(value == null ? undefined : String(value));
  let open = $state(false);
  let optionsReady = $state(true);
  let searchTerm = $state("");
  let searchInput = $state<HTMLInputElement | null>(null);
  let lastPointerOrigin = $state<PointerOrigin | undefined>();
  let keyboardIndex = $state(-1);
  const selectId = `ui-select-${crypto.randomUUID()}`;
  const initialOptionLimit = 100;
  const selectedOption = $derived(
    normalized.find((option: NormalizedOption) => option.value === selected),
  );
  const visibleOptions = $derived(
    searchable && searchTerm.trim()
      ? normalized.filter((option) => matchesFuzzy(option.label, searchTerm.trim()))
      : normalized.slice(0, initialOptionLimit),
  );
  const renderedOptions = $derived(optionsReady ? visibleOptions : []);
  $effect(() => {
    if (keyboardIndex >= renderedOptions.length) {
      keyboardIndex = renderedOptions.length - 1;
    }
  });
  $effect(() => {
    const next = value == null ? undefined : String(value);
    if (selected !== next) selected = next;
  });
  function update(next: string | undefined): void {
    if (next == null) return;
    selected = next;
    value = next;
    const origin = lastPointerOrigin;
    lastPointerOrigin = undefined;
    onValueChange?.(next, origin);
  }
  function fontFamilyStyle(fontFamily: string | undefined): string | undefined {
    return fontFamily ? `font-family: ${JSON.stringify(fontFamily)};` : undefined;
  }
  function marqueeIfOverflow(node: HTMLElement): { destroy: () => void } {
    let frame = 0;
    const updateMarquee = (): void => {
      node.classList.remove("ui-select-marquee");
      const track = node.firstElementChild as HTMLElement | null;
      const content = track?.firstElementChild as HTMLElement | null;
      const contentWidth = Math.ceil(
        Math.max(
          content?.getBoundingClientRect().width ?? 0,
          content?.scrollWidth ?? 0,
          track?.scrollWidth ?? 0,
        ),
      );
      const availableWidth = Math.floor(node.getBoundingClientRect().width);
      node.classList.toggle("ui-select-marquee", contentWidth - availableWidth > 1);
      node.style.setProperty("--ui-select-marquee-distance", `${contentWidth + 32}px`);
    };
    const scheduleUpdate = (): void => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(updateMarquee);
    };
    const resizeObserver = new ResizeObserver(scheduleUpdate);
    const mutationObserver = new MutationObserver(scheduleUpdate);
    resizeObserver.observe(node);
    const content = node.firstElementChild?.firstElementChild;
    if (content instanceof HTMLElement) resizeObserver.observe(content);
    mutationObserver.observe(node, { childList: true, characterData: true, subtree: true });
    void document.fonts?.ready.then(scheduleUpdate);
    scheduleUpdate();
    return {
      destroy: () => {
        cancelAnimationFrame(frame);
        resizeObserver.disconnect();
        mutationObserver.disconnect();
      },
    };
  }
  function focusSearchInput(): void {
    if (!open || !searchable) return;
    requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        if (open && searchable) searchInput?.focus({ preventScroll: true });
      });
    });
  }
  function setKeyboardIndex(index: number): void {
    if (renderedOptions.length === 0) {
      keyboardIndex = -1;
      return;
    }
    keyboardIndex = (index + renderedOptions.length) % renderedOptions.length;
    requestAnimationFrame(() => {
      document
        .getElementById(`${selectId}-option-${keyboardIndex}`)
        ?.scrollIntoView({ block: "nearest" });
    });
  }
  function handleSearchKeydown(event: KeyboardEvent): void {
    switch (event.key) {
      case "ArrowDown":
        event.preventDefault();
        event.stopPropagation();
        setKeyboardIndex(keyboardIndex + 1);
        break;
      case "ArrowUp":
        event.preventDefault();
        event.stopPropagation();
        setKeyboardIndex(keyboardIndex < 0 ? renderedOptions.length - 1 : keyboardIndex - 1);
        break;
      case "Home":
        event.preventDefault();
        event.stopPropagation();
        setKeyboardIndex(0);
        break;
      case "End":
        event.preventDefault();
        event.stopPropagation();
        setKeyboardIndex(renderedOptions.length - 1);
        break;
      case "Enter": {
        event.preventDefault();
        event.stopPropagation();
        const option = renderedOptions[keyboardIndex];
        if (!option) return;
        update(option.value);
        open = false;
        searchTerm = "";
        break;
      }
      case "Escape":
        break;
      default:
        event.stopPropagation();
    }
  }
  function handleTriggerKeydown(event: KeyboardEvent): void {
    if (
      !open ||
      !searchable ||
      event.defaultPrevented ||
      event.isComposing ||
      event.ctrlKey ||
      event.metaKey ||
      event.altKey ||
      event.key.length !== 1
    ) {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    searchInput?.focus({ preventScroll: true });
    searchTerm += event.key;
  }
  function handleOpenChange(nextOpen: boolean): void {
    if (nextOpen) {
      keyboardIndex = 0;
      optionsReady = false;
      requestAnimationFrame(() => {
        if (open) optionsReady = true;
      });
      window.setTimeout(focusSearchInput, 32);
    } else {
      keyboardIndex = -1;
      searchTerm = "";
      optionsReady = !searchable;
    }
  }
</script>

{#snippet selectContent()}
  <Select.Content
    id={`${selectId}-listbox`}
    align="end"
    class={`ui-select-content ${glass ? "ui-select-content--glass" : ""}`}
  >
    {#if searchable}<div class="ui-select-search-wrap">
        <Search class="ui-select-search-icon" size={15} strokeWidth={2} aria-hidden="true" />
        <input
          bind:this={searchInput}
          class="ui-select-search"
          type="search"
          value={searchTerm}
          placeholder={searchPlaceholder}
          aria-label={searchPlaceholder}
          role="combobox"
          aria-autocomplete="list"
          aria-expanded={open}
          aria-controls={`${selectId}-listbox`}
          aria-activedescendant={keyboardIndex >= 0
            ? `${selectId}-option-${keyboardIndex}`
            : undefined}
          oninput={(event) => {
            searchTerm = event.currentTarget.value;
            keyboardIndex = 0;
          }}
          onblur={(event) => {
            const nextTarget = event.relatedTarget as HTMLElement | null;
            if (open && searchable && !nextTarget?.closest(".ui-select-item")) {
              window.setTimeout(focusSearchInput, 0);
            }
          }}
          onkeydown={handleSearchKeydown}
        />
      </div>{/if}
    {#if renderedOptions.length === 0 && searchTerm}<div class="ui-select-empty">
        {emptyLabel}
      </div>{/if}
    {#each renderedOptions as option, index (option.value)}
      <Select.Item
        id={`${selectId}-option-${index}`}
        value={option.value}
        label={option.label}
        class={`ui-select-item ${selected === option.value ? "is-selected" : ""} ${keyboardIndex === index ? "keyboard-highlighted" : ""}`}
        onHighlight={() => (keyboardIndex = index)}
        onpointerdown={(event) => (lastPointerOrigin = { x: event.clientX, y: event.clientY })}
        ><span
          class="ui-select-item-label"
          style={fontFamilyStyle(option.fontFamily)}
          use:marqueeIfOverflow
          ><span class="ui-select-marquee-track"
            ><span>{option.label}</span><span aria-hidden="true">{option.label}</span></span
          ></span
        >{#if selected === option.value}<Check
            class="ui-select-item-check"
            size={15}
            strokeWidth={2.4}
            aria-hidden="true"
          />{/if}</Select.Item
      >
    {/each}
  </Select.Content>
{/snippet}

<Select.Root
  type="single"
  bind:open
  {disabled}
  value={selected}
  items={renderedOptions.map((option) => ({ value: option.value, label: option.label }))}
  onValueChange={update}
  onOpenChange={handleOpenChange}
>
  <Select.Trigger
    class={`ui-select ${glass ? "ui-select--glass" : ""} ${className}`}
    style={fontFamilyStyle(selectedOption?.fontFamily)}
    onkeydown={handleTriggerKeydown}
  >
    <span
      class:ui-select-placeholder={!selectedOption}
      class="ui-select-value"
      use:marqueeIfOverflow
    >
      <span class="ui-select-marquee-track"
        ><span>{selectedOption?.label ?? placeholder}</span><span aria-hidden="true"
          >{selectedOption?.label ?? placeholder}</span
        ></span
      >
    </span>
    <ChevronDown class="ui-select-chevron" size={16} strokeWidth={2} aria-hidden="true" />
  </Select.Trigger>
  {#if portal}
    <Select.Portal>{@render selectContent()}</Select.Portal>
  {:else}
    {@render selectContent()}
  {/if}
</Select.Root>

<style>
  :global(.ui-select) {
    width: 100%;
    height: 38px;
    min-height: 38px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 12px;
    color: var(--text);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 8px;
    cursor: pointer;
    font-size: 0.9286rem;
    line-height: 1.4;
    text-align: left;
  }
  :global(.ui-select:hover) {
    border-color: color-mix(in srgb, var(--primary) 60%, var(--border));
  }
  :global(.ui-select-chevron) {
    flex: none;
    color: var(--muted);
    transition: transform 0.15s ease;
  }
  :global(.ui-select[data-state="open"] .ui-select-chevron) {
    transform: rotate(180deg);
  }
  :global(.ui-select--glass),
  :global(:root[data-native-material] .ui-select) {
    background: color-mix(in srgb, var(--material-content-bg) 62%, transparent);
    border-color: color-mix(in srgb, var(--border) 38%, rgba(255, 255, 255, 0.82));
    border-radius: 14px;
    box-shadow:
      inset 0 1px rgba(255, 255, 255, 0.4),
      inset 0 -1px rgba(255, 255, 255, 0.08);
    backdrop-filter: blur(28px) saturate(1.28);
    -webkit-backdrop-filter: blur(28px) saturate(1.28);
  }
  :global(.ui-select--glass:hover),
  :global(.ui-select--glass[data-state="open"]),
  :global(:root[data-native-material] .ui-select:hover),
  :global(:root[data-native-material] .ui-select[data-state="open"]) {
    background: color-mix(in srgb, var(--material-content-bg) 82%, transparent);
    border-color: color-mix(in srgb, var(--primary) 66%, var(--border));
    box-shadow:
      inset 0 1px rgba(255, 255, 255, 0.54),
      0 8px 24px rgba(0, 0, 0, 0.13);
  }
  :global(.ui-select-value) {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  :global(.ui-select-placeholder) {
    color: var(--muted);
  }
  :global(.ui-select-content) {
    z-index: 900;
    margin-top: 6px;
    width: var(--bits-floating-anchor-width, 0px);
    min-width: 0;
    max-width: calc(100vw - 32px);
    max-height: 280px;
    overflow: auto;
    padding: 5px;
    color: var(--text);
    background: var(--overlay-surface);
    border: 1px solid color-mix(in srgb, var(--border) 82%, var(--text) 10%);
    border-radius: var(--gds-radius-md);
    box-shadow:
      0 10px 28px rgba(0, 0, 0, 0.16),
      0 2px 8px rgba(0, 0, 0, 0.08);
    font-size: 0.9286rem;
    transform-origin: top right;
    will-change: opacity, transform;
  }
  :global(.ui-select-content[data-state="open"]) {
    animation: ui-select-content-in 0.16s cubic-bezier(0.22, 1, 0.36, 1);
  }
  :global(.ui-select-content--glass),
  :global(:root[data-native-material] .ui-select-content) {
    padding: 7px;
    background: color-mix(in srgb, var(--material-content-bg) 92%, transparent);
    border-color: color-mix(in srgb, var(--border) 28%, rgba(255, 255, 255, 0.9));
    border-radius: 18px;
    box-shadow:
      inset 0 1px rgba(255, 255, 255, 0.68),
      inset 0 -1px rgba(255, 255, 255, 0.18),
      0 18px 40px rgba(0, 0, 0, 0.18),
      0 4px 12px rgba(0, 0, 0, 0.08);
    backdrop-filter: blur(40px) saturate(1.34);
    -webkit-backdrop-filter: blur(40px) saturate(1.34);
  }
  :global(.ui-select-search-wrap) {
    min-height: 34px;
    display: flex;
    align-items: center;
    gap: 7px;
    margin: 2px 2px 6px;
    padding: 0 9px;
    color: var(--muted);
    background: color-mix(in srgb, var(--surface-soft) 82%, transparent);
    border: 1px solid transparent;
    border-radius: 6px;
    transition:
      background-color 0.15s ease,
      border-color 0.15s ease,
      box-shadow 0.15s ease;
  }
  :global(.ui-select-search-wrap:focus-within) {
    background: var(--surface);
    border-color: color-mix(in srgb, var(--primary) 68%, var(--border));
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--primary) 13%, transparent);
  }
  :global(.ui-select-search-icon) {
    flex: none;
  }
  :global(.ui-select-search) {
    width: 100%;
    min-width: 0;
    min-height: 32px;
    padding: 0;
    color: var(--text);
    background: transparent;
    border: 0;
    outline: 0;
    font: inherit;
  }
  :global(.ui-select-empty) {
    padding: 12px 10px;
    color: var(--muted);
    font-size: 0.8571rem;
    text-align: center;
  }
  :global(.ui-select-item) {
    min-height: 36px;
    display: flex;
    align-items: center;
    padding: 8px 10px;
    border-radius: 6px;
    cursor: pointer;
    outline: 0;
  }
  :global(.ui-select-item:hover),
  :global(.ui-select-item.keyboard-highlighted) {
    background: color-mix(in srgb, var(--primary) 11%, transparent);
  }
  :global(.ui-select-item.is-selected) {
    color: var(--primary);
    font-weight: 600;
  }
  :global(.ui-select-content--glass .ui-select-item),
  :global(:root[data-native-material] .ui-select-content .ui-select-item) {
    min-height: 36px;
    padding: 7px 12px;
    border-radius: 11px;
  }
  :global(.ui-select-content--glass .ui-select-item.is-selected),
  :global(:root[data-native-material] .ui-select-content .ui-select-item.is-selected) {
    background: color-mix(in srgb, var(--primary) 14%, transparent);
  }
  :global(.ui-select-item-check) {
    flex: none;
    margin-left: 8px;
    color: var(--primary);
  }
  :global(.ui-select-item) {
    overflow: hidden;
    white-space: nowrap;
  }
  :global(.ui-select-item-label) {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  :global(.ui-select-value > span),
  :global(.ui-select-item-label > span) {
    display: inline-block;
    min-width: max-content;
    white-space: nowrap;
  }
  :global(.ui-select-marquee-track > span:last-child) {
    display: none;
  }
  :global(.ui-select-marquee > .ui-select-marquee-track) {
    display: flex;
    gap: 32px;
  }
  :global(.ui-select-marquee > .ui-select-marquee-track > span:last-child) {
    display: inline-block;
  }
  :global(.ui-select:hover .ui-select-value.ui-select-marquee > .ui-select-marquee-track),
  :global(
    .ui-select[data-state="open"] .ui-select-value.ui-select-marquee > .ui-select-marquee-track
  ),
  :global(.ui-select-item:hover .ui-select-marquee > .ui-select-marquee-track),
  :global(.ui-select-item.keyboard-highlighted .ui-select-marquee > .ui-select-marquee-track) {
    animation: ui-select-marquee 2.8s linear 0.6s infinite;
  }
  @keyframes ui-select-content-in {
    from {
      opacity: 0;
      transform: translateY(-3px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
  @keyframes ui-select-marquee {
    to {
      transform: translateX(calc(-1 * var(--ui-select-marquee-distance)));
    }
  }
  @media (prefers-reduced-motion: reduce) {
    :global(.ui-select:hover .ui-select-value.ui-select-marquee > .ui-select-marquee-track),
    :global(
      .ui-select[data-state="open"] .ui-select-value.ui-select-marquee > .ui-select-marquee-track
    ),
    :global(.ui-select-item:hover .ui-select-marquee > .ui-select-marquee-track),
    :global(.ui-select-item.keyboard-highlighted .ui-select-marquee > .ui-select-marquee-track) {
      animation: none;
    }
  }
</style>
