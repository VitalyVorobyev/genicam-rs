import type { FitContainResult } from "./viewerUtils";

export const MAX_ZOOM = 16;

/**
 * Compute the fit-contain scale for an image inside a box.
 * Never upscales: scale is capped at 1.0.
 * Returns 1 when either dimension is zero to avoid division-by-zero.
 */
export function computeFitScale(
  imgW: number,
  imgH: number,
  boxW: number,
  boxH: number,
): number {
  if (imgW <= 0 || imgH <= 0 || boxW <= 0 || boxH <= 0) return 1;
  return Math.min(boxW / imgW, boxH / imgH, 1);
}

/**
 * Clamp a zoom scale to [min, MAX_ZOOM].
 */
export function clampZoom(scale: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, scale));
}

/**
 * Clamp pan offsets so that at least 40 px of the image remains visible
 * on each edge. At fit scale (scale ≈ fitScale) clamps to (0, 0).
 *
 * panX / panY are CSS-space offsets applied after centering the canvas.
 * At fit scale the canvas already fills to contain, so no pan is needed.
 */
export function clampPan(
  panX: number,
  panY: number,
  scale: number,
  imgW: number,
  imgH: number,
  boxW: number,
  boxH: number,
): { panX: number; panY: number } {
  const fitScale = computeFitScale(imgW, imgH, boxW, boxH);
  if (Math.abs(scale - fitScale) < 1e-9) return { panX: 0, panY: 0 };
  const ratio = fitScale > 0 ? scale / fitScale : 1;

  // Displayed canvas dimensions in CSS pixels
  const fitW = imgW * fitScale;
  const fitH = imgH * fitScale;
  const dispW = fitW * ratio;
  const dispH = fitH * ratio;

  // Maximum allowed pan so that at least 40 px of image edge stays visible
  const MARGIN = 40;
  const maxPanX = Math.max(0, (dispW - MARGIN) / 2);
  const maxPanY = Math.max(0, (dispH - MARGIN) / 2);

  return {
    panX: Math.max(-maxPanX, Math.min(maxPanX, panX)),
    panY: Math.max(-maxPanY, Math.min(maxPanY, panY)),
  };
}

/**
 * Adjust pan after a scale change so that the point under the mouse stays fixed.
 *
 * mouseX / mouseY are coordinates relative to the centre of the wrap box
 * (i.e. already shifted so that (0,0) is the box centre).
 */
export function zoomAroundPoint(
  currentScale: number,
  nextScale: number,
  panX: number,
  panY: number,
  mouseX: number,
  mouseY: number,
  _boxW: number,
  _boxH: number,
): { panX: number; panY: number } {
  if (currentScale === 0) return { panX, panY };
  const scaleDelta = nextScale / currentScale;
  // The point under the cursor in "canvas space" before zoom:
  // canvasPoint = (mouse - pan) / currentScale
  // After zoom we want: mouse = pan' + canvasPoint * nextScale
  // => pan' = mouse - canvasPoint * nextScale
  //         = mouse - (mouse - pan) * scaleDelta
  const newPanX = mouseX - (mouseX - panX) * scaleDelta;
  const newPanY = mouseY - (mouseY - panY) * scaleDelta;
  return { panX: newPanX, panY: newPanY };
}

/**
 * Format a zoom label for display in the toolbar.
 * Returns "Fit" when scale is within 0.001 of fitScale, else "N%".
 */
export function formatZoomLabel(scale: number, fitScale: number): string {
  if (Math.abs(scale - fitScale) < 0.001) return "Fit";
  return `${Math.round(scale * 100)}%`;
}

/**
 * Compute the CSS transform string for the canvas element.
 * Uses centre-anchored positioning: translate(-50%,-50%) then pan then scale.
 */
export function buildTransform(
  panX: number,
  panY: number,
  scale: number,
  fitScale: number,
): string {
  const s = fitScale > 0 ? scale / fitScale : 1;
  return `translate(-50%, -50%) translate(${panX}px, ${panY}px) scale(${s})`;
}

/**
 * Re-export FitContainResult so hook consumers can import from one place.
 */
export type { FitContainResult };
