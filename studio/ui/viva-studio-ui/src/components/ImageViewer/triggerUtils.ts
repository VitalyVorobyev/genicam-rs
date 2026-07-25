export { extractNumericConstraints, clampValue } from "./exposureGainUtils";
import type { NodeValueEntry } from "../../device/types";

/**
 * Format a trigger delay value with one decimal place and a µs suffix.
 * Example: 10000 → "10000.0 µs"
 */
export function formatTriggerDelayLabel(v: number): string {
  return `${v.toFixed(1)} µs`;
}

/**
 * Return the string value of a node from liveValues, or null if the entry is
 * absent or its value is not a string.
 */
export function resolveLiveEnumValue(
  liveValues: Map<string, NodeValueEntry>,
  nodeName: string,
): string | null {
  const entry = liveValues.get(nodeName);
  if (entry === undefined) return null;
  if (typeof entry.value !== "string") return null;
  return entry.value;
}
