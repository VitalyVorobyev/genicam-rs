import type { EnumEntry } from "../../xml_model/uigraph";
import { isTauri } from "../../tauri";
import { formatFrameCount } from "./viewerUtils";

interface AcquisitionSectionProps {
  isConnected: boolean;
  isAcquiring: boolean;
  frameCount: number;
  onStartAcq: () => Promise<void>;
  onStopAcq: () => Promise<void>;
  acquisitionModeEntries: EnumEntry[];
}

export function AcquisitionSection({
  isConnected,
  isAcquiring,
  frameCount,
  acquisitionModeEntries,
}: AcquisitionSectionProps) {
  if (!isConnected) {
    return <p className="sidebar-placeholder">No device connected.</p>;
  }

  const handleModeChange = async (e: React.ChangeEvent<HTMLSelectElement>) => {
    const newMode = e.target.value;
    if (!isTauri()) return;
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("write_node", { nodeName: "AcquisitionMode", value: newMode });
    } catch (err) {
      console.error("AcquisitionMode write failed:", err);
    }
  };

  const frameDisplay =
    !isAcquiring && frameCount === 0 ? "\u2014" : formatFrameCount(frameCount);

  return (
    <div className="acq-section acq-section--compact">
      {acquisitionModeEntries.length > 0 && (
        <div className="acq-compact-row">
          <span className="slider-row__name">Mode</span>
          <select
            className="iv-auto-select"
            disabled={isAcquiring}
            onChange={handleModeChange}
          >
            {acquisitionModeEntries.map((entry) => (
              <option key={entry.name} value={entry.name}>
                {entry.display_name ?? entry.name}
              </option>
            ))}
          </select>
        </div>
      )}
      <div className="acq-compact-row">
        <span className="slider-row__name">Frames</span>
        <span className="frame-counter__value">{frameDisplay}</span>
      </div>
    </div>
  );
}
