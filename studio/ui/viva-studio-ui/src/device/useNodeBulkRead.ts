import { useCallback } from "react";
import { isTauri } from "../tauri";
import type { NodeValueEntry } from "./types";

export function useNodeBulkRead() {
  const readBulk = useCallback(
    async (names: string[]): Promise<Map<string, NodeValueEntry>> => {
      if (!isTauri() || names.length === 0) return new Map();
      const { invoke } = await import("@tauri-apps/api/core");
      const result = await invoke<Record<string, NodeValueEntry>>("read_nodes_bulk", { names });
      return new Map(Object.entries(result));
    },
    []
  );
  return { readBulk };
}
