import { describe, it, expect } from "vitest";
import {
  computeFitScale,
  clampZoom,
  clampPan,
  zoomAroundPoint,
  formatZoomLabel,
  buildTransform,
  MAX_ZOOM,
} from "./zoomPanUtils";

// ── computeFitScale ────────────────────────────────────────────

describe("computeFitScale", () => {
  it("test_computeFitScale_no_upscale", () => {
    // Image smaller than box — scale capped at 1.0
    const result = computeFitScale(320, 240, 1920, 1080);
    expect(result).toBe(1);
  });

  it("test_computeFitScale_width_constrained", () => {
    // Image 1920×1080 in 960×1080 box — width constrains, scale = 0.5
    const result = computeFitScale(1920, 1080, 960, 1080);
    expect(result).toBeCloseTo(0.5, 6);
  });

  it("test_computeFitScale_height_constrained", () => {
    // Image 1920×1080 in 1920×540 box — height constrains, scale = 0.5
    const result = computeFitScale(1920, 1080, 1920, 540);
    expect(result).toBeCloseTo(0.5, 6);
  });

  it("test_computeFitScale_square_fit", () => {
    // Image 512×512 in 512×512 box — exactly 1.0
    const result = computeFitScale(512, 512, 512, 512);
    expect(result).toBe(1);
  });

  it("test_computeFitScale_zero_box", () => {
    // Guard against division by zero — returns 1
    const result = computeFitScale(640, 480, 0, 0);
    expect(result).toBe(1);
  });
});

// ── clampZoom ──────────────────────────────────────────────────

describe("clampZoom", () => {
  it("test_clampZoom_below_min", () => {
    // Scale below minScale is clamped up
    const fitScale = 0.25;
    const result = clampZoom(0.1, fitScale, MAX_ZOOM);
    expect(result).toBe(fitScale);
  });

  it("test_clampZoom_above_max", () => {
    // Scale above MAX_ZOOM is clamped down
    const result = clampZoom(20, 0.25, MAX_ZOOM);
    expect(result).toBe(MAX_ZOOM);
  });

  it("test_clampZoom_within_range", () => {
    // Scale within [min, MAX_ZOOM] passes through unchanged
    const result = clampZoom(2.0, 0.5, MAX_ZOOM);
    expect(result).toBe(2.0);
  });
});

// ── clampPan ───────────────────────────────────────────────────

describe("clampPan", () => {
  it("test_clampPan_at_fit_scale_returns_zero", () => {
    // At fit scale no pan is needed — any offset must be clamped to exactly zero
    const fitScale = computeFitScale(640, 480, 800, 600);
    const result = clampPan(999, 999, fitScale, 640, 480, 800, 600);
    expect(result.panX).toBe(0);
    expect(result.panY).toBe(0);
  });

  it("test_clampPan_excessive_pan_clamped", () => {
    // At 2× fit scale, excessive pan is bounded
    const fitScale = 0.5; // image 1920×1080 in 960×540 box
    const scale = fitScale * 2; // scale = 1.0
    const result = clampPan(10000, 10000, scale, 1920, 1080, 960, 540);
    // dispW = 1920 * 1.0 = 1920; maxPanX = (1920 - 40) / 2 = 940
    expect(result.panX).toBeCloseTo(940, 5);
    expect(result.panY).toBeCloseTo(520, 5);
  });

  it("test_clampPan_negative_pan_clamped", () => {
    // Negative (left/up) pan is also bounded symmetrically
    const fitScale = 0.5;
    const scale = fitScale * 2;
    const result = clampPan(-10000, -10000, scale, 1920, 1080, 960, 540);
    expect(result.panX).toBeCloseTo(-940, 5);
    expect(result.panY).toBeCloseTo(-520, 5);
  });
});

// ── zoomAroundPoint ────────────────────────────────────────────

describe("zoomAroundPoint", () => {
  it("test_zoomAroundPoint_from_center_no_pan_change", () => {
    // Zooming from the box centre (mouseX=0, mouseY=0) with pan at zero
    // leaves pan at zero regardless of scale change
    const result = zoomAroundPoint(1.0, 2.0, 0, 0, 0, 0, 800, 600);
    expect(result.panX).toBeCloseTo(0, 9);
    expect(result.panY).toBeCloseTo(0, 9);
  });

  it("test_zoomAroundPoint_from_corner_shifts_pan", () => {
    // Zooming from a non-centre point with existing pan shifts pan correctly.
    // currentScale=1, nextScale=2, panX=0, panY=0, mouseX=100, mouseY=50
    // newPanX = mouseX - (mouseX - panX) * (nextScale/currentScale)
    //         = 100 - (100 - 0) * 2 = 100 - 200 = -100
    // newPanY = 50 - (50 - 0) * 2 = 50 - 100 = -50
    const result = zoomAroundPoint(1.0, 2.0, 0, 0, 100, 50, 800, 600);
    expect(result.panX).toBeCloseTo(-100, 9);
    expect(result.panY).toBeCloseTo(-50, 9);
  });
});

// ── formatZoomLabel ────────────────────────────────────────────

describe("formatZoomLabel", () => {
  it("test_formatZoomLabel_at_fit_scale", () => {
    // scale === fitScale → "Fit"
    expect(formatZoomLabel(0.5, 0.5)).toBe("Fit");
  });

  it("test_formatZoomLabel_100_percent", () => {
    // scale = 1.0 → "100%"
    expect(formatZoomLabel(1.0, 0.5)).toBe("100%");
  });

  it("test_formatZoomLabel_rounds_fractional", () => {
    // scale = 0.756 → "76%"
    expect(formatZoomLabel(0.756, 0.25)).toBe("76%");
  });

  it("test_formatZoomLabel_near_fit_within_epsilon", () => {
    // |scale - fitScale| < 0.001 → "Fit"
    expect(formatZoomLabel(0.5009, 0.501)).toBe("Fit");
  });
});

// ── buildTransform ─────────────────────────────────────────────

describe("buildTransform", () => {
  it("test_buildTransform_at_fit_scale", () => {
    // scale === fitScale === 0.5, pan at origin → ratio is 1
    // transform must contain scale(1) and translate(-50%, -50%)
    const result = buildTransform(0, 0, 0.5, 0.5);
    expect(result).toContain("scale(1)");
    expect(result).toContain("translate(-50%, -50%)");
  });

  it("test_buildTransform_zero_fitScale_guard", () => {
    // fitScale === 0 would cause division-by-zero — guard falls back to ratio 1
    // result must not contain Infinity or NaN
    const result = buildTransform(0, 0, 1.0, 0);
    expect(result).not.toMatch(/Infinity|NaN/);
    expect(result).toContain("scale(1)");
  });
});
