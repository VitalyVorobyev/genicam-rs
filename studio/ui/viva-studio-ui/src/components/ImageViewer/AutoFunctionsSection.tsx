import type { ParseXmlResponse } from "../../xml_model/uigraph";
import type { NodeValueEntry } from "../../device/types";
import { isTauri } from "../../tauri";
import { resolveLiveEnumValue } from "./triggerUtils";

interface AutoFunctionsSectionProps {
  isConnected: boolean;
  externalModel: ParseXmlResponse | null;
  liveValues: Map<string, NodeValueEntry>;
}

async function writeNode(nodeName: string, value: number | string): Promise<void> {
  if (!isTauri()) return;
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("write_node", { nodeName, value });
  } catch (err) {
    console.error(`write_node(${nodeName}) failed:`, err);
  }
}

export function AutoFunctionsSection({
  isConnected,
  externalModel,
  liveValues,
}: AutoFunctionsSectionProps) {
  if (!isConnected) {
    return <p className="sidebar-placeholder">No device connected.</p>;
  }

  const nodes = externalModel?.graph.nodes_by_name;

  const exposureAutoEntries = nodes?.["ExposureAuto"]?.enum_entries ?? [];
  const gainAutoEntries = nodes?.["GainAuto"]?.enum_entries ?? [];
  const balanceWhiteAutoEntries = nodes?.["BalanceWhiteAuto"]?.enum_entries ?? [];

  const exposureAutoValue = resolveLiveEnumValue(liveValues, "ExposureAuto");
  const gainAutoValue = resolveLiveEnumValue(liveValues, "GainAuto");
  const balanceWhiteAutoValue = resolveLiveEnumValue(liveValues, "BalanceWhiteAuto");

  const handleExposureAutoChange = async (e: React.ChangeEvent<HTMLSelectElement>) => {
    await writeNode("ExposureAuto", e.target.value);
  };

  const handleGainAutoChange = async (e: React.ChangeEvent<HTMLSelectElement>) => {
    await writeNode("GainAuto", e.target.value);
  };

  const handleBalanceWhiteAutoChange = async (e: React.ChangeEvent<HTMLSelectElement>) => {
    await writeNode("BalanceWhiteAuto", e.target.value);
  };

  return (
    <div className="exp-gain-section">
      {exposureAutoEntries.length > 0 && (
        <div className="if-enum-row">
          <span className="slider-row__name">Exposure Auto</span>
          <select
            className="iv-auto-select"
            value={exposureAutoValue ?? ""}
            onChange={handleExposureAutoChange}
          >
            {exposureAutoEntries.map((entry) => (
              <option key={entry.name} value={entry.name}>
                {entry.display_name ?? entry.name}
              </option>
            ))}
          </select>
        </div>
      )}

      {gainAutoEntries.length > 0 && (
        <div className="if-enum-row">
          <span className="slider-row__name">Gain Auto</span>
          <select
            className="iv-auto-select"
            value={gainAutoValue ?? ""}
            onChange={handleGainAutoChange}
          >
            {gainAutoEntries.map((entry) => (
              <option key={entry.name} value={entry.name}>
                {entry.display_name ?? entry.name}
              </option>
            ))}
          </select>
        </div>
      )}

      {balanceWhiteAutoEntries.length > 0 && (
        <div className="if-enum-row">
          <span className="slider-row__name">White Balance Auto</span>
          <select
            className="iv-auto-select"
            value={balanceWhiteAutoValue ?? ""}
            onChange={handleBalanceWhiteAutoChange}
          >
            {balanceWhiteAutoEntries.map((entry) => (
              <option key={entry.name} value={entry.name}>
                {entry.display_name ?? entry.name}
              </option>
            ))}
          </select>
        </div>
      )}
    </div>
  );
}
