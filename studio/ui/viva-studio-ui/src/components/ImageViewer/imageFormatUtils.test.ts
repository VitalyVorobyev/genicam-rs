import { describe, it, expect } from "vitest";
import type { NumericConstraintsResolved } from "./exposureGainUtils";
import { resolveIntStep, formatPixelDimension, deriveOffsetMax } from "./imageFormatUtils";

// ── resolveIntStep ────────────────────────────────────────────────────────────

describe("resolveIntStep", () => {
  it("test_resolveIntStep_positive_inc", () => {
    const c: NumericConstraintsResolved = { min: 0, max: 4096, inc: 4 };
    expect(resolveIntStep(c)).toBe(4);
  });

  it("test_resolveIntStep_zero_inc", () => {
    const c: NumericConstraintsResolved = { min: 0, max: 4096, inc: 0 };
    expect(resolveIntStep(c)).toBe(1);
  });

  it("test_resolveIntStep_undefined", () => {
    expect(resolveIntStep(undefined)).toBe(1);
  });
});

// ── formatPixelDimension ──────────────────────────────────────────────────────

describe("formatPixelDimension", () => {
  it("test_formatPixelDimension_typical", () => {
    expect(formatPixelDimension(1920)).toBe("1920 px");
  });

  it("test_formatPixelDimension_zero", () => {
    expect(formatPixelDimension(0)).toBe("0 px");
  });
});

// ── deriveOffsetMax ───────────────────────────────────────────────────────────

describe("deriveOffsetMax", () => {
  it("test_deriveOffsetMax_from_live", () => {
    // SensorWidth=4096, Width=1920 → max=2176
    expect(deriveOffsetMax(4096, 1920, 4095)).toBe(2176);
  });

  it("test_deriveOffsetMax_clamp_zero", () => {
    // roi > sensor → clamp to 0
    expect(deriveOffsetMax(1920, 4096, 4095)).toBe(0);
  });

  it("test_deriveOffsetMax_null_sensor_fallback", () => {
    expect(deriveOffsetMax(null, 1920, 4095)).toBe(4095);
  });

  it("test_deriveOffsetMax_null_roi_fallback", () => {
    expect(deriveOffsetMax(4096, null, 4095)).toBe(4095);
  });
});
