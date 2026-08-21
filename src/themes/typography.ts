export const DEFAULT_FONT_SIZE = 14;
export const MIN_FONT_SIZE = 12;
export const MAX_FONT_SIZE = 20;

const DEFAULT_FONT_FAMILY = 'Inter, "PingFang SC", "Microsoft YaHei", sans-serif';

function fontStack(fontFamily: string, fallback: string): string {
  const family = fontFamily.trim();
  const escapedFamily = family.replace(/\\/g, "\\\\").replace(/"/g, '\\"');
  return family ? `"${escapedFamily}", ${fallback}` : fallback;
}

export function applyTypography(fontSize: number, fontFamily: string): void {
  const size = Math.min(MAX_FONT_SIZE, Math.max(MIN_FONT_SIZE, fontSize || DEFAULT_FONT_SIZE));

  document.documentElement.style.fontSize = `${size}px`;
  document.documentElement.style.setProperty(
    "--app-font-family",
    fontStack(fontFamily, DEFAULT_FONT_FAMILY),
  );
}
