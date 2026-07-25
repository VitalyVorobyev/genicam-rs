import { useEffect, useState } from "react";
import { isTauri } from "../tauri";
import type { ImageMeta, ConnectionState } from "./types";

export function useImageMeta() {
  const [imageMeta, setImageMeta] = useState<ImageMeta | null>(null);

  useEffect(() => {
    if (!isTauri()) return;

    let unlistenMeta: (() => void) | null = null;
    let unlistenConn: (() => void) | null = null;
    let cancelled = false;

    import("@tauri-apps/api/event").then(({ listen }) => {
      if (cancelled) return;

      listen<ImageMeta>("image-meta-changed", (e) => {
        setImageMeta(e.payload);
      }).then((fn) => {
        if (cancelled) fn();
        else unlistenMeta = fn;
      });

      listen<ConnectionState>("connection-state-changed", (e) => {
        const kind = e.payload.kind;
        if (kind === "disconnected" || kind === "error") {
          setImageMeta(null);
        }
      }).then((fn) => {
        if (cancelled) fn();
        else unlistenConn = fn;
      });
    });

    return () => {
      cancelled = true;
      unlistenMeta?.();
      unlistenConn?.();
    };
  }, []);

  return { imageMeta };
}
