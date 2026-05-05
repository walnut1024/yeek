// VendorProxy — automatic in-app updates from GitHub Releases.
// Thin wrapper over @tauri-apps/plugin-updater.

import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import type { Update } from "@tauri-apps/plugin-updater";

export type { Update };

/**
 * Silently check for an available update.
 * Returns null when no update is found or the check fails (no throw).
 */
export async function checkForUpdate(): Promise<Update | null> {
  try {
    const update = await check();
    return update ?? null;
  } catch {
    return null;
  }
}

/**
 * Download the update with progress callbacks, then install and relaunch.
 *
 * Progress callback receives (downloaded_bytes, total_bytes_or_null).
 * On completion calls update.install() then relaunch() — the app exits
 * and restarts with the new version automatically.
 */
export async function downloadAndInstall(
  update: Update,
  onProgress: (downloaded: number, total: number | null) => void,
): Promise<void> {
  let downloaded = 0;
  let total: number | null = null;

  await update.download((event) => {
    switch (event.event) {
      case "Started":
        total = event.data.contentLength ?? null;
        onProgress(0, total);
        break;
      case "Progress":
        downloaded += event.data.chunkLength;
        onProgress(downloaded, total);
        break;
      case "Finished":
        break;
    }
  });

  await update.install();
  await relaunch();
}
