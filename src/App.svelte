<script lang="ts">
  import { onMount } from "svelte";
  import { Copy, Layers3, Minus, Moon, Settings, Square, Sun, X } from "@lucide/svelte";
  import {
    closeWindow,
    isWindowMaximized,
    minimizeWindow,
    onWindowResized,
    startWindowDragging,
    toggleMaximize,
  } from "@api/window";
  import { locale, setLocale, t } from "@i18n";
  import logo from "./assets/logo.png";
  import defaultTheme from "./themes/default";
  import { applyTypography, readFontFamily, readFontSize } from "./themes/typography";
  import AboutDialog from "./lib/components/AboutDialog.svelte";
  import LayoutView from "./lib/components/LayoutView.svelte";
  import SettingsView from "./lib/components/SettingsView.svelte";
  import SplashScreen from "./lib/components/SplashScreen.svelte";

  type View = "layout" | "settings";
  type ThemeMode = "light" | "dark";
  let activeView = $state<View>(readView());
  let themeMode = $state<ThemeMode>(readThemeMode());
  let fontSize = $state(readFontSize());
  let fontFamily = $state(readFontFamily());
  let lightingIntensity = $state(readLightingIntensity());
  let maximized = $state(false);
  let aboutOpen = $state(false);
  let splashVisible = $state(true);
  let isMac = $state(false);
  let shortcutModifier = $state("Ctrl");

  const navigation = [
    { id: "layout", label: "gds.layout", icon: Layers3 },
    { id: "settings", label: "gds.settings", icon: Settings },
  ] as const;

  type LayoutAction =
    | "openGds"
    | "openProject"
    | "saveProject"
    | "createBaseplate"
    | "renameSelected"
    | "deleteSelected"
    | "resetCamera";
  type MenuId = "file" | "edit" | "view" | "help";
  let openMenu = $state<MenuId | null>(null);

  function preventNativeContextMenu(event: MouseEvent) {
    event.preventDefault();
  }

  onMount(() => {
    isMac = /Mac|iPhone|iPad/.test(navigator.platform);
    shortcutModifier = isMac ? "⌘" : "Ctrl";
    const savedLocale = localStorage.getItem("gds3d.locale");
    if (savedLocale === "zh-CN" || savedLocale === "en") setLocale(savedLocale);
    applyTheme();
    applyTypography(fontSize, fontFamily);
    const closeMenu = () => (openMenu = null);
    window.addEventListener("click", closeMenu);
    window.addEventListener("contextmenu", preventNativeContextMenu);
    window.addEventListener("keydown", handleKeyboardShortcut, true);
    void isWindowMaximized().then((value) => (maximized = value));
    let stopResize: (() => void) | undefined;
    void onWindowResized(() => {
      void isWindowMaximized().then((value) => (maximized = value));
    }).then((unlisten) => (stopResize = unlisten));
    return () => {
      window.removeEventListener("click", closeMenu);
      window.removeEventListener("contextmenu", preventNativeContextMenu);
      window.removeEventListener("keydown", handleKeyboardShortcut, true);
      stopResize?.();
    };
  });

  function readView(): View {
    const saved = localStorage.getItem("gds3d.active-view");
    return saved === "settings" ? saved : "layout";
  }

  function readThemeMode(): ThemeMode {
    const saved = localStorage.getItem("gds3d.theme-mode");
    return saved === "dark" ? "dark" : "light";
  }

  function readLightingIntensity(): number {
    const storedValue = localStorage.getItem("gds3d.lighting-intensity");
    if (storedValue === null) return 1;
    const saved = Number(storedValue);
    return Number.isFinite(saved) ? Math.min(2, Math.max(0.1, saved)) : 1;
  }

  function navigate(view: View) {
    activeView = view;
    localStorage.setItem("gds3d.active-view", view);
  }

  function applyTheme() {
    const dark = themeMode === "dark";
    const colors = defaultTheme[dark ? "dark" : "light"];
    document.documentElement.dataset.theme = dark ? "dark" : "light";
    document.documentElement.style.setProperty("--background", colors.bg);
    document.documentElement.style.setProperty("--surface", colors.bgSecondary);
    document.documentElement.style.setProperty("--surface-soft", colors.bgSecondary);
    document.documentElement.style.setProperty("--surface-strong", colors.bgTertiary);
    document.documentElement.style.setProperty("--slider-track", colors.bgTertiary);
    document.documentElement.style.setProperty("--slider-thumb-border", colors.bgSecondary);
    document.documentElement.style.setProperty("--overlay-surface", colors.bgSecondary);
    document.documentElement.style.setProperty("--primary", colors.primary);
    document.documentElement.style.setProperty("--primary-hover", colors.primarySolidHover);
    document.documentElement.style.setProperty("--primary-solid", colors.primarySolid);
    document.documentElement.style.setProperty("--primary-solid-hover", colors.primarySolidHover);
    document.documentElement.style.setProperty("--accent", colors.secondary);
    document.documentElement.style.setProperty("--text", colors.textPrimary);
    document.documentElement.style.setProperty("--muted", colors.textSecondary);
    document.documentElement.style.setProperty("--border", colors.border);
  }

  function changeThemeMode(nextMode: ThemeMode) {
    themeMode = nextMode;
    localStorage.setItem("gds3d.theme-mode", nextMode);
    applyTheme();
  }

  function changeLocale() {
    const nextLocale = $locale === "zh-CN" ? "en" : "zh-CN";
    setLocale(nextLocale);
    localStorage.setItem("gds3d.locale", nextLocale);
  }

  function changeTypography(next: { fontSize?: number; fontFamily?: string }) {
    if (next.fontSize !== undefined) {
      fontSize = next.fontSize;
      localStorage.setItem("gds3d.font-size", String(fontSize));
    }
    if (next.fontFamily !== undefined) {
      fontFamily = next.fontFamily;
      localStorage.setItem("gds3d.font-family", fontFamily);
    }
    applyTypography(fontSize, fontFamily);
  }

  function changeLightingIntensity(next: number) {
    lightingIntensity = Math.min(2, Math.max(0.1, next));
    localStorage.setItem("gds3d.lighting-intensity", String(lightingIntensity));
  }

  function requestLayoutAction(action: LayoutAction) {
    openMenu = null;
    if (activeView !== "layout") {
      navigate("layout");
      requestAnimationFrame(() =>
        window.dispatchEvent(
          new CustomEvent<LayoutAction>("gds3d-layout-action", { detail: action }),
        ),
      );
      return;
    }
    window.dispatchEvent(new CustomEvent<LayoutAction>("gds3d-layout-action", { detail: action }));
  }

  function shortcutLabel(key: string) {
    return shortcutModifier === "⌘" ? `⌘${key}` : `Ctrl+${key}`;
  }

  function handleKeyboardShortcut(event: KeyboardEvent) {
    if (event.defaultPrevented || event.repeat || event.isComposing || splashVisible) return;
    if (document.querySelector('[role="dialog"][data-state="open"]')) return;
    const key = event.key.toLowerCase();
    const primaryModifier = (event.ctrlKey || event.metaKey) && !event.altKey;
    let action: LayoutAction | null = null;

    if (primaryModifier) {
      if (key === "o") action = event.shiftKey ? "openProject" : "openGds";
      else if (!event.shiftKey && key === "s") action = "saveProject";
      else if (!event.shiftKey && key === "n") action = "createBaseplate";
      else if (!event.shiftKey && key === "f") action = "resetCamera";
      else if (isMac && event.metaKey && !event.shiftKey && key === "backspace") {
        action = "deleteSelected";
      }
    } else if (!event.shiftKey && activeView === "layout") {
      if (openMenu !== null || document.querySelector(".tree-context-menu")) return;
      const target = event.target as HTMLElement | null;
      if (target?.closest("input, textarea, select, [contenteditable='true']")) return;
      if (!isMac && key === "delete") action = "deleteSelected";
      else if (!isMac && key === "f2") action = "renameSelected";
      else if (isMac && key === "enter") {
        const interactiveTarget = target?.closest("button, a, [role='menuitem']");
        if (interactiveTarget && !target?.closest(".object-row")) return;
        action = "renameSelected";
      }
    }

    if (!action) return;
    event.preventDefault();
    event.stopPropagation();
    requestLayoutAction(action);
  }

  function openMenuFromClick(event: MouseEvent, menu: MenuId) {
    event.stopPropagation();
    openMenu = menu;
  }

  function switchOpenMenu(menu: MenuId) {
    if (openMenu !== null) openMenu = menu;
  }

  function openAbout() {
    openMenu = null;
    aboutOpen = true;
  }

  function handleTitlebarMouseDown(event: MouseEvent) {
    if (event.button !== 0) return;
    const target = event.target as HTMLElement;
    if (target.closest("button, [role='menu'], input, select, a")) return;
    void startWindowDragging();
  }

  async function maximizeWindow() {
    await toggleMaximize();
    maximized = await isWindowMaximized();
  }
