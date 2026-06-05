import { describe, expect, it } from "vitest";
import { formatResumeError } from "../resume-error";

describe("formatResumeError", () => {
  it("uses the error message without the JavaScript Error prefix", () => {
    expect(formatResumeError(new Error("Validation error: Invalid working directory: /tmp/missing"))).toBe(
      "Validation error: Invalid working directory: /tmp/missing",
    );
  });

  it("stringifies non-error values", () => {
    expect(formatResumeError("failed")).toBe("failed");
  });
});
