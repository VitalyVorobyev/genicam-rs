import { useCallback, useEffect, useState } from "react";
import { isTauri } from "../tauri";
import type { AcquisitionStatus, ConnectionState, StreamerInfo } from "./types";

export function useAcquisition() {
  const [status, setStatus] = useState<AcquisitionStatus>({
    active: false,
    fps: null,
    dropped: 0,
  });
  const [streamerInfo, setStreamerInfo] = useState<StreamerInfo | null>(null);

  useEffect(() => {
    if (!isTauri()) return;

    let unlistenStatus: (() => void) | null = null;
    let unlistenConnection: (() => void) | null = null;
    let cancelled = false;

    import("@tauri-apps/api/event").then(({ listen }) => {
      if (cancelled) return;
      listen<AcquisitionStatus>("acquisition-status", (e) => {
        setStatus(e.payload);
      }).then((fn) => {
        if (cancelled) fn();
        else unlistenStatus = fn;
      });

      listen<ConnectionState>("connection-state-changed", (e) => {
        const kind = e.payload.kind;
        if (kind === "disconnected" || kind === "error") {
          setStreamerInfo(null);
        }
      }).then((fn) => {
        if (cancelled) fn();
        else unlistenConnection = fn;
      });
    });

    return () => {
      cancelled = true;
      unlistenStatus?.();
      unlistenConnection?.();
    };
  }, []);

  const start = useCallback(async (): Promise<StreamerInfo> => {
    const { invoke } = await import("@tauri-apps/api/core");
    const info = await invoke<StreamerInfo>("start_acquisition");
    setStreamerInfo(info);
    return info;
  }, []);

  const stop = useCallback(async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("stop_acquisition");
  }, []);

  return { status, streamerInfo, start, stop };
}