</script>

<svelte:head><title>gds3d</title></svelte:head>

<div class="app-shell sidebar-collapsed">
  <!-- svelte-ignore a11y_no_static_element_interactions (native window drag gesture) -->
  <header class:macos-overlay={isMac} class="titlebar" onmousedown={handleTitlebarMouseDown}>
    <div class="titlebar-menu">
      {#if !isMac}
        <button
          class="menu-logo-button"
          title={t("gds.layout")}
          aria-label={t("gds.layout")}
          onclick={() => navigate("layout")}><img class="menu-logo" src={logo} alt="" /></button
        >
      {/if}
      <nav class="menubar" aria-label="gds3d">
        <div class="menu-root">
          <button
            class:active={openMenu === "file"}
            class="menu-trigger"
            aria-expanded={openMenu === "file"}
            onmouseenter={() => switchOpenMenu("file")}
            onclick={(event) => openMenuFromClick(event, "file")}>{t("gds.menuFile")}</button
          >
          {#if openMenu === "file"}
            <div class="app-menu" role="menu">
              <button role="menuitem" onclick={() => requestLayoutAction("openGds")}
                ><span>{t("gds.openGds")}</span><kbd>{shortcutLabel("O")}</kbd></button
              >
              <div class="menu-separator"></div>
              <button role="menuitem" onclick={() => requestLayoutAction("openProject")}
                ><span>{t("gds.openProject")}</span><kbd
                  >{shortcutModifier === "⌘" ? "⌘⇧O" : "Ctrl+Shift+O"}</kbd
                ></button
              >
              <button role="menuitem" onclick={() => requestLayoutAction("saveProject")}
                ><span>{t("gds.saveProject")}</span><kbd>{shortcutLabel("S")}</kbd></button
              >
            </div>
          {/if}
        </div>
        <div class="menu-root">
          <button
            class:active={openMenu === "edit"}
            class="menu-trigger"
            aria-expanded={openMenu === "edit"}
            onmouseenter={() => switchOpenMenu("edit")}
            onclick={(event) => openMenuFromClick(event, "edit")}>{t("gds.menuEdit")}</button
          >
          {#if openMenu === "edit"}
            <div class="app-menu" role="menu">
              <button role="menuitem" onclick={() => requestLayoutAction("createBaseplate")}
                ><span>{t("gds.addBaseplate")}</span><kbd>{shortcutLabel("N")}</kbd></button
              >
              <div class="menu-separator"></div>
              <button role="menuitem" onclick={() => requestLayoutAction("renameSelected")}
                ><span>{t("gds.rename")}</span><kbd>{isMac ? "Enter" : "F2"}</kbd></button
              >
              <button role="menuitem" onclick={() => requestLayoutAction("deleteSelected")}
                ><span>{t("gds.delete")}</span><kbd>{isMac ? "⌘⌫" : "Delete"}</kbd></button
              >
            </div>
          {/if}
        </div>
        <div class="menu-root">
          <button
            class:active={openMenu === "view"}
            class="menu-trigger"
            aria-expanded={openMenu === "view"}
            onmouseenter={() => switchOpenMenu("view")}
            onclick={(event) => openMenuFromClick(event, "view")}>{t("gds.menuView")}</button
          >
          {#if openMenu === "view"}
            <div class="app-menu" role="menu">
              <button role="menuitem" onclick={() => requestLayoutAction("resetCamera")}
                ><span>{t("gds.resetCamera")}</span><kbd>{shortcutLabel("F")}</kbd></button
              >
            </div>
          {/if}
        </div>
        <div class="menu-root">
          <button
            class:active={openMenu === "help"}
            class="menu-trigger"
            aria-expanded={openMenu === "help"}
            onmouseenter={() => switchOpenMenu("help")}
            onclick={(event) => openMenuFromClick(event, "help")}>{t("gds.menuHelp")}</button
          >
          {#if openMenu === "help"}
            <div class="app-menu" role="menu">
              <button role="menuitem" onclick={openAbout}>{t("gds.about")}</button>
            </div>
          {/if}
        </div>
      </nav>
      <div class="titlebar-drag-space"></div>
    </div>
    <div class="titlebar-actions">
      <button
        class="header-language-button"
        title={t($locale === "zh-CN" ? "gds.switchToEnglish" : "gds.switchToChinese")}
        onclick={changeLocale}
        ><span aria-hidden="true">{$locale === "zh-CN" ? "中" : "EN"}</span></button
      >
      <div class="theme-switcher" role="group" aria-label={t("gds.theme")}>
        <div
          class="theme-indicator"
          style={`transform: translateX(${["light", "dark"].indexOf(themeMode) * 26}px)`}
        ></div>
        <button
          class:active={themeMode === "light"}
          class="theme-button"
          title={t("gds.lightTheme")}
          onclick={() => changeThemeMode("light")}><Sun size={16} /></button
        >
        <button
          class:active={themeMode === "dark"}
          class="theme-button"
          title={t("gds.darkTheme")}
          onclick={() => changeThemeMode("dark")}><Moon size={16} /></button
        >
      </div>
      {#if !isMac}
        <div class="window-controls">
          <button class="window-button" title={t("gds.windowMinimize")} onclick={minimizeWindow}
            ><Minus size={12} /></button
          >
          <button class="window-button" title={t("gds.windowMaximize")} onclick={maximizeWindow}
            >{#if maximized}<Copy size={12} />{:else}<Square size={12} />{/if}</button
          >
          <button
            class="window-button window-button-close"
            title={t("gds.windowClose")}
            onclick={closeWindow}><X size={12} /></button
          >
        </div>
      {/if}
    </div>
  </header>

  <aside class="sidebar collapsed">
    <nav class="sidebar-nav" aria-label="gds3d">
      <div class="nav-group">
        {#each navigation.slice(0, 1) as item}
          {@const Icon = item.icon}
          <button
            class:active={activeView === item.id}
            class="nav-item"
            title={t(item.label)}
            onclick={() => navigate(item.id)}><Icon class="nav-icon" size={19} /></button
          >
        {/each}
      </div>
      <div class="nav-group nav-group-bottom">
        {#each navigation.slice(1) as item}
          {@const Icon = item.icon}
          <button
            class:active={activeView === item.id}
            class="nav-item"
            title={t(item.label)}
            onclick={() => navigate(item.id)}><Icon class="nav-icon" size={19} /></button
          >
        {/each}
      </div>
    </nav>
  </aside>

  <main class="app-content">
    <div class:inactive={activeView !== "layout"} class="app-view layout-app-view">
      <LayoutView {themeMode} {lightingIntensity} active={activeView === "layout"} />
    </div>
    {#if activeView === "settings"}<div class="app-view settings-app-view">
        <SettingsView
          {fontSize}
          {fontFamily}
          {lightingIntensity}
          ontypographychange={changeTypography}
          onlightingchange={changeLightingIntensity}
        />
      </div>{/if}
  </main>
</div>

<AboutDialog bind:open={aboutOpen} />
{#if splashVisible}<SplashScreen onready={() => (splashVisible = false)} />{/if}

<style>
  .app-view {
    grid-area: 1 / 1;
    min-width: 0;
    min-height: 0;
    display: grid;
  }
  .layout-app-view.inactive {
    visibility: hidden;
    pointer-events: none;
  }
  .settings-app-view {
    background: transparent;
  }
</style>
