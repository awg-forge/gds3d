<script lang="ts">
  import { ChevronDown, ChevronRight, Eye, EyeOff, FolderOpen, RotateCcw } from "@lucide/svelte";
  import { onDestroy, onMount, tick } from "svelte";
  import {
    chooseGdsPath,
    chooseProjectPath,
    chooseProjectSavePath,
    chooseViewExportPath,
    clearScene,
    createBaseplate,
    deleteSceneObject,
    getEditorStatus,
    getSceneSnapshot,
    importGds,
    inspectGdsFile,
    loadProject,
    saveProject,
    saveProjectAs,
    exportView,
    exportModel,
    inspectOccurrence,
    redoScene,
    setObjectsVisibility,
    undoScene,
    updateObjectDisplay,
    type EditorStatus,
    type GdsFileInfo,
    type GdsLayerSelection,
    type Occurrence,
    type OccurrenceInspection,
    type SceneSnapshot,
    type ViewCapture,
    type ViewExportSettings,
  } from "@api/gds";
  import { t } from "@i18n";
  import { finishToast, showLoadingToast, showToast } from "../toast";
  import Button from "./ui/Button.svelte";
  import ColorPicker from "./ui/ColorPicker.svelte";
  import Dialog from "./ui/Dialog.svelte";
  import ImportDialog from "./ImportDialog.svelte";
  import ExportDialog from "./ExportDialog.svelte";
  import Input from "./ui/Input.svelte";
  import Slider from "./ui/Slider.svelte";
  import Viewport from "../Viewport.svelte";

  let {
    themeMode,
    lightingIntensity,
    active,
    onhistorychange,
    onsaveforexitready,
  }: {
    themeMode: "light" | "dark";
    lightingIntensity: number;
    active: boolean;
    onhistorychange?: (status: EditorStatus) => void;
    onsaveforexitready?: (save: (() => Promise<boolean>) | null) => void;
  } = $props();

  type Entry = {
    kind?: string;
    payload?: {
      id?: string;
      display?: {
        name?: string;
        color?: string;
        opacity?: number;
        visible?: boolean;
        z_min?: number;
        z_max?: number;
        defaults?: {
          name?: string;
          color?: string;
          opacity?: number;
          z_min?: number;
          z_max?: number;
        };
      };
      file_path?: string;
      cell_name?: string;
      layer?: number;
      datatype?: number;
      bounds?: {
        min_x?: number;
        min_y?: number;
        max_x?: number;
        max_y?: number;
      };
    };
  };

  type LayerGroup = {
    key: string;
    name: string;
    entries: Entry[];
  };

  type DisplayPatch = {
    name?: string;
    color?: string;
    opacity?: number;
    visible?: boolean;
    z_min?: number;
    z_max?: number;
  };

  type ViewportDisplayEvent = {
    objectId: string;
    update: DisplayPatch;
  };

  type ViewportMeshWaiter = {
    resetKey: number;
    objectIds: string;
    resolve: () => void;
    reject: (reason: unknown) => void;
  };

  type ResizablePanel = "layers" | "properties";
  type ContextMenu =
    | { kind: "blank"; x: number; y: number }
    | { kind: "object"; x: number; y: number; objectId: string; group?: LayerGroup };

  const panelResizeStep = 12;
  const resizerWidth = 8;
  const minimumZSpan = 1;
  const minimumViewportWidth = 360;
  const minimumLayerPanelWidth = 200;
  const maximumLayerPanelWidth = 440;
  const minimumPropertiesPanelWidth = 340;
  const maximumPropertiesPanelWidth = 500;

  let scene = $state<SceneSnapshot | null>(null);
  let sceneRevision = $state(0);
  let selectedId = $state<string | null>(null);
  let selectedOccurrence = $state<OccurrenceInspection | null>(null);
  let occurrenceRequest = 0;
  let busy = $state(false);
  let openingFile = $state<"gds" | "project" | null>(null);
  let importCandidate = $state<GdsFileInfo | null>(null);
  let collapsedGroups = $state<string[]>([]);
  let workbench = $state<HTMLDivElement | null>(null);
  let layerPanelWidth = $state(
    clamp(
      readPanelWidth("gds3d.layer-panel-width", 240),
      minimumLayerPanelWidth,
      maximumLayerPanelWidth,
    ),
  );
  let propertiesPanelWidth = $state(
    clamp(
      readPanelWidth("gds3d.properties-panel-width", 360),
      minimumPropertiesPanelWidth,
      maximumPropertiesPanelWidth,
    ),
  );
  let resizingPanel = $state<ResizablePanel | null>(null);
  let contextMenu = $state<ContextMenu | null>(null);
  let renameTarget = $state<string | null>(null);
  let renameValue = $state("");
  let renameOpen = $state(false);
  let renameInput = $state<HTMLInputElement>();
  let resizeStartX = 0;
  let resizeStartWidth = 0;
  const pendingDisplayUpdates = new Map<string, DisplayPatch>();
  let displayUpdateTimer: number | undefined;
  let viewportMeshWaiter: ViewportMeshWaiter | null = null;
  let captureViewport: ((width: number, height: number) => Promise<ViewCapture>) | null = null;
  let exportViewportModel: ((format: "glb" | "stl") => Promise<string>) | null = null;
  let exportDialogOpen = $state(false);
  let projectPath = $state<string | null>(null);
  let readyViewportResetKey = -1;
  let readyViewportObjectIds = "";
  let selected = $derived(
    scene?.objects.find((entry) => (entry as Entry).payload?.id === selectedId) as
      | Entry
      | undefined,
  );
  let objects = $derived(
    (scene?.objects ?? [])
      .map((entry) => entry as Entry)
      .filter((entry) => entry.kind === "GdsLayer"),
  );
  let baseplates = $derived(
    (scene?.objects ?? [])
      .map((entry) => entry as Entry)
      .filter((entry) => entry.kind === "Baseplate"),
  );
  let viewportObjectIds = $derived(
    (scene?.objects ?? [])
      .map((entry) => (entry as Entry).payload?.id ?? "")
      .filter(Boolean)
      .join("|"),
  );
  let layerGroups = $derived(groupLayers(objects));
  let objectCount = $derived(objects.length + baseplates.length);

  function updateEditorStatus(status: EditorStatus) {
    projectPath = status.projectPath;
    onhistorychange?.(status);
  }

  function replaceScene(snapshot: SceneSnapshot) {
    scene = snapshot;
    updateEditorStatus(snapshot);
  }

  function readPanelWidth(key: string, fallback: number): number {
    const saved = Number(localStorage.getItem(key));
    return Number.isFinite(saved) && saved > 0 ? saved : fallback;
  }

  function clamp(value: number, minimum: number, maximum: number): number {
    return Math.min(Math.max(value, minimum), Math.max(minimum, maximum));
  }

  function firstLayerId(snapshot: SceneSnapshot): string | null {
    const firstLayer = snapshot.objects
      .map((entry) => entry as Entry)
      .find((entry) => entry.kind === "GdsLayer" && entry.payload?.id);
    return firstLayer?.payload?.id ?? null;
  }

  function snapshotObjectIds(snapshot: SceneSnapshot): string {
    return snapshot.objects
      .map((entry) => (entry as Entry).payload?.id ?? "")
      .filter(Boolean)
      .join("|");
  }

  function errorMessage(reason: unknown): string {
    return reason instanceof Error ? reason.message : String(reason);
  }

  function shapeTypeLabel(shapeType: string): string {
    const translationKey =
      {
        Boundary: "gds.shapeTypes.boundary",
        Path: "gds.shapeTypes.path",
        Rectangle: "gds.shapeTypes.rectangle",
      }[shapeType] ?? null;
    return translationKey ? t(translationKey) : shapeType;
  }

  function selectSceneObject(objectId: string | null) {
    void inspectViewportPick(objectId ? { objectId, occurrence: null } : null);
  }

  async function inspectViewportPick(
    pick: { objectId: string; occurrence: Occurrence | null } | null,
  ) {
    const request = ++occurrenceRequest;
    if (!pick?.occurrence) {
      selectedId = pick?.objectId ?? null;
      selectedOccurrence = null;
      return;
    }
    try {
      const inspection = await inspectOccurrence(pick.occurrence);
      if (request === occurrenceRequest) {
        selectedId = pick.objectId;
        selectedOccurrence = inspection;
      }
    } catch (reason) {
      if (request === occurrenceRequest) showToast(errorMessage(reason), "error");
    }
  }

  function waitForViewportMeshes(resetKey: number, objectIds: string): Promise<void> {
    if (readyViewportResetKey === resetKey && readyViewportObjectIds === objectIds) {
      return Promise.resolve();
    }
    return new Promise((resolve, reject) => {
      viewportMeshWaiter = { resetKey, objectIds, resolve, reject };
    });
  }

  function handleViewportMeshesReady(resetKey: number, objectIds: string) {
    readyViewportResetKey = resetKey;
    readyViewportObjectIds = objectIds;
    const waiter = viewportMeshWaiter;
    if (!waiter || waiter.resetKey !== resetKey || waiter.objectIds !== objectIds) return;
    viewportMeshWaiter = null;
    waiter.resolve();
  }

  function handleViewportMeshesError(resetKey: number, reason: unknown) {
    const waiter = viewportMeshWaiter;
    if (!waiter || waiter.resetKey !== resetKey) {
      showToast(errorMessage(reason), "error");
      return;
    }
    viewportMeshWaiter = null;
    waiter.reject(reason);
  }

  function panelWidthLimit(panel: ResizablePanel): number {
    if (!workbench) {
      return panel === "layers" ? maximumLayerPanelWidth : maximumPropertiesPanelWidth;
    }
    const propertiesVisible = window.matchMedia("(min-width: 1101px)").matches;
    const visibleResizerCount = propertiesVisible ? 2 : 1;
    const availableWidth =
      workbench.clientWidth - resizerWidth * visibleResizerCount - minimumViewportWidth;
    return panel === "layers"
      ? Math.min(
          maximumLayerPanelWidth,
          availableWidth - (propertiesVisible ? propertiesPanelWidth : 0),
        )
      : Math.min(maximumPropertiesPanelWidth, availableWidth - layerPanelWidth);
  }

  function setPanelWidth(panel: ResizablePanel, width: number) {
    if (panel === "layers") {
      layerPanelWidth = clamp(width, minimumLayerPanelWidth, panelWidthLimit(panel));
      return;
    }
    propertiesPanelWidth = clamp(width, minimumPropertiesPanelWidth, panelWidthLimit(panel));
  }

  function startPanelResize(event: PointerEvent, panel: ResizablePanel) {
    if (event.button !== 0) return;
    event.preventDefault();
    resizingPanel = panel;
    resizeStartX = event.clientX;
    resizeStartWidth = panel === "layers" ? layerPanelWidth : propertiesPanelWidth;
    (event.currentTarget as HTMLButtonElement).setPointerCapture(event.pointerId);
  }

  function continuePanelResize(event: PointerEvent, panel: ResizablePanel) {
    if (resizingPanel !== panel) return;
    const pointerDelta = event.clientX - resizeStartX;
    setPanelWidth(panel, resizeStartWidth + (panel === "layers" ? pointerDelta : -pointerDelta));
  }

  function finishPanelResize(event: PointerEvent, panel: ResizablePanel) {
    if (resizingPanel !== panel) return;
    const resizer = event.currentTarget as HTMLButtonElement;
    if (resizer.hasPointerCapture(event.pointerId)) {
      resizer.releasePointerCapture(event.pointerId);
    }
    resizingPanel = null;
    persistPanelWidth(panel);
  }

  function resizePanelWithKeyboard(event: KeyboardEvent, panel: ResizablePanel) {
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
    event.preventDefault();
    const direction = event.key === "ArrowRight" ? 1 : -1;
    const currentWidth = panel === "layers" ? layerPanelWidth : propertiesPanelWidth;
    setPanelWidth(
      panel,
      currentWidth + direction * panelResizeStep * (panel === "layers" ? 1 : -1),
    );
    persistPanelWidth(panel);
  }

  function persistPanelWidth(panel: ResizablePanel) {
    const key = panel === "layers" ? "gds3d.layer-panel-width" : "gds3d.properties-panel-width";
    const width = panel === "layers" ? layerPanelWidth : propertiesPanelWidth;
    localStorage.setItem(key, String(Math.round(width)));
  }

  function groupLayers(entries: Entry[]): LayerGroup[] {
    const groups = new Map<string, LayerGroup>();
    for (const entry of entries) {
      const cellName = entry.payload?.cell_name ?? t("gds.layout");
      const key = `${entry.payload?.file_path ?? ""}\u0000${cellName}`;
      const group = groups.get(key) ?? { key, name: cellName, entries: [] };
      group.entries.push(entry);
      groups.set(key, group);
    }
    return [...groups.values()];
  }

  function toggleGroup(key: string) {
    collapsedGroups = collapsedGroups.includes(key)
      ? collapsedGroups.filter((groupKey) => groupKey !== key)
      : [...collapsedGroups, key];
  }

  function openContextMenu(event: MouseEvent, menu: ContextMenu) {
    event.preventDefault();
    event.stopPropagation();
    contextMenu = menu;
  }

  function closeContextMenu() {
    contextMenu = null;
  }

  function requestRename(objectId: string) {
    const object = (scene?.objects ?? []).find(
      (entry) => (entry as Entry).payload?.id === objectId,
    ) as Entry | undefined;
    renameTarget = objectId;
    renameValue = object?.payload?.display?.name ?? "";
    renameOpen = true;
    closeContextMenu();
  }

  function confirmRename() {
    const objectId = renameTarget;
    const name = renameValue.trim();
    if (!objectId || !name) return;
    updateDisplay(objectId, { name });
    renameOpen = false;
    renameTarget = null;
  }

  $effect(() => {
    if (!renameOpen) renameTarget = null;
  });

  $effect(() => {
    if (!renameOpen || !renameInput) return;
    const input = renameInput;
    requestAnimationFrame(() => {
      if (!renameOpen || renameInput !== input) return;
      input.focus();
      input.select();
    });
  });

  function addBaseplate(group?: LayerGroup) {
    closeContextMenu();
    const existingIds = new Set(
      (scene?.objects ?? []).map((entry) => (entry as Entry).payload?.id).filter(Boolean),
    );
    void run(async () => {
      const updatedScene = await createBaseplate(
        group
          ? {
              filePath: group.entries[0]?.payload?.file_path ?? "",
              cellName: group.name,
            }
          : undefined,
      );
      replaceScene(updatedScene);
      const addedBaseplate = updatedScene.objects
        .map((entry) => entry as Entry)
        .find(
          (entry) =>
            entry.kind === "Baseplate" &&
            Boolean(entry.payload?.id) &&
            !existingIds.has(entry.payload?.id),
        );
      selectSceneObject(addedBaseplate?.payload?.id ?? selectedId);
    });
  }

  function deleteObject(objectId: string) {
    closeContextMenu();
    void run(async () => {
      const updatedScene = await deleteSceneObject(objectId);
      replaceScene(updatedScene);
      selectSceneObject(firstLayerId(updatedScene));
    });
  }

  function patchLocalDisplay(objectIds: string[], update: DisplayPatch) {
    if (!scene) return;
    const selectedIds = new Set(objectIds);
    for (const objectId of objectIds) {
      window.dispatchEvent(
        new CustomEvent<ViewportDisplayEvent>("gds3d-viewport-display", {
          detail: { objectId, update },
        }),
      );
    }
    for (const entry of scene.objects) {
      const object = entry as Entry;
      if (!object.payload?.id || !selectedIds.has(object.payload.id) || !object.payload.display) {
        continue;
      }
      Object.assign(object.payload.display, update);
    }
    scene.revision += 1;
  }

  function scheduleDisplayUpdate(id: string, update: DisplayPatch) {
    patchLocalDisplay([id], update);
    pendingDisplayUpdates.set(id, { ...pendingDisplayUpdates.get(id), ...update });
    if (displayUpdateTimer !== undefined) window.clearTimeout(displayUpdateTimer);
    displayUpdateTimer = window.setTimeout(() => void flushDisplayUpdates(), 60);
  }

  async function flushDisplayUpdates() {
    if (displayUpdateTimer !== undefined) window.clearTimeout(displayUpdateTimer);
    displayUpdateTimer = undefined;
    const updates = [...pendingDisplayUpdates.entries()];
    pendingDisplayUpdates.clear();
    try {
      await Promise.all(
        updates.map(([objectId, update]) =>
          updateObjectDisplay({
            objectId,
            name: update.name,
            color: update.color,
            opacity: update.opacity,
            visible: update.visible,
            zMin: update.z_min,
            zMax: update.z_max,
          }),
        ),
      );
      if (updates.length > 0) updateEditorStatus(await getEditorStatus());
    } catch (reason) {
      showToast(errorMessage(reason), "error");
    }
  }

  function applyHistorySnapshot(snapshot: SceneSnapshot) {
    replaceScene(snapshot);
    for (const entry of snapshot.objects) {
      const object = entry as Entry;
      const id = object.payload?.id;
      const display = object.payload?.display;
      if (!id || !display) continue;
      window.dispatchEvent(
        new CustomEvent<ViewportDisplayEvent>("gds3d-viewport-display", {
          detail: {
            objectId: id,
            update: {
              color: display.color,
              opacity: display.opacity,
              visible: display.visible,
              z_min: display.z_min,
              z_max: display.z_max,
            },
          },
        }),
      );
    }
    const selectionStillExists = snapshot.objects.some(
      (entry) => (entry as Entry).payload?.id === selectedId,
    );
    if (!selectionStillExists) selectSceneObject(firstLayerId(snapshot));
  }

  function changeHistory(direction: "undo" | "redo") {
    closeContextMenu();
    void run(async () => {
      await flushDisplayUpdates();
      applyHistorySnapshot(direction === "undo" ? await undoScene() : await redoScene());
    });
  }

  async function updateGroupVisibility(group: LayerGroup, visible: boolean) {
    const objectIds = group.entries
      .map((entry) => entry.payload?.id)
      .filter((id): id is string => Boolean(id));
    patchLocalDisplay(objectIds, { visible });
    await flushDisplayUpdates();
    try {
      updateEditorStatus(await setObjectsVisibility(objectIds, visible));
    } catch (reason) {
      showToast(errorMessage(reason), "error");
    }
  }

  onMount(() => {
    onsaveforexitready?.(saveCurrentProject);
    const handleLayoutAction = (event: Event) => {
      switch ((event as CustomEvent<string>).detail) {
        case "openGds":
          void chooseGds();
          break;
        case "openProject":
          void openProject();
          break;
        case "saveProject":
          void saveCurrentProject();
          break;
        case "saveAs":
          void saveCurrentProjectAs();
          break;
        case "exportAs":
          openExportDialog();
          break;
        case "closeProject":
          void closeProject();
          break;
        case "resetCamera":
          resetCamera();
          break;
        case "viewTop":
          setCameraView("top");
          break;
        case "viewFront":
          setCameraView("front");
          break;
        case "viewLeft":
          setCameraView("left");
          break;
        case "viewRight":
          setCameraView("right");
          break;
        case "viewBack":
          setCameraView("back");
          break;
        case "viewBottom":
          setCameraView("bottom");
          break;
        case "createBaseplate":
          addBaseplate();
          break;
        case "undo":
          changeHistory("undo");
          break;
        case "redo":
          changeHistory("redo");
          break;
        case "renameSelected":
          if (selectedId) requestRename(selectedId);
          break;
        case "deleteSelected":
          if (selectedId) deleteObject(selectedId);
          break;
      }
    };
    const closeMenus = () => closeContextMenu();
    window.addEventListener("gds3d-layout-action", handleLayoutAction);
    window.addEventListener("click", closeMenus);
    void getSceneSnapshot().then((snapshot) => {
      replaceScene(snapshot);
      sceneRevision += 1;
      return undefined;
    });
    return () => {
      onsaveforexitready?.(null);
      window.removeEventListener("gds3d-layout-action", handleLayoutAction);
      window.removeEventListener("click", closeMenus);
    };
  });

  onDestroy(() => {
    viewportMeshWaiter?.reject(new Error("viewport was destroyed"));
    viewportMeshWaiter = null;
    void flushDisplayUpdates();
  });

  async function chooseGds() {
    const path = await chooseGdsPath();
    if (!path) return;
    openingFile = "gds";
    try {
      await run(
        async () => {
          importCandidate = await inspectGdsFile(path);
        },
        (reason) => t("gds.importFailed", { message: errorMessage(reason) }),
      );
    } finally {
      openingFile = null;
    }
  }

  async function confirmImport(selections: GdsLayerSelection[]) {
    const candidate = importCandidate;
    if (!candidate) return;
    await run(
      async () => {
        const importedScene = await importGds(candidate.file_path, selections);
        replaceScene(importedScene);
        selectSceneObject(firstLayerId(importedScene));
        sceneRevision += 1;
        const meshesReady = waitForViewportMeshes(sceneRevision, snapshotObjectIds(importedScene));
        await tick();
        await meshesReady;
        importCandidate = null;
        showToast(t("gds.importSuccess"), "success");
      },
      (reason) => t("gds.importFailed", { message: errorMessage(reason) }),
    );
  }

  async function openProject() {
    const path = await chooseProjectPath();
    if (!path) return;
    openingFile = "project";
    busy = true;
    const toastId = showLoadingToast(t("gds.projectOpening"));
    try {
      await nextPaint();
      const loadedScene = await loadProject(path);
      replaceScene(loadedScene);
      sceneRevision += 1;
      selectedId = null;
      const meshesReady = waitForViewportMeshes(sceneRevision, snapshotObjectIds(loadedScene));
      await tick();
      await meshesReady;
      finishToast(toastId, t("gds.projectOpenSuccess"), "success");
    } catch (reason) {
      finishToast(toastId, t("gds.projectOpenFailed", { message: errorMessage(reason) }), "error");
    } finally {
      busy = false;
      openingFile = null;
    }
  }

  async function saveCurrentProject(): Promise<boolean> {
    if (!scene) return false;
    if (projectPath) {
      return persistProject(null);
    }
    const path = await chooseProjectSavePath();
    return path ? persistProject(path) : false;
  }

  async function saveCurrentProjectAs(): Promise<boolean> {
    if (!scene) return false;
    const path = await chooseProjectSavePath(projectPath ?? undefined);
    return path ? persistProject(path) : false;
  }

  async function persistProject(path: string | null): Promise<boolean> {
    await flushDisplayUpdates();
    busy = true;
    try {
      replaceScene(path ? await saveProjectAs(path) : await saveProject());
      showToast(t("gds.projectSaveSuccess"), "success");
      return true;
    } catch (reason) {
      showToast(t("gds.projectSaveFailed", { message: errorMessage(reason) }), "error");
      return false;
    } finally {
      busy = false;
    }
  }

  function openExportDialog() {
    if (!scene || !captureViewport) return;
    exportDialogOpen = true;
  }

  async function exportCurrentView(settings: ViewExportSettings) {
    const modelExport = settings.format === "glb" || settings.format === "stl";
    if ((!modelExport && !captureViewport) || (modelExport && !exportViewportModel)) return;
    const path = await chooseViewExportPath(settings.format, settings.quality);
    if (!path) return;
    exportDialogOpen = false;
    busy = true;
    const toastId = showLoadingToast(
      modelExport
        ? t("gds.exportingModel", { format: settings.format.toUpperCase() })
        : t("gds.exporting", { width: settings.width, height: settings.height }),
    );
    try {
      await nextPaint();
      if (modelExport) {
        const dataUrl = await exportViewportModel?.(settings.format as "glb" | "stl");
        if (!dataUrl) throw new Error(t("gds.exportUnavailable"));
        await exportModel(path, dataUrl);
      } else {
        const capture = await captureViewport?.(settings.width, settings.height);
        if (!capture) throw new Error(t("gds.exportUnavailable"));
        await exportView(path, capture);
      }
      finishToast(toastId, t("gds.exportSuccess"), "success");
    } catch (reason) {
      finishToast(toastId, t("gds.exportFailed", { message: errorMessage(reason) }), "error");
    } finally {
      busy = false;
    }
  }

  function nextPaint(): Promise<void> {
    return new Promise((resolve) =>
      requestAnimationFrame(() => requestAnimationFrame(() => window.setTimeout(resolve, 0))),
    );
  }

  async function closeProject() {
    await flushDisplayUpdates();
    await run(
      async () => {
        const clearedScene = await clearScene();
        replaceScene(clearedScene);
        selectedId = null;
        importCandidate = null;
        collapsedGroups = [];
        closeContextMenu();
        renameOpen = false;
        sceneRevision += 1;
        const meshesReady = waitForViewportMeshes(sceneRevision, snapshotObjectIds(clearedScene));
        await tick();
        await meshesReady;
        showToast(t("gds.closeProjectSuccess"), "success");
      },
      (reason) => t("gds.closeProjectFailed", { message: errorMessage(reason) }),
    );
  }

  async function run(
    action: () => Promise<unknown>,
    failureMessage: (reason: unknown) => string = errorMessage,
  ) {
    busy = true;
    try {
      await action();
    } catch (reason) {
      showToast(failureMessage(reason), "error");
    } finally {
      busy = false;
    }
  }

  function updateDisplay(id: string, update: DisplayPatch) {
    scheduleDisplayUpdate(id, update);
  }

  function updateNumber(id: string, key: "opacity" | "z_min" | "z_max", value: number) {
    if (!Number.isFinite(value)) return undefined;
    const display = (
      scene?.objects.find((entry) => (entry as Entry).payload?.id === id) as Entry | undefined
    )?.payload?.display;
    const clamped =
      key === "opacity"
        ? Math.min(1, Math.max(0, value))
        : key === "z_min"
          ? Math.min(value, (display?.z_max ?? value + minimumZSpan) - minimumZSpan)
          : key === "z_max"
            ? Math.max(value, (display?.z_min ?? value - minimumZSpan) + minimumZSpan)
            : value;
    updateDisplay(id, { [key]: clamped });
    return clamped;
  }

  function resetCamera() {
    window.dispatchEvent(new CustomEvent("gds3d-reset-camera"));
  }

  function setCameraView(view: "top" | "front" | "left" | "right" | "back" | "bottom") {
    window.dispatchEvent(new CustomEvent("gds3d-reset-camera", { detail: view }));
  }
