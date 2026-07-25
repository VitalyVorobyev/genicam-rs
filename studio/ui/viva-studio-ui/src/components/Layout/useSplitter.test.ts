import { describe, it, expect } from "vitest";
import { clampSplitter } from "./useSplitter";

describe("clampSplitter", () => {
  it("test_clampSplitter_within_bounds", () => {
    expect(clampSplitter(300, 100, 600)).toBe(300);
  });

  it("test_clampSplitter_below_min", () => {
    expect(clampSplitter(50, 100, 600)).toBe(100);
  });

  it("test_clampSplitter_above_max", () => {
    expect(clampSplitter(700, 100, 600)).toBe(600);
  });

  it("test_clampSplitter_at_min", () => {
    expect(clampSplitter(100, 100, 600)).toBe(100);
  });

  it("test_clampSplitter_at_max", () => {
    expect(clampSplitter(600, 100, 600)).toBe(600);
  });

  it("test_clampSplitter_negative_value_clamped_to_min", () => {
    expect(clampSplitter(-50, 160, 420)).toBe(160);
  });
});
