import { invoke } from "@tauri-apps/api/core";

export interface Bounds2d { min_x: number; min_y: number; max_x: number; max_y: number }
export interface GdsLayerSelection { cell_name: string; layer: number; datatype: number }
export interface GdsLayerInfo { selection: GdsLayerSelection; polygon_count: number; bounds: Bounds2d }
export interface GdsCellInfo { name: string; layers: GdsLayerInfo[] }
export interface GdsFileInfo { file_path: string; cells: GdsCellInfo[] }
export interface SceneSnapshot { revision: number; objects: unknown[] }

export function inspectGdsFile(path: string): Promise<GdsFileInfo> { return invoke("inspect_gds_file", { path }); }
export function importGds(path: string): Promise<SceneSnapshot> { return invoke("import_gds", { path }); }
export function getSceneSnapshot(): Promise<SceneSnapshot> { return invoke("scene_snapshot"); }
export function updateObjectDisplay(update: { objectId: string; color?: string; brightness?: number; visible?: boolean; zMin?: number; zMax?: number }): Promise<SceneSnapshot> { return invoke("update_object_display", { update }); }
export function saveProject(path: string): Promise<void> { return invoke("save_project", { path }); }
