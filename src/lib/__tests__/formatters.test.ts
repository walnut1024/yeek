import { describe, it, expect } from "vitest";
import { truncate } from "../../pages/sessions/graph/build-tree";

describe("build-tree helpers", () => {
  describe("truncate", () => {
    it("returns empty string for falsy input", () => {
      expect(truncate("")).toBe("");
    });

    it("returns short text unchanged", () => {
      expect(truncate("hello")).toBe("hello");
    });

    it("truncates at default 48 chars", () => {
      const input = "a".repeat(60);
      expect(truncate(input)).toBe("a".repeat(48) + "\u2026");
    });

    it("truncates at custom length", () => {
      const input = "a".repeat(20);
      expect(truncate(input, 10)).toBe("aaaaaaaaaa\u2026");
    });

    it("collapses newlines", () => {
      expect(truncate("hello\nworld")).toBe("hello world");
    });

    it("trims whitespace", () => {
      expect(truncate("  hello  ")).toBe("hello");
    });
  });
});
