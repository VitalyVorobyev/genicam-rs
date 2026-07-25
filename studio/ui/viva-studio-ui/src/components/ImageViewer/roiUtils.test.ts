import { describe, it, expect } from "vitest";
import {
  buildImageRect,
  clampRoiToImage,
  roiIsValid,
  formatRoiLabel,
} from "./roiUtils";

describe("buildImageRect", () => {
  it("test_buildImageRect_normal", () => {
    // Forward drag: top-left to bottom-right.
    const rect = buildImageRect({ x: 10, y: 20 }, { x: 110, y: 70 });
    expect(rect).toEqual({ x: 10, y: 20, w: 100, h: 50 });
  });

  it("test_buildImageRect_reversed", () => {
    // Backward drag (bottom-right to top-left) → same normalised result.
    const rect = buildImageRect({ x: 110, y: 70 }, { x: 10, y: 20 });
    expect(rect).toEqual({ x: 10, y: 20, w: 100, h: 50 });
  });

  it("test_buildImageRect_single_point", () => {
    // Degenerate drag: both points are the same → w=0, h=0.
    const rect = buildImageRect({ x: 50, y: 50 }, { x: 50, y: 50 });
    expect(rect).toEqual({ x: 50, y: 50, w: 0, h: 0 });
  });

  it("test_buildImageRect_diagonal_reversed_x_only", () => {
    // Only x is reversed.
    const rect = buildImageRect({ x: 200, y: 0 }, { x: 100, y: 80 });
    expect(rect).toEqual({ x: 100, y: 0, w: 100, h: 80 });
  });
});

describe("clampRoiToImage", () => {
  it("test_clampRoiToImage_fits", () => {
    // ROI entirely within a 640×480 image → unchanged.
    const roi = { x: 10, y: 10, w: 100, h: 100 };
    expect(clampRoiToImage(roi, 640, 480)).toEqual(roi);
  });

  it("test_clampRoiToImage_clipped_right", () => {
    // ROI extends past the right edge → w is reduced.
    const roi = { x: 600, y: 0, w: 100, h: 50 };
    const result = clampRoiToImage(roi, 640, 480);
    expect(result.x).toBe(600);
    expect(result.w).toBe(40); // 640 - 600
  });

  it("test_clampRoiToImage_clipped_bottom", () => {
    // ROI extends past the bottom edge → h is reduced.
    const roi = { x: 0, y: 460, w: 50, h: 100 };
    const result = clampRoiToImage(roi, 640, 480);
    expect(result.y).toBe(460);
    expect(result.h).toBe(20); // 480 - 460
  });

  it("test_clampRoiToImage_clipped_fully_outside_x", () => {
    // ROI starts beyond the right edge: x is clamped to imgW-1=639, leaving w=1.
    const roi = { x: 700, y: 0, w: 100, h: 100 };
    const result = clampRoiToImage(roi, 640, 480);
    expect(result.x).toBe(639);
    expect(result.w).toBe(1); // only 1 pixel remains at the last column
  });

  it("test_clampRoiToImage_negative_x_clamped_to_zero", () => {
    // Negative x is clamped to 0, w adjusted.
    const roi = { x: -10, y: 0, w: 50, h: 50 };
    const result = clampRoiToImage(roi, 640, 480);
    expect(result.x).toBe(0);
    expect(result.w).toBe(50); // w not affected by x clamp (still fits)
  });
});

describe("roiIsValid", () => {
  it("test_roiIsValid_true", () => {
    expect(roiIsValid({ x: 0, y: 0, w: 100, h: 50 })).toBe(true);
  });

  it("test_roiIsValid_false_zero_width", () => {
    expect(roiIsValid({ x: 0, y: 0, w: 0, h: 50 })).toBe(false);
  });

  it("test_roiIsValid_false_zero_height", () => {
    expect(roiIsValid({ x: 0, y: 0, w: 100, h: 0 })).toBe(false);
  });

  it("test_roiIsValid_false_both_zero", () => {
    expect(roiIsValid({ x: 0, y: 0, w: 0, h: 0 })).toBe(false);
  });
});

describe("formatRoiLabel", () => {
  it("test_formatRoiLabel", () => {
    expect(formatRoiLabel({ x: 0, y: 0, w: 640, h: 480 })).toBe(
      "640 \u00d7 480 @ (0, 0)",
    );
  });

  it("test_formatRoiLabel_with_offset", () => {
    expect(formatRoiLabel({ x: 100, y: 50, w: 320, h: 240 })).toBe(
      "320 \u00d7 240 @ (100, 50)",
    );
  });
});
