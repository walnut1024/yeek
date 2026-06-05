import { describe, expect, it } from "vitest";
import { terminalOptionsForPlatform } from "../terminal-options";

describe("terminalOptionsForPlatform", () => {
  it("returns macOS terminal options in product-name order", () => {
    expect(terminalOptionsForPlatform("macos").map((option) => option.label)).toEqual([
      "Ghostty",
      "iTerm2",
      "Terminal.app",
      "cmux",
      "Warp",
      "WezTerm",
      "kitty",
      "Alacritty",
    ]);
  });

  it("returns Linux terminal options without macOS or Windows apps", () => {
    expect(terminalOptionsForPlatform("linux").map((option) => option.label)).toEqual([
      "Ghostty",
      "WezTerm",
      "kitty",
      "Alacritty",
      "GNOME Terminal",
      "Konsole",
      "Xfce Terminal",
      "XTerm",
    ]);
  });

  it("returns Windows terminal options with official display names", () => {
    expect(terminalOptionsForPlatform("windows").map((option) => option.label)).toEqual([
      "Windows Terminal",
      "PowerShell 7",
      "Windows PowerShell",
      "Command Prompt",
    ]);
  });
});
