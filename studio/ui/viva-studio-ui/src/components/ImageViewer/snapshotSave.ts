/**
 * Save a PNG blob to disk.
 *
 * In Tauri mode: shows a native file-save dialog via plugin-dialog, then
 * writes the bytes via plugin-fs. Both plugins are imported dynamically so
 * they are never bundled into the browser/WASM build.
 *
 * In browser mode: triggers a download via a temporary <a download> link.
 */
import { isTauri } from "../../tauri";

export async function saveSnapshot(
  blob: Blob,
  suggestedName: string,
): Promise<void> {
  if (isTauri()) {
    const { save } = await import("@tauri-apps/plugin-dialog");
    const { writeFile } = await import("@tauri-apps/plugin-fs");

    const path = await save({
      defaultPath: suggestedName,
      filters: [{ name: "PNG Image", extensions: ["png"] }],
    });

    if (!path) {
      // User cancelled the dialog — silent no-op
      return;
    }

    const bytes = new Uint8Array(await blob.arrayBuffer());
    await writeFile(path, bytes);
    return;
  }

  // Browser fallback: trigger a download link
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = suggestedName;
  a.style.display = "none";
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}
