export { extractNumericConstraints, clampValue } from "./exposureGainUtils";
export { resolveIntStep } from "./imageFormatUtils";

/** Format a packet size value with a " B" suffix. Example: 1500 → "1500 B" */
export function formatPacketSizeLabel(v: number): string {
  return `${v} B`;
}

/** Format an inter-packet delay value with a " ticks" suffix. Example: 100 → "100 ticks" */
export function formatInterPacketDelayLabel(v: number): string {
  return `${v} ticks`;
}
