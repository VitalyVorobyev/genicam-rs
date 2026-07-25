/**
 * ROI (Region of Interest) utility types and pure functions for the Image Viewer.
 *
 * An ImageRect describes a rectangular ROI in image-pixel coordinates:
 *   x → OffsetX (left edge),  y → OffsetY (top edge)
 *   w → Width,                h → Height
 *
 * All values are non-negative integers. w/h may be zero for a degenerate (empty) rect.
 */

/** Axis-aligned rectangle in image-pixel coordinates. */
export interface ImageRect {
  x: number; // OffsetX — left edge (pixels from image left)
  y: number; // OffsetY — top edge  (pixels from image top)
  w: number; // Width
  h: number; // Height
}

/**
 * Build a normalised ImageRect from two arbitrary image-space points.
 *
 * The result always has non-negative w and h regardless of drag direction.
 *
 * @param a  First image-space point (drag start).
 * @param b  Second image-space point (drag end).
 */
export function buildImageRect(
  a: { x: number; y: number },
  b: { x: number; y: number },
): ImageRect {
  const x = Math.min(a.x, b.x);
  const y = Math.min(a.y, b.y);
  const w = Math.abs(b.x - a.x);
  const h = Math.abs(b.y - a.y);
  return { x: Math.round(x), y: Math.round(y), w: Math.round(w), h: Math.round(h) };
}

/**
 * Clamp an ImageRect so it does not extend beyond the image boundaries.
 *
 * - x/y are clamped to [0, imgW-1] / [0, imgH-1].
 * - w/h are reduced so that x+w ≤ imgW and y+h ≤ imgH.
 * - If x ≥ imgW or y ≥ imgH the result has w=0 or h=0.
 */
export function clampRoiToImage(roi: ImageRect, imgW: number, imgH: number): ImageRect {
  const x = Math.max(0, Math.min(roi.x, imgW - 1));
  const y = Math.max(0, Math.min(roi.y, imgH - 1));
  const w = Math.max(0, Math.min(roi.w, imgW - x));
  const h = Math.max(0, Math.min(roi.h, imgH - y));
  return { x, y, w, h };
}

/**
 * Returns true when the ROI has a non-zero area (can be applied to the camera).
 */
export function roiIsValid(roi: ImageRect): boolean {
  return roi.w > 0 && roi.h > 0;
}

/**
 * Format an ROI as a human-readable string.
 * Example: { x:0, y:0, w:640, h:480 } → "640 × 480 @ (0, 0)"
 */
export function formatRoiLabel(roi: ImageRect): string {
  return `${roi.w} \u00d7 ${roi.h} @ (${roi.x}, ${roi.y})`;
}
