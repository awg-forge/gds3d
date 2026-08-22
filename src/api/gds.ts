import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";

export interface Bounds2d {
  min_x: number;
  min_y: number;
  max_x: number;
  max_y: number;
}
export interface GdsLayerSelection {
  cell_name: string;
  layer: number;
  datatype: number;
}
export interface GdsLayerInfo {
  selection: GdsLayerSelection;
  polygon_count: number;
  bounds: Bounds2d;
}
export interface GdsCellInfo {
  name: string;
  layers: GdsLayerInfo[];
}
export interface GdsFileInfo {
  file_path: string;
  cells: GdsCellInfo[];
}
export interface SceneSnapshot {
  revision: number;
  objects: unknown[];
  occurrences: RenderObjectOccurrences[];
}
export interface Occurrence {
  root_cell: number;
  leaf_cell: number;
  instance_path: number[];
  shape_id: number;
}
export interface RenderObjectOccurrences {
  objectId: string;
  occurrences: Occurrence[];
}
export interface OccurrenceInspection {
  cell_name: string;
  shape_id: number;
  shape_type: string;
  layer: number;
  datatype: number;
  instance_path: number[];
  hierarchy_path: string[];
}
export type ViewExportFormat = "png" | "glb" | "stl";
export type ViewExportQuality = "low" | "standard" | "high" | "ultra";
export interface ViewCapture {
  dataUrl: string;
  width: number;
  height: number;
}
export interface ViewExportSettings {
  format: ViewExportFormat;
  width: number;
  height: number;
  quality?: ViewExportQuality;
}

export function inspectGdsFile(path: string): Promise<GdsFileInfo> {
  return invoke("inspect_gds_file", { path });
}
export function importGds(path: string, selections: GdsLayerSelection[]): Promise<SceneSnapshot> {
  return invoke("import_gds", { path, selections });
}
export function getSceneSnapshot(): Promise<SceneSnapshot> {
  return invoke("scene_snapshot");
}
export function inspectOccurrence(occurrence: Occurrence): Promise<OccurrenceInspection> {
  return invoke("inspect_occurrence", { occurrence });
}
export function clearScene(): Promise<SceneSnapshot> {
  return invoke("clear_scene");
}
export function updateObjectDisplay(update: {
  objectId: string;
  name?: string;
  color?: string;
  opacity?: number;
  visible?: boolean;
  zMin?: number;
  zMax?: number;
}): Promise<void> {
  return invoke("update_object_display", { update });
}
export function setObjectsVisibility(objectIds: string[], visible: boolean): Promise<void> {
  return invoke("set_objects_visibility", { objectIds, visible });
}
export function createBaseplate(target?: {
  filePath: string;
  cellName: string;
}): Promise<SceneSnapshot> {
  return invoke("create_baseplate", { target });
}
export function deleteSceneObject(objectId: string): Promise<SceneSnapshot> {
  return invoke("delete_scene_object", { objectId });
}
export function undoScene(): Promise<SceneSnapshot> {
  return invoke("undo_scene");
}
export function redoScene(): Promise<SceneSnapshot> {
  return invoke("redo_scene");
}
export function saveProject(path: string): Promise<void> {
  return invoke("save_project", { path });
}
export function loadProject(path: string): Promise<SceneSnapshot> {
  return invoke("load_project", { path });
}
export async function exportView(path: string, capture: ViewCapture): Promise<void> {
  await invoke("export_view", { path, capture });
}
export function exportModel(path: string, dataUrl: string): Promise<void> {
  return invoke("export_model", { path, dataUrl });
}

export async function chooseGdsPath(): Promise<string | null> {
  const selected = await open({
    multiple: false,
    filters: [{ name: "GDSII layout", extensions: ["gds", "gdsii"] }],
  });
  return typeof selected === "string" ? selected : null;
}

export async function chooseProjectPath(): Promise<string | null> {
  const selected = await open({
    multiple: false,
    filters: [{ name: "gds3d project", extensions: ["gds3d"] }],
  });
  return typeof selected === "string" ? selected : null;
}

export function chooseProjectSavePath(): Promise<string | null> {
  return save({
    defaultPath: "gds3d-project.gds3d",
    filters: [{ name: "gds3d project", extensions: ["gds3d"] }],
  });
}

export async function chooseViewExportPath(
  format: ViewExportFormat,
  quality?: ViewExportQuality,
): Promise<string | null> {
  const descriptions: Record<ViewExportFormat, string> = {
    png: "PNG image",
    glb: "glTF binary model",
    stl: "STL model",
  };
  const qualitySuffix = format === "png" && quality ? `-${quality}` : "";
  const selected = await save({
    defaultPath: `gds3d-view${qualitySuffix}.${format}`,
    filters: [{ name: descriptions[format], extensions: [format] }],
  });
  if (!selected) return null;
  return selected.toLowerCase().endsWith(`.${format}`) ? selected : `${selected}.${format}`;
}
