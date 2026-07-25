export { extractNumericConstraints, clampValue } from "./exposureGainUtils";
import type { NumericConstraintsResolved } from "./exposureGainUtils";

/**
 * Return the integer step from constraints.inc when it is positive, otherwise 1.
 * Ensures slider step is always a valid positive integer.
 */
export function resolveIntStep(constraints: NumericConstraintsResolved | undefined): number {
  if (constraints === undefined) return 1;
  return constraints.inc > 0 ? constraints.inc : 1;
}

/**
 * Format an integer pixel dimension with a "px" suffix.
 * Example: 1920 → "1920 px"
 */
export function formatPixelDimension(v: number): string {
  return `${v} px`;
}

/**
 * Derive the effective maximum for an offset axis.
 *
 * When both live sensor dimension and live ROI dimension are known, the max is
 * `max(0, sensorDim - roiDim)` — mirroring the camera service MS-12 logic.
 * Falls back to `staticMax` when either live value is absent.
 */
export function deriveOffsetMax(
  sensorDim: number | null,
  roiDim: number | null,
  staticMax: number,
): number {
  if (sensorDim === null || roiDim === null) return staticMax;
  const computed = sensorDim - roiDim;
  return computed > 0 ? computed : 0;
}
