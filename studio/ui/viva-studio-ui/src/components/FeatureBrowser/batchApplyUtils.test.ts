import { describe, it, expect } from "vitest";
import { countApplicableDrafts, formatBatchProgress } from "./batchApplyUtils";

// ── countApplicableDrafts ─────────────────────────────────────────────────────

describe("countApplicableDrafts", () => {
  it("test_countApplicableDrafts_no_drafts", () => {
    expect(countApplicableDrafts({}, {})).toBe(0);
  });

  it("test_countApplicableDrafts_all_valid", () => {
    const drafts = { Width: 640, Height: 480, Gain: 1.5 };
    const errors = {};
    expect(countApplicableDrafts(drafts, errors)).toBe(3);
  });

  it("test_countApplicableDrafts_some_invalid", () => {
    const drafts = { Width: 640, Height: 0, Gain: 1.5 };
    const errors = { Height: [{ message: "Value out of range" }] };
    expect(countApplicableDrafts(drafts, errors)).toBe(2);
  });

  it("test_countApplicableDrafts_all_invalid", () => {
    const drafts = { Width: 0, Height: 0 };
    const errors = {
      Width:  [{ message: "Too small" }],
      Height: [{ message: "Too small" }],
    };
    expect(countApplicableDrafts(drafts, errors)).toBe(0);
  });

  it("test_countApplicableDrafts_empty_error_array_counts_as_valid", () => {
    // An empty errors array means no errors — the draft is applicable.
    const drafts = { Width: 640 };
    const errors = { Width: [] };
    expect(countApplicableDrafts(drafts, errors)).toBe(1);
  });
});

// ── formatBatchProgress ───────────────────────────────────────────────────────

describe("formatBatchProgress", () => {
  it("test_formatBatchProgress_start", () => {
    expect(formatBatchProgress(0, 5)).toBe("Applying 0 of 5\u2026");
  });

  it("test_formatBatchProgress_middle", () => {
    expect(formatBatchProgress(3, 5)).toBe("Applying 3 of 5\u2026");
  });

  it("test_formatBatchProgress_done", () => {
    expect(formatBatchProgress(5, 5)).toBe("Applying 5 of 5\u2026");
  });

  it("test_formatBatchProgress_single_item", () => {
    expect(formatBatchProgress(1, 1)).toBe("Applying 1 of 1\u2026");
  });
});
