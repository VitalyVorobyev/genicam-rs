import { describe, it, expect, beforeEach } from "vitest";

// Minimal localStorage mock — must be set up before importing the module
// so that module-level code that reads localStorage sees the mock.
const store: Record<string, string> = {};
const localStorageMock = {
  getItem: (key: string): string | null => store[key] ?? null,
  setItem: (key: string, value: string): void => {
    store[key] = value;
  },
  removeItem: (key: string): void => {
    delete store[key];
  },
  clear: (): void => {
    for (const k of Object.keys(store)) delete store[k];
  },
};

Object.defineProperty(globalThis, "localStorage", {
  value: localStorageMock,
  writable: true,
});

import { buildStorageKey, readStoredSections } from "./useSectionPersistence";

describe("useSectionPersistence helpers", () => {
  beforeEach(() => {
    localStorageMock.clear();
  });

  it("test_build_storage_key_with_model", () => {
    expect(buildStorageKey("acA1920")).toBe("iv-sections-acA1920");
  });

  it("test_build_storage_key_null_model", () => {
    expect(buildStorageKey(null)).toBe("iv-sections-__default__");
  });

  it("test_read_stored_sections_empty", () => {
    expect(readStoredSections("acA1920")).toEqual({});
  });

  it("test_read_stored_sections_valid", () => {
    localStorageMock.setItem(
      "iv-sections-acA1920",
      JSON.stringify({ acquisition_control: true, image_format: false }),
    );
    expect(readStoredSections("acA1920")).toEqual({
      acquisition_control: true,
      image_format: false,
    });
  });

  it("test_read_stored_sections_malformed_json", () => {
    localStorageMock.setItem("iv-sections-acA1920", "not-json");
    expect(readStoredSections("acA1920")).toEqual({});
  });

  it("test_read_stored_sections_non_object", () => {
    localStorageMock.setItem("iv-sections-acA1920", "42");
    expect(readStoredSections("acA1920")).toEqual({});
  });
});
