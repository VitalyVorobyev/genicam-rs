export { extractNumericConstraints, clampValue } from "./exposureGainUtils";
export { resolveLiveEnumValue } from "./triggerUtils";

/**
 * Format a balance ratio value with two decimal places. No unit suffix —
 * balance ratios are dimensionless.
 * Example: 1.5 → "1.50"
 */
export function formatBalanceRatioLabel(v: number): string {
  return v.toFixed(2);
}

/**
 * Format a gamma value with two decimal places. No unit suffix — gamma is
 * dimensionless.
 * Example: 1.0 → "1.00"
 */
export function formatGammaLabel(v: number): string {
  return v.toFixed(2);
}
