import { describe, it, expect } from "vitest";
import { nodeKindCssKey, nodeKindIcon } from "./helpers";

describe("nodeKindCssKey", () => {
  it("test_nodeKindCssKey_integer", () => {
    expect(nodeKindCssKey("Integer")).toBe("integer");
  });

  it("test_nodeKindCssKey_float", () => {
    expect(nodeKindCssKey("Float")).toBe("float");
  });

  it("test_nodeKindCssKey_boolean", () => {
    expect(nodeKindCssKey("Boolean")).toBe("boolean");
  });

  it("test_nodeKindCssKey_string", () => {
    expect(nodeKindCssKey("String")).toBe("string");
  });

  it("test_nodeKindCssKey_enumeration", () => {
    expect(nodeKindCssKey("Enumeration")).toBe("enum");
  });

  it("test_nodeKindCssKey_command", () => {
    expect(nodeKindCssKey("Command")).toBe("command");
  });

  it("test_nodeKindCssKey_register", () => {
    expect(nodeKindCssKey("Register")).toBe("register");
  });

  it("test_nodeKindCssKey_category", () => {
    expect(nodeKindCssKey("Category")).toBe("category");
  });

  it("test_nodeKindCssKey_unknown_object", () => {
    // UiNodeKind can be an object { Unknown: { tag: string } }
    expect(nodeKindCssKey({ Unknown: { tag: "SomeTag" } })).toBe("unknown");
  });

  it("test_nodeKindCssKey_unrecognised_string", () => {
    // String kinds not in the switch fall through to "unknown"
    expect(nodeKindCssKey("Foobar" as never)).toBe("unknown");
  });
});

describe("nodeKindIcon", () => {
  it("test_nodeKindIcon_integer", () => {
    expect(nodeKindIcon("Integer")).toBe("#");
  });

  it("test_nodeKindIcon_command", () => {
    expect(nodeKindIcon("Command")).toBe(">");
  });

  it("test_nodeKindIcon_enumeration", () => {
    expect(nodeKindIcon("Enumeration")).toBe("=");
  });

  it("test_nodeKindIcon_unknown", () => {
    // Unknown kind object → middle dot
    expect(nodeKindIcon({ Unknown: { tag: "X" } })).toBe("\u00B7");
  });

  it("test_nodeKindIcon_category", () => {
    expect(nodeKindIcon("Category")).toBe("\u25A6");
  });
});
