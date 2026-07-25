import { describe, it, expect } from "vitest";
import type { UiNode } from "../../xml_model/uigraph";
import type { NodeValueEntry } from "../../device/types";
import {
  extractNumericConstraints,
  clampValue,
  formatExposureLabel,
  formatGainLabel,
  resolveAutoState,
  isAutoContinuous,
} from "./exposureGainUtils";

// Minimal UiNode fixture builder
function makeNode(constraints?: { min?: number; max?: number; inc?: number }): UiNode {
  return {
    name: "TestNode",
    kind: "Float",
    constraints,
    raw: { tag: "Float", attributes: { Name: "TestNode" }, children_text: {} },
  };
}

// ── extractNumericConstraints ─────────────────────────────────────────────────

describe("extractNumericConstraints", () => {
  it("test_extractNumericConstraints_undefined_node_returns_defaults", () => {
    const result = extractNumericConstraints(undefined, { min: 100, max: 100000, inc: 1 });
    expect(result).toEqual({ min: 100, max: 100000, inc: 1 });
  });

  it("test_extractNumericConstraints_full_constraints_present", () => {
    const node = makeNode({ min: 50, max: 50000, inc: 10 });
    const result = extractNumericConstraints(node, { min: 100, max: 100000, inc: 1 });
    expect(result).toEqual({ min: 50, max: 50000, inc: 10 });
  });

  it("test_extractNumericConstraints_partial_min_only", () => {
    const node = makeNode({ min: 200 });
    const result = extractNumericConstraints(node, { min: 100, max: 100000, inc: 1 });
    expect(result).toEqual({ min: 200, max: 100000, inc: 1 });
  });

  it("test_extractNumericConstraints_no_constraints_field", () => {
    const node = makeNode(undefined);
    const result = extractNumericConstraints(node, { min: 100, max: 100000, inc: 1 });
    expect(result).toEqual({ min: 100, max: 100000, inc: 1 });
  });

  it("test_extractNumericConstraints_inc_absent_defaults_to_zero", () => {
    const node = makeNode({ min: 10, max: 1000 });
    const result = extractNumericConstraints(node, { min: 0, max: 500 });
    expect(result.inc).toBe(0);
  });
});

// ── clampValue ────────────────────────────────────────────────────────────────

describe("clampValue", () => {
  it("test_clampValue_within_range", () => {
    expect(clampValue(500, 100, 1000)).toBe(500);
  });

  it("test_clampValue_below_min", () => {
    expect(clampValue(50, 100, 1000)).toBe(100);
  });

  it("test_clampValue_above_max", () => {
    expect(clampValue(2000, 100, 1000)).toBe(1000);
  });

  it("test_clampValue_exactly_min", () => {
    expect(clampValue(100, 100, 1000)).toBe(100);
  });

  it("test_clampValue_exactly_max", () => {
    expect(clampValue(1000, 100, 1000)).toBe(1000);
  });
});

// ── formatExposureLabel ───────────────────────────────────────────────────────

describe("formatExposureLabel", () => {
  it("test_formatExposureLabel_integer_value", () => {
    expect(formatExposureLabel(10000)).toBe("10000.0 µs");
  });

  it("test_formatExposureLabel_fractional_value", () => {
    expect(formatExposureLabel(10000.5)).toBe("10000.5 µs");
  });

  it("test_formatExposureLabel_zero", () => {
    expect(formatExposureLabel(0)).toBe("0.0 µs");
  });
});

// ── formatGainLabel ───────────────────────────────────────────────────────────

describe("formatGainLabel", () => {
  it("test_formatGainLabel_integer", () => {
    expect(formatGainLabel(12)).toBe("12.00 dB");
  });

  it("test_formatGainLabel_fractional", () => {
    expect(formatGainLabel(1.5)).toBe("1.50 dB");
  });

  it("test_formatGainLabel_zero", () => {
    expect(formatGainLabel(0)).toBe("0.00 dB");
  });
});

// ── resolveAutoState ──────────────────────────────────────────────────────────

describe("resolveAutoState", () => {
  it("test_resolveAutoState_node_present_string_value", () => {
    const map = new Map<string, NodeValueEntry>([
      ["ExposureAuto", { value: "Continuous", access_mode: "RW" }],
    ]);
    expect(resolveAutoState(map, "ExposureAuto")).toBe("Continuous");
  });

  it("test_resolveAutoState_node_absent", () => {
    const map = new Map<string, NodeValueEntry>();
    expect(resolveAutoState(map, "ExposureAuto")).toBeNull();
  });

  it("test_resolveAutoState_node_value_is_number", () => {
    const map = new Map<string, NodeValueEntry>([
      ["ExposureAuto", { value: 42, access_mode: "RW" }],
    ]);
    expect(resolveAutoState(map, "ExposureAuto")).toBeNull();
  });
});

// ── isAutoContinuous ──────────────────────────────────────────────────────────

describe("isAutoContinuous", () => {
  it("test_isAutoContinuous_continuous", () => {
    expect(isAutoContinuous("Continuous")).toBe(true);
  });

  it("test_isAutoContinuous_off", () => {
    expect(isAutoContinuous("Off")).toBe(false);
  });

  it("test_isAutoContinuous_null", () => {
    expect(isAutoContinuous(null)).toBe(false);
  });
});
