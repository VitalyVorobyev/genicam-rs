import { describe, it, expect } from "vitest";
import { formatBalanceRatioLabel, formatGammaLabel } from "./colorProcessingUtils";

// ── formatBalanceRatioLabel ───────────────────────────────────────────────────

describe("formatBalanceRatioLabel", () => {
  it("test_formatBalanceRatioLabel_typical", () => {
    expect(formatBalanceRatioLabel(1.5)).toBe("1.50");
  });

  it("test_formatBalanceRatioLabel_integer", () => {
    expect(formatBalanceRatioLabel(1)).toBe("1.00");
  });

  it("test_formatBalanceRatioLabel_zero", () => {
    expect(formatBalanceRatioLabel(0)).toBe("0.00");
  });

  it("test_formatBalanceRatioLabel_max", () => {
    expect(formatBalanceRatioLabel(4.0)).toBe("4.00");
  });

  it("test_formatBalanceRatioLabel_negative", () => {
    expect(formatBalanceRatioLabel(-0.5)).toBe("-0.50");
  });
});

// ── formatGammaLabel ──────────────────────────────────────────────────────────

describe("formatGammaLabel", () => {
  it("test_formatGammaLabel_one", () => {
    expect(formatGammaLabel(1)).toBe("1.00");
  });

  it("test_formatGammaLabel_fractional", () => {
    expect(formatGammaLabel(2.5)).toBe("2.50");
  });

  it("test_formatGammaLabel_zero_point_one", () => {
    expect(formatGammaLabel(0.1)).toBe("0.10");
  });

  it("test_formatGammaLabel_negative", () => {
    expect(formatGammaLabel(-1)).toBe("-1.00");
  });
});
