import { describe, it, expect } from "vitest";
import { formatBackendChip } from "./backendStatus";

describe("formatBackendChip", () => {
  it("test_remote_mode — a loaded Zenoh config reads as Remote", () => {
    const chip = formatBackendChip({ mode: "Remote", zenoh_config_error: null });
    expect(chip.label).toBe("Remote");
    expect(chip.state).toBe("remote");
  });

  it("test_embedded_mode — no ZENOH_CONFIG reads as a deliberate Embedded", () => {
    const chip = formatBackendChip({ mode: "Embedded", zenoh_config_error: null });
    expect(chip.label).toBe("Embedded");
    expect(chip.state).toBe("embedded");
    // The tooltip has to name the trap behind #132, not just the mode.
    expect(chip.title).toContain("127.0.0.1");
  });

  it("test_embedded_after_config_failure — a failed ZENOH_CONFIG reads as degraded", () => {
    const error = "Failed to load ZENOH_CONFIG=/tmp/nope.json5: No such file";
    const chip = formatBackendChip({ mode: "Embedded", zenoh_config_error: error });
    expect(chip.label).toBe("Embedded");
    // Not "embedded": the user asked for remote and did not get it.
    expect(chip.state).toBe("degraded");
    expect(chip.title).toBe(error);
  });
});
