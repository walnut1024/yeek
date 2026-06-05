export type TerminalPlatform = "macos" | "linux" | "windows";

export interface TerminalOption {
  label: string;
  value: string;
}

const OPTIONS: Record<TerminalPlatform, TerminalOption[]> = {
  macos: [
    { label: "Ghostty", value: "Ghostty" },
    { label: "iTerm2", value: "iTerm2" },
    { label: "Terminal.app", value: "Terminal.app" },
    { label: "cmux", value: "cmux" },
    { label: "Warp", value: "Warp" },
    { label: "WezTerm", value: "WezTerm" },
    { label: "kitty", value: "kitty" },
    { label: "Alacritty", value: "Alacritty" },
  ],
  linux: [
    { label: "Ghostty", value: "ghostty" },
    { label: "WezTerm", value: "wezterm" },
    { label: "kitty", value: "kitty" },
    { label: "Alacritty", value: "alacritty" },
    { label: "GNOME Terminal", value: "gnome-terminal" },
    { label: "Konsole", value: "konsole" },
    { label: "Xfce Terminal", value: "xfce4-terminal" },
    { label: "XTerm", value: "xterm" },
  ],
  windows: [
    { label: "Windows Terminal", value: "wt.exe" },
    { label: "PowerShell 7", value: "pwsh.exe" },
    { label: "Windows PowerShell", value: "powershell.exe" },
    { label: "Command Prompt", value: "cmd.exe" },
  ],
};

export function terminalOptionsForPlatform(platform: TerminalPlatform): TerminalOption[] {
  return OPTIONS[platform];
}

export function currentTerminalPlatform(): TerminalPlatform {
  const userAgent = navigator.userAgent.toLowerCase();
  if (userAgent.includes("win")) return "windows";
  if (userAgent.includes("linux")) return "linux";
  return "macos";
}
