export const DEFAULT_FONT_SIZE = 14;
export const MIN_FONT_SIZE = 12;
export const MAX_FONT_SIZE = 20;

const fallbackFontFamily = 'Inter, "PingFang SC", "Microsoft YaHei", sans-serif';

export function clampFontSize(value: number): number {
  return Math.min(MAX_FONT_SIZE, Math.max(MIN_FONT_SIZE, value || DEFAULT_FONT_SIZE));
}

export function applyTypography(fontSize: number, fontFamily: string): void {
  const selectedFamily = fontFamily.trim();
  const escapedFamily = selectedFamily.replace(/\\/g, "\\\\").replace(/"/g, '\\"');
  const family = selectedFamily ? `"${escapedFamily}", ${fallbackFontFamily}` : fallbackFontFamily;

  document.documentElement.style.fontSize = `${clampFontSize(fontSize)}px`;
  document.documentElement.style.setProperty("--app-font-family", family);
}

export function readFontSize(): number {
  const value = Number(localStorage.getItem("gds3d.font-size"));
  return Number.isFinite(value) ? clampFontSize(value) : DEFAULT_FONT_SIZE;
}

export function readFontFamily(): string {
  return localStorage.getItem("gds3d.font-family") ?? "";
}
