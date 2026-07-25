import { describe, it, expect } from "vitest";
import { formatToastDuration } from "./ToastContext";

describe("formatToastDuration", () => {
  it("test_formatToastDuration_info", () => {
    expect(formatToastDuration("info")).toBe(3000);
  });

  it("test_formatToastDuration_success", () => {
    expect(formatToastDuration("success")).toBe(3000);
  });

  it("test_formatToastDuration_warning", () => {
    expect(formatToastDuration("warning")).toBe(5000);
  });

  it("test_formatToastDuration_error", () => {
    expect(formatToastDuration("error")).toBe(6000);
  });

  it("test_formatToastDuration_error_longer_than_warning", () => {
    // Error toasts stay visible longer than warnings.
    expect(formatToastDuration("error")).toBeGreaterThan(formatToastDuration("warning"));
  });
});
