import type { FeatureState } from "../../../device/types";

interface CommandViewProps {
  canExecute: boolean;
  disabledReason: string;
  onExecute: () => void;
  /**
   * Live device state for the command node. When `access_mode` is `"RO"` or
   * `"NA"` the command is gated off regardless of `canExecute`, so the user
   * does not fire a click that the device will silently reject.
   */
  liveState?: FeatureState;
}

// Command nodes are not executable in offline mode; the button stays disabled
// until a live provider implements executeCommand.
export function CommandView({
  canExecute,
  disabledReason,
  onExecute,
  liveState,
}: CommandViewProps) {
  const accessBlocked =
    liveState?.access_mode === "RO" || liveState?.access_mode === "NA";
  const unavailable = liveState !== undefined && liveState.is_available === false;
  const disabled = !canExecute || accessBlocked || unavailable;
  let title: string;
  if (!canExecute) {
    title = disabledReason || "Offline mode — command disabled.";
  } else if (accessBlocked) {
    title = `Command not writable (access_mode=${liveState?.access_mode ?? "?"}).`;
  } else if (unavailable) {
    title = "Command not available in the current device state.";
  } else {
    title = "Execute command";
  }

  return (
    <div className="editor">
      <button type="button" disabled={disabled} title={title} onClick={onExecute}>
        Execute
      </button>
      <div className="editor__hint">{title}</div>
    </div>
  );
}
