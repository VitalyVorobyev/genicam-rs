import { describe, it, expect } from "vitest";
import { formatPacketSizeLabel, formatInterPacketDelayLabel } from "./transportLayerUtils";

describe("formatPacketSizeLabel", () => {
  it("test_formatPacketSizeLabel_typical", () => {
    expect(formatPacketSizeLabel(1500)).toBe("1500 B");
  });

  it("test_formatPacketSizeLabel_zero", () => {
    expect(formatPacketSizeLabel(0)).toBe("0 B");
  });
});

describe("formatInterPacketDelayLabel", () => {
  it("test_formatInterPacketDelayLabel_zero", () => {
    expect(formatInterPacketDelayLabel(0)).toBe("0 ticks");
  });

  it("test_formatInterPacketDelayLabel_typical", () => {
    expect(formatInterPacketDelayLabel(5000)).toBe("5000 ticks");
  });
});
