export function isTauri(): boolean {
  return typeof window.__TAURI_INTERNALS__ !== "undefined";
}

export async function ping(): Promise<string> {
  const { invoke } = await import("@tauri-apps/api/core");
  return await invoke<string>("ping");
}
