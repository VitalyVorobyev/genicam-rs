import type { UiNode } from "./uigraph";
import type { NodeValue, ValueError } from "./values";

// Client-side validation keeps the UX responsive in offline mode and mirrors
// the checks we expect from a future live backend. This keeps drafts honest
// without duplicating parsing or mutating the UiGraph contract.
export function validateDraft(node: UiNode, value: NodeValue): ValueError[] {
  if (value === null || value === undefined) {
    return [];
  }

  if (typeof node.kind !== "string") {
    return [];
  }

  switch (node.kind) {
    case "Integer":
      return validateInteger(node, value);
    case "Float":
      return validateFloat(node, value);
    case "Boolean":
      return validateBoolean(value);
    case "String":
      return validateString(value);
    case "Enumeration":
      return validateEnum(node, value);
    default:
      return [];
  }
}

function validateInteger(node: UiNode, value: NodeValue): ValueError[] {
  const errors: ValueError[] = [];
  if (typeof value !== "number" || !Number.isFinite(value)) {
    errors.push({ message: "Value must be a number." });
    return errors;
  }
  if (!Number.isInteger(value)) {
    errors.push({ message: "Value must be an integer." });
  }

  applyNumericConstraints(node, value, errors, true);
  return errors;
}

function validateFloat(node: UiNode, value: NodeValue): ValueError[] {
  const errors: ValueError[] = [];
  if (typeof value !== "number" || !Number.isFinite(value)) {
    errors.push({ message: "Value must be a number." });
    return errors;
  }

  applyNumericConstraints(node, value, errors, false);
  return errors;
}

function validateBoolean(value: NodeValue): ValueError[] {
  if (typeof value !== "boolean") {
    return [{ message: "Value must be true or false." }];
  }
  return [];
}

function validateString(value: NodeValue): ValueError[] {
  if (typeof value !== "string") {
    return [{ message: "Value must be a string." }];
  }
  return [];
}

function validateEnum(node: UiNode, value: NodeValue): ValueError[] {
  if (!isEnumValue(value)) {
    return [{ message: "Value must be a valid enumeration entry." }];
  }

  const entries = node.enum_entries ?? [];
  if (entries.length === 0) {
    return [{ message: "No enumeration entries are available." }];
  }

  const match = entries.find((entry) => entry.name === value.enumName);
  if (!match) {
    return [{ message: "Value must match one of the enumeration entries." }];
  }

  return [];
}

function applyNumericConstraints(
  node: UiNode,
  value: number,
  errors: ValueError[],
  enforceInc: boolean
) {
  const { min, max, inc } = node.constraints ?? {};
  if (min !== undefined && value < min) {
    errors.push({ message: `Value must be ≥ ${min}.` });
  }
  if (max !== undefined && value > max) {
    errors.push({ message: `Value must be ≤ ${max}.` });
  }

  if (enforceInc && inc !== undefined && inc !== 0) {
    const base = min ?? 0;
    if (!isAlignedToIncrement(value, base, inc)) {
      errors.push({ message: `Value must align to increment ${inc}.` });
    }
  }
}

function isAlignedToIncrement(value: number, base: number, inc: number): boolean {
  const delta = (value - base) / inc;
  const nearest = Math.round(delta);
  return Math.abs(delta - nearest) < 1e-6;
}

function isEnumValue(value: NodeValue): value is { enumName: string } {
  return (
    typeof value === "object" &&
    value !== null &&
    "enumName" in value &&
    typeof (value as { enumName?: unknown }).enumName === "string"
  );
}
