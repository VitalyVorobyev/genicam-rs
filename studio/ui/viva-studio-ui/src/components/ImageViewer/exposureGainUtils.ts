import type { UiNode } from "../../xml_model/uigraph";
import type { NodeValueEntry } from "../../device/types";

export interface NumericConstraintsResolved {
  min: number;
  max: number;
  inc: number;
}

/**
 * Extract numeric constraints from a UiNode, falling back to provided defaults
 * for any missing field. `inc` defaults to 0 when absent from both node and defaults.
 */
export function extractNumericConstraints(
  node: UiNode | undefined,
  defaults: { min: number; max: number; inc?: number },
): NumericConstraintsResolved {
  const c = node?.constraints;
  return {
    min: c?.min ?? defaults.min,
    max: c?.max ?? defaults.max,
    inc: c?.inc ?? defaults.inc ?? 0,
  };
}

/**
 * Clamp `value` to the inclusive range [min, max].
 */
export function clampValue(value: number, min: number, max: number): number {
  if (value < min) return min;
  if (value > max) return max;
  return value;
}

/**
 * Format an exposure time value with one decimal place and a µs suffix.
 * Example: 10000 → "10000.0 µs"
 */
export function formatExposureLabel(v: number): string {
  return `${v.toFixed(1)} µs`;
}

/**
 * Format a gain value with two decimal places and a dB suffix.
 * Example: 1.5 → "1.50 dB"
 */
export function formatGainLabel(v: number): string {
  return `${v.toFixed(2)} dB`;
}

/**
 * Return the string value of an auto node from liveValues, or null if the
 * entry is absent or its value is not a string.
 */
export function resolveAutoState(
  liveValues: Map<string, NodeValueEntry>,
  nodeName: string,
): string | null {
  const entry = liveValues.get(nodeName);
  if (entry === undefined) return null;
  if (typeof entry.value !== "string") return null;
  return entry.value;
}

/**
 * Return true only when the auto state is exactly "Continuous".
 */
export function isAutoContinuous(state: string | null): boolean {
  return state === "Continuous";
}
