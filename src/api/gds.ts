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
}
export type ViewExportFormat = "png" | "svg" | "glb" | "stl";
export interface ViewCapture {
  dataUrl: string;
  width: number;
  height: number;
}
export interface ViewExportSettings {
  format: ViewExportFormat;
  width: number;
  height: number;
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
export function saveProject(path: string): Promise<void> {
  return invoke("save_project", { path });
}
export function loadProject(path: string): Promise<SceneSnapshot> {
  return invoke("load_project", { path });
}
export function exportView(
  path: string,
  format: ViewExportFormat,
  capture: ViewCapture,
): Promise<void> {
  return invoke("export_view", { path, format, capture });
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

export async function chooseViewExportPath(format: ViewExportFormat): Promise<string | null> {
  const descriptions: Record<ViewExportFormat, string> = {
    png: "PNG image",
    svg: "SVG image",
    glb: "glTF binary model",
    stl: "STL model",
  };
  const selected = await save({
    defaultPath: `gds3d-view.${format}`,
    filters: [{ name: descriptions[format], extensions: [format] }],
  });
  if (!selected) return null;
  return selected.toLowerCase().endsWith(`.${format}`) ? selected : `${selected}.${format}`;
}
