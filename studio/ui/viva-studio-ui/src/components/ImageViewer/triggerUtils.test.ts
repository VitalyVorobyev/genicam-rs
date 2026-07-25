import { describe, it, expect } from "vitest";
import type { NodeValueEntry } from "../../device/types";
import { formatTriggerDelayLabel, resolveLiveEnumValue } from "./triggerUtils";

// ── formatTriggerDelayLabel ───────────────────────────────────────────────────

describe("formatTriggerDelayLabel", () => {
  it("test_formatTriggerDelayLabel_integer", () => {
    expect(formatTriggerDelayLabel(10000)).toBe("10000.0 µs");
  });

  it("test_formatTriggerDelayLabel_fractional", () => {
    expect(formatTriggerDelayLabel(123.456)).toBe("123.5 µs");
  });

  it("test_formatTriggerDelayLabel_zero", () => {
    expect(formatTriggerDelayLabel(0)).toBe("0.0 µs");
  });

  it("test_formatTriggerDelayLabel_large", () => {
    expect(formatTriggerDelayLabel(1000000)).toBe("1000000.0 µs");
  });
});

// ── resolveLiveEnumValue ──────────────────────────────────────────────────────

describe("resolveLiveEnumValue", () => {
  it("test_resolveLiveEnumValue_string_present", () => {
    const map = new Map<string, NodeValueEntry>([
      ["TriggerMode", { value: "On", access_mode: "RW" }],
    ]);
    expect(resolveLiveEnumValue(map, "TriggerMode")).toBe("On");
  });

  it("test_resolveLiveEnumValue_absent", () => {
    const map = new Map<string, NodeValueEntry>();
    expect(resolveLiveEnumValue(map, "TriggerMode")).toBeNull();
  });

  it("test_resolveLiveEnumValue_number_value", () => {
    const map = new Map<string, NodeValueEntry>([
      ["TriggerMode", { value: 1, access_mode: "RW" }],
    ]);
    expect(resolveLiveEnumValue(map, "TriggerMode")).toBeNull();
  });
});
