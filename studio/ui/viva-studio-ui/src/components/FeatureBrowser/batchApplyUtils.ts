import type { NodeValue, ValueError } from "../../xml_model/values";

/**
 * Count the number of draft entries that have no validation errors and can
 * therefore be applied to the device.
 *
 * @param drafts  The current draft-value map (name → value).
 * @param errors  The current validation-error map (name → errors[]).
 */
export function countApplicableDrafts(
  drafts: Record<string, NodeValue>,
  errors: Record<string, ValueError[]>,
): number {
  return Object.keys(drafts).filter(
    (name) => (errors[name] ?? []).length === 0,
  ).length;
}

/**
 * Format a human-readable batch-apply progress string.
 *
 * Example: formatBatchProgress(3, 5) → "Applying 3 of 5…"
 *
 * @param done   Number of writes completed so far.
 * @param total  Total number of writes in the batch.
 */
export function formatBatchProgress(done: number, total: number): string {
  return `Applying ${done} of ${total}\u2026`;
}