</script>

{#if exportDialogOpen}
  <ExportDialog {busy} onexport={exportCurrentView} oncancel={() => (exportDialogOpen = false)} />
{/if}

<div class="layout-page" aria-busy={busy}>
  <div
    bind:this={workbench}
    class:resizing={resizingPanel !== null}
    class="layout-workbench"
    style={`--layer-panel-width: ${layerPanelWidth}px; --properties-panel-width: ${propertiesPanelWidth}px`}
  >
    <aside
      class="layout-panel layer-panel"
      role="region"
      aria-label={t("gds.layers")}
      oncontextmenu={(event) =>
        openContextMenu(event, { kind: "blank", x: event.clientX, y: event.clientY })}
    >
      <div class="panel-heading">
        <h2>{t("gds.layers")}</h2>
        <span>{objectCount}</span>
      </div>
      {#if scene?.objects.length}<div class="object-list">
          {#each layerGroups as group}
            {@const groupVisible = group.entries.every(
              (entry) => entry.payload?.display?.visible ?? true,
            )}
            <div class="tree-group">
              <div class="tree-root-row">
                <button class="tree-root" onclick={() => toggleGroup(group.key)}>
                  {#if collapsedGroups.includes(group.key)}<ChevronRight
                      class="tree-chevron"
                      size={16}
                    />{:else}<ChevronDown class="tree-chevron" size={16} />{/if}
                  <strong>{group.name}</strong>
                </button>
                <button
                  class:visible={groupVisible}
                  class="visibility-button"
                  title={t(groupVisible ? "gds.hideGroup" : "gds.showGroup")}
                  aria-label={t(groupVisible ? "gds.hideGroup" : "gds.showGroup")}
                  aria-pressed={groupVisible}
                  onclick={() => updateGroupVisibility(group, !groupVisible)}
                  >{#if groupVisible}<Eye size={16} />{:else}<EyeOff size={16} />{/if}</button
                >
              </div>
              {#if !collapsedGroups.includes(group.key)}
                <div class="tree-children">
                  {#each group.entries as entry}{@const id = entry.payload?.id ?? ""}
                    {@const visible = entry.payload?.display?.visible ?? true}
                    <div
                      class:selected={id === selectedId}
                      class="object-row"
                      role="treeitem"
                      tabindex="-1"
                      aria-selected={id === selectedId}
                      oncontextmenu={(event) => {
                        selectSceneObject(id);
                        openContextMenu(event, {
                          kind: "object",
                          x: event.clientX,
                          y: event.clientY,
                          objectId: id,
                          group,
                        });
                      }}
                    >
                      <Button
                        class="object-button"
                        variant="ghost"
                        size="sm"
                        onclick={() => selectSceneObject(id)}
                        >{entry.payload?.display?.name ?? t("gds.layer")}</Button
                      ><button
                        class:visible
                        class="visibility-button"
                        title={t(visible ? "gds.hideLayer" : "gds.showLayer")}
                        aria-label={t(visible ? "gds.hideLayer" : "gds.showLayer")}
                        aria-pressed={visible}
                        onclick={() => id && updateDisplay(id, { visible: !visible })}
                        >{#if visible}<Eye size={16} />{:else}<EyeOff size={16} />{/if}</button
                      >
                    </div>{/each}
                </div>
              {/if}
            </div>
          {/each}
          {#if baseplates.length}
            {#each baseplates as entry}{@const id = entry.payload?.id ?? ""}
              <div
                class:selected={id === selectedId}
                class="object-row baseplate-root"
                role="treeitem"
                tabindex="-1"
                aria-selected={id === selectedId}
                oncontextmenu={(event) => {
                  selectSceneObject(id);
                  openContextMenu(event, {
                    kind: "object",
                    x: event.clientX,
                    y: event.clientY,
                    objectId: id,
                  });
                }}
              >
                <Button
                  class="object-button"
                  variant="ghost"
                  size="sm"
                  onclick={() => selectSceneObject(id)}
                  >{entry.payload?.display?.name ?? t("gds.baseplate")}</Button
                ><button
                  class:visible={entry.payload?.display?.visible ?? true}
                  class="visibility-button"
                  onclick={() =>
                    id &&
                    updateDisplay(id, { visible: !(entry.payload?.display?.visible ?? true) })}
                  >{#if entry.payload?.display?.visible ?? true}<Eye size={16} />{:else}<EyeOff
                      size={16}
                    />{/if}</button
                >
              </div>
            {/each}
          {/if}
        </div>{:else}<div class="empty-actions">
          <Button
            size="sm"
            loading={openingFile === "gds"}
            disabled={busy && openingFile !== "gds"}
            onclick={chooseGds}>{t("gds.openGds")}</Button
          >
          <Button
            size="sm"
            variant="outline"
            loading={openingFile === "project"}
            disabled={busy && openingFile !== "project"}
            onclick={openProject}>{t("gds.openProject")}</Button
          >
        </div>{/if}
    </aside>

    <button
      class:active={resizingPanel === "layers"}
      class="panel-resizer layer-resizer"
      title={t("gds.resizeLayers")}
      aria-label={t("gds.resizeLayers")}
      onpointerdown={(event) => startPanelResize(event, "layers")}
      onpointermove={(event) => continuePanelResize(event, "layers")}
      onpointerup={(event) => finishPanelResize(event, "layers")}
      onpointercancel={(event) => finishPanelResize(event, "layers")}
      onkeydown={(event) => resizePanelWithKeyboard(event, "layers")}
      ><span class="resizer-grip" aria-hidden="true"></span></button
    >

    <section class="viewport-panel">
      {#if scene}<Viewport
          objects={scene.objects}
          occurrences={scene.occurrences}
          objectIds={viewportObjectIds}
          {themeMode}
          {lightingIntensity}
          resetKey={sceneRevision}
          resizePaused={resizingPanel !== null || !active}
          onPick={inspectViewportPick}
          onMeshesReady={handleViewportMeshesReady}
          onMeshesError={handleViewportMeshesError}
          onCaptureReady={(capture) => (captureViewport = capture)}
          onModelExportReady={(exporter) => (exportViewportModel = exporter)}
        />{:else}<div class="viewport-empty">
          <FolderOpen size={32} /><strong>{t("gds.openLayout")}</strong><span
            >{t("gds.sceneAppearsHere")}</span
          >
        </div>{/if}
      {#if resizingPanel}<div class="viewport-resize-mask" aria-hidden="true"></div>{/if}
    </section>

    <button
      class:active={resizingPanel === "properties"}
      class="panel-resizer properties-resizer"
      title={t("gds.resizeProperties")}
      aria-label={t("gds.resizeProperties")}
      onpointerdown={(event) => startPanelResize(event, "properties")}
      onpointermove={(event) => continuePanelResize(event, "properties")}
      onpointerup={(event) => finishPanelResize(event, "properties")}
      onpointercancel={(event) => finishPanelResize(event, "properties")}
      onkeydown={(event) => resizePanelWithKeyboard(event, "properties")}
      ><span class="resizer-grip" aria-hidden="true"></span></button
    >

    <aside class="layout-panel properties-panel">
      <div class="panel-heading"><h2>{t("gds.properties")}</h2></div>
      {#if selected}{@const id = selected.payload?.id ?? ""}
        {@const display = selected.payload?.display}
        {@const defaults = display?.defaults}
        <div class="property-grid">
          <section class="property-section">
            <h3>{t("gds.basic")}</h3>
            {#if selected.kind === "Baseplate"}
              <div class="property-field readonly property-type">
                <span>{t("gds.objectType")}</span><strong>{t("gds.baseplate")}</strong>
              </div>
            {:else if selectedOccurrence}
              <div class="property-field readonly property-type">
                <span>{t("gds.objectType")}</span><strong
                  >{shapeTypeLabel(selectedOccurrence.shape_type)}</strong
                >
              </div>
              <div class="property-field readonly">
                <span>{t("gds.cell")}</span><strong>{selectedOccurrence.cell_name}</strong>
              </div>
              <div class="property-field readonly">
                <span>{t("gds.layer")}</span><strong
                  >L{selectedOccurrence.layer}/D{selectedOccurrence.datatype}</strong
                >
              </div>
              {#if selectedOccurrence.hierarchy_path.length > 1}
                <div class="property-field readonly">
                  <span>{t("gds.hierarchy")}</span><strong
                    >{selectedOccurrence.hierarchy_path.join(" / ")}</strong
                  >
                </div>
              {/if}
            {:else}
              <div class="property-field readonly">
                <span>{t("gds.cell")}</span><strong>{selected.payload?.cell_name ?? "—"}</strong>
              </div>
              <div class="property-field readonly">
                <span>{t("gds.layer")}</span><strong
                  >L{selected.payload?.layer ?? "—"}/D{selected.payload?.datatype ?? "—"}</strong
                >
              </div>
            {/if}
          </section>

          <section class="property-section">
            <h3>{t("gds.display")}</h3>
            <div class="property-field">
              <span>{t("gds.color")}</span>
              <div class="property-control color-control">
                <ColorPicker
                  label={t("gds.layerColor")}
                  value={display?.color ?? "#2D6CDF"}
                  onvaluechange={(color) => id && updateDisplay(id, { color })}
                />
                <code>{display?.color ?? "#2D6CDF"}</code>
                <button
                  class="reset-button"
                  disabled={!defaults?.color || display?.color === defaults.color}
                  title={t("gds.reset")}
                  aria-label={t("gds.reset")}
                  onclick={() =>
                    id && defaults?.color && updateDisplay(id, { color: defaults.color })}
                  ><RotateCcw size={14} /></button
                >
              </div>
            </div>
            <div class="property-field">
              <span>{t("gds.opacity")}</span>
              <div class="slider-control">
                <Slider
                  value={display?.opacity ?? 1}
                  min={0}
                  max={1}
                  step={0.05}
                  ariaLabel={t("gds.opacity")}
                  onvaluechange={(opacity) =>
                    id ? updateNumber(id, "opacity", opacity) : undefined}
                />
                <Input
                  class="number-input"
                  type="number"
                  value={display?.opacity ?? 1}
                  min={0}
                  max={1}
                  step={0.05}
                  hideNumberControls
                  onchange={(event) =>
                    id && updateNumber(id, "opacity", Number(event.currentTarget.value))}
                />
                <button
                  class="reset-button"
                  disabled={defaults?.opacity === undefined ||
                    display?.opacity === defaults.opacity}
                  title={t("gds.reset")}
                  aria-label={t("gds.reset")}
                  onclick={() =>
                    id &&
                    defaults?.opacity !== undefined &&
                    updateNumber(id, "opacity", defaults.opacity)}><RotateCcw size={14} /></button
                >
              </div>
            </div>
          </section>

          <section class="property-section">
            <h3>{t("gds.bounds")}</h3>
            <div class="property-field readonly">
              <span>{t("gds.xMin")}</span><strong
                >{selected.payload?.bounds?.min_x?.toFixed(4) ?? "—"}</strong
              >
            </div>
            <div class="property-field readonly">
              <span>{t("gds.xMax")}</span><strong
                >{selected.payload?.bounds?.max_x?.toFixed(4) ?? "—"}</strong
              >
            </div>
            <div class="property-field readonly">
              <span>{t("gds.yMin")}</span><strong
                >{selected.payload?.bounds?.min_y?.toFixed(4) ?? "—"}</strong
              >
            </div>
            <div class="property-field readonly">
              <span>{t("gds.yMax")}</span><strong
                >{selected.payload?.bounds?.max_y?.toFixed(4) ?? "—"}</strong
              >
            </div>
            <div class="property-field">
              <label for="property-z-min">{t("gds.zMin")}</label>
              <div class="slider-control">
                <Slider
                  value={display?.z_min ?? 0}
                  min={-100}
                  max={100}
                  step={0.1}
                  ariaLabel={t("gds.zMin")}
                  onvaluechange={(value) => (id ? updateNumber(id, "z_min", value) : undefined)}
                />
                <Input
                  id="property-z-min"
                  class="number-input z-number-input"
                  type="number"
                  value={display?.z_min ?? 0}
                  min={-100}
                  max={(display?.z_max ?? 100) - minimumZSpan}
                  step={0.1}
                  hideNumberControls
                  onchange={(event) =>
                    id && updateNumber(id, "z_min", Number(event.currentTarget.value))}
                />
                <button
                  class="reset-button"
                  disabled={defaults?.z_min === undefined || display?.z_min === defaults.z_min}
                  title={t("gds.reset")}
                  aria-label={t("gds.reset")}
                  onclick={() =>
                    id &&
                    defaults?.z_min !== undefined &&
                    updateNumber(id, "z_min", defaults.z_min)}><RotateCcw size={14} /></button
                >
              </div>
            </div>
            <div class="property-field">
              <label for="property-z-max">{t("gds.zMax")}</label>
              <div class="slider-control">
                <Slider
                  value={display?.z_max ?? 0}
                  min={-100}
                  max={100}
                  step={0.1}
                  ariaLabel={t("gds.zMax")}
                  onvaluechange={(value) => (id ? updateNumber(id, "z_max", value) : undefined)}
                />
                <Input
                  id="property-z-max"
                  class="number-input z-number-input"
                  type="number"
                  value={display?.z_max ?? 0}
                  min={(display?.z_min ?? -100) + minimumZSpan}
                  max={100}
                  step={0.1}
                  hideNumberControls
                  onchange={(event) =>
                    id && updateNumber(id, "z_max", Number(event.currentTarget.value))}
                />
                <button
                  class="reset-button"
                  disabled={defaults?.z_max === undefined || display?.z_max === defaults.z_max}
                  title={t("gds.reset")}
                  aria-label={t("gds.reset")}
                  onclick={() =>
                    id &&
                    defaults?.z_max !== undefined &&
                    updateNumber(id, "z_max", defaults.z_max)}><RotateCcw size={14} /></button
                >
              </div>
            </div>
          </section>
        </div>{:else if scene}<p class="empty">{t("gds.selectLayer")}</p>{:else}<p class="empty">
          {t("gds.noScene")}
        </p>{/if}
    </aside>
  </div>
</div>
{#if importCandidate}
  <ImportDialog
    info={importCandidate}
    {busy}
    onimport={confirmImport}
    oncancel={() => (importCandidate = null)}
  />
{/if}

{#if renameTarget}
  <Dialog
    bind:open={renameOpen}
    title={t("gds.rename")}
    closeLabel={t("gds.closeDialog")}
    width="360px"
  >
    <Input
      bind:input={renameInput}
      bind:value={renameValue}
      onkeydown={(event) => event.key === "Enter" && confirmRename()}
    />
    {#snippet footer()}
      <Button variant="ghost" onclick={() => (renameOpen = false)}>{t("gds.cancel")}</Button>
      <Button onclick={confirmRename}>{t("gds.confirm")}</Button>
    {/snippet}
  </Dialog>
{/if}

{#if contextMenu}
  {@const menu = contextMenu}
  <div
    class="tree-context-menu"
    role="menu"
    tabindex="-1"
    style={`left: ${contextMenu.x}px; top: ${contextMenu.y}px`}
  >
    {#if menu.kind === "blank"}
      <button role="menuitem" onclick={() => addBaseplate()}>{t("gds.addBaseplate")}</button>
    {:else}
      <button role="menuitem" onclick={() => addBaseplate(menu.group)}
        >{t("gds.addBaseplate")}</button
      >
      <div class="context-menu-separator"></div>
      <button role="menuitem" onclick={() => requestRename(menu.objectId)}>{t("gds.rename")}</button
      >
      <button class="danger" role="menuitem" onclick={() => deleteObject(menu.objectId)}
        >{t("gds.delete")}</button
      >
    {/if}
  </div>
{/if}

<style>
  .layout-page {
    min-width: 0;
    min-height: 0;
    display: grid;
    grid-template-rows: minmax(0, 1fr);
    padding: 8px;
    background: transparent;
  }
  .layout-workbench {
    min-width: 0;
    min-height: 0;
    display: grid;
    grid-template-columns:
      var(--layer-panel-width) var(--panel-resizer-width, 8px) minmax(0, 1fr)
      var(--panel-resizer-width, 8px) var(--properties-panel-width);
    overflow: hidden;
  }
  .layout-workbench.resizing {
    cursor: col-resize;
    user-select: none;
  }
  .layout-panel {
    min-width: 0;
    overflow: auto;
    padding: 16px;
    border: 1px solid var(--border);
    border-radius: var(--gds-radius-md);
    background: var(--surface);
    box-shadow: var(--card-shadow);
  }
  .panel-resizer {
    --resizer-grip-color: var(--muted);
    position: relative;
    min-width: 0;
    height: 100%;
    padding: 0;
    border: 0;
    background: transparent;
    cursor: col-resize;
    touch-action: none;
  }
  .resizer-grip {
    position: absolute;
    top: 50%;
    left: 50%;
    width: 2px;
    height: 2px;
    border-radius: var(--gds-radius-full);
    background: var(--resizer-grip-color);
    box-shadow:
      0 -4px var(--resizer-grip-color),
      0 4px var(--resizer-grip-color);
    opacity: 0.48;
    pointer-events: none;
    transform: translate(-50%, -50%);
    transition: opacity 0.14s ease;
  }
  .panel-resizer:hover .resizer-grip,
  .panel-resizer:focus-visible .resizer-grip,
  .panel-resizer.active .resizer-grip {
    --resizer-grip-color: var(--primary);
    opacity: 0.92;
  }
  .panel-resizer::after {
    content: "";
    position: absolute;
    top: 50%;
    left: 50%;
    width: 2px;
    height: 48px;
    border-radius: var(--gds-radius-full);
    background: var(--primary);
    opacity: 0;
    transform: translate(-50%, -50%) scaleY(0.55);
    transition:
      opacity 0.14s ease,
      transform 0.14s ease;
  }
  .panel-resizer:hover::after,
  .panel-resizer:focus-visible::after,
  .panel-resizer.active::after {
    opacity: 0.7;
    transform: translate(-50%, -50%) scaleY(1);
  }
  .panel-resizer:focus-visible {
    outline: none;
  }
  .panel-heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 14px;
  }
  .panel-heading h2 {
    margin: 0;
    font-size: 1rem;
  }
  .panel-heading > span {
    padding: 2px 7px;
    border-radius: 999px;
    color: var(--muted);
    background: var(--surface-soft);
    font-size: 0.78rem;
  }
  .empty {
    color: var(--muted);
  }
  .object-list {
    display: grid;
    gap: 5px;
  }
  .tree-root {
    width: 100%;
    min-height: 32px;
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 0 5px;
    border: 0;
    border-radius: inherit;
    color: var(--text);
    background: transparent;
    text-align: left;
    cursor: pointer;
  }
  .tree-root-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 28px;
    align-items: center;
    border-radius: var(--gds-radius-md);
  }
  .tree-root-row:hover {
    background: color-mix(in srgb, var(--text) 6%, transparent);
  }
  .tree-root :global(.tree-chevron) {
    flex: 0 0 auto;
    color: var(--muted);
  }
  .tree-root strong {
    overflow: hidden;
    font-size: 0.9rem;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tree-children {
    display: grid;
    gap: 2px;
    margin-left: 12px;
    padding-left: 8px;
  }
  .empty-actions {
    display: grid;
    gap: 8px;
  }
  .empty-actions :global(.ui-button) {
    width: 100%;
  }
  .object-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 28px;
    align-items: center;
    border-radius: var(--gds-radius-md);
  }
  .object-row.selected {
    background: color-mix(in srgb, var(--primary) 12%, transparent);
  }
  .object-row :global(.object-button) {
    width: 100%;
    justify-content: flex-start;
    overflow: hidden;
  }
  .baseplate-root :global(.object-button) {
    padding-left: 25px;
  }
  .visibility-button {
    width: 26px;
    height: 26px;
    display: grid;
    place-items: center;
    padding: 0;
    border: 0;
    border-radius: var(--gds-radius-xs);
    color: var(--muted);
    background: transparent;
    cursor: pointer;
  }
  .visibility-button:hover {
    color: var(--text);
    background: color-mix(in srgb, var(--text) 8%, transparent);
  }
  .visibility-button.visible {
    color: var(--primary);
  }
  .tree-context-menu {
    position: fixed;
    z-index: 900;
    min-width: 132px;
    padding: 4px;
    border: 1px solid var(--border);
    border-radius: var(--gds-radius-md);
    background: var(--overlay-surface);
    box-shadow: 0 12px 28px color-mix(in srgb, #000 24%, transparent);
  }
  .tree-context-menu button {
    width: 100%;
    min-height: 28px;
    padding: 0 9px;
    border: 0;
    border-radius: var(--gds-radius-xs);
    color: var(--text);
    background: transparent;
    text-align: left;
    cursor: pointer;
  }
  .tree-context-menu button:hover {
    background: color-mix(in srgb, var(--primary) 13%, transparent);
  }
  .tree-context-menu button.danger:hover {
    color: var(--danger);
    background: color-mix(in srgb, var(--danger) 12%, transparent);
  }
  .context-menu-separator {
    height: 1px;
    margin: 4px 3px;
    background: var(--border);
  }
  .viewport-panel {
    position: relative;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
    border: 1px solid var(--border);
    border-radius: var(--gds-radius-md);
    background: var(--surface);
    box-shadow: var(--card-shadow);
  }
  .viewport-resize-mask {
    position: absolute;
    z-index: 10;
    inset: 0;
    pointer-events: none;
    background: color-mix(in srgb, var(--surface) 20%, transparent);
    backdrop-filter: blur(28px);
    -webkit-backdrop-filter: blur(28px);
  }
  .viewport-empty {
    width: 100%;
    height: 100%;
    display: grid;
    place-content: center;
    justify-items: center;
    gap: 10px;
    color: var(--muted);
  }
  .property-grid {
    --property-reset-slot: 28px;
    display: grid;
    margin: 0 -16px;
  }
  .property-section {
    display: grid;
    gap: 9px;
    padding: 2px 16px 16px;
  }
  .property-section + .property-section {
    padding-top: 14px;
    border-top: 1px solid var(--border);
  }
  .property-section h3 {
    order: -2;
    margin: 0 0 3px;
    color: var(--text);
    font-size: 0.82rem;
    font-weight: 650;
  }
  .property-field {
    min-width: 0;
    display: grid;
    grid-template-columns: 76px minmax(0, 1fr);
    align-items: center;
    gap: 8px;
    min-height: 30px;
    color: var(--muted);
    font-size: 0.84rem;
  }
  .property-field.property-type {
    order: -1;
  }
  .property-field > label,
  .property-field > span {
    min-width: 0;
  }
  .property-field.readonly strong {
    min-width: 0;
    overflow: hidden;
    margin-right: var(--property-reset-slot);
    padding: 6px 8px;
    border: 1px solid transparent;
    border-radius: var(--gds-radius-xs);
    color: var(--text);
    background: var(--surface-soft);
    font-size: 0.82rem;
    font-weight: 500;
    text-align: right;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .property-control,
  .slider-control,
  .color-control {
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .property-control :global(.ui-input),
  .slider-control :global(.ui-input) {
    min-width: 0;
    height: 30px;
    min-height: 30px;
    padding: 0 8px;
    border-radius: var(--gds-radius-xs);
    font-size: 0.82rem;
    text-align: right;
  }
  .color-control code {
    min-width: 0;
    flex: 1;
    overflow: hidden;
    color: var(--text);
    font-family: inherit;
    font-size: 0.78rem;
    text-align: right;
    text-overflow: ellipsis;
  }
  .slider-control :global(.settings-slider) {
    min-width: 0;
    flex: 1;
  }
  .slider-control :global(.number-input) {
    width: 48px;
    flex: 0 0 48px;
    text-align: right;
  }
  .slider-control :global(.z-number-input) {
    width: 62px;
    flex-basis: 62px;
  }
  .reset-button {
    width: 28px;
    height: 28px;
    flex: 0 0 28px;
    display: grid;
    place-items: center;
    padding: 0;
    border: 0;
    border-radius: var(--gds-radius-xs);
    color: var(--muted);
    background: transparent;
    cursor: pointer;
  }
  .reset-button:hover:not(:disabled) {
    color: var(--text);
    background: color-mix(in srgb, var(--text) 8%, transparent);
  }
  .reset-button:disabled {
    opacity: 0.28;
    cursor: default;
  }
  .empty {
    line-height: 1.55;
  }
</style>
