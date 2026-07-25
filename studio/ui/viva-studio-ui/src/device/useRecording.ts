import { useCallback, useEffect, useRef, useState } from "react";
import { isTauri } from "../tauri";

export interface RecordingStatus {
  active: boolean;
  file_path: string | null;
  frame_count: number;
  bytes_written: number;
  elapsed_secs: number;
}

export function useRecording() {
  const [status, setStatus] = useState<RecordingStatus>({
    active: false,
    file_path: null,
    frame_count: 0,
    bytes_written: 0,
    elapsed_secs: 0,
  });
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const startRecording = useCallback(async () => {
    if (!isTauri()) return;
    const { invoke } = await import("@tauri-apps/api/core");
    const result = await invoke<RecordingStatus>("start_recording");
    setStatus(result);
  }, []);

  const stopRecording = useCallback(async () => {
    if (!isTauri()) return;
    const { invoke } = await import("@tauri-apps/api/core");
    const result = await invoke<RecordingStatus>("stop_recording");
    setStatus(result);
  }, []);

  // Poll recording status while active
  useEffect(() => {
    if (!status.active || !isTauri()) return;

    const poll = async () => {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        const s = await invoke<RecordingStatus>("get_recording_status");
        setStatus(s);
      } catch { /* */ }
    };

    pollRef.current = setInterval(poll, 500);
    return () => {
      if (pollRef.current) clearInterval(pollRef.current);
    };
  }, [status.active]);

  return { recordingStatus: status, startRecording, stopRecording };
}
