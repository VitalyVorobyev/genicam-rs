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
      {/*
        `.btn` is not optional decoration. The bare `button` reset in
        components.css is transparent, borderless and muted, so an unclassed
        button reads as a line of text — which is exactly how a Command node's
        only action was reported (#110). This is the same class the feature
        panel's Apply carries, so the primary action of a Command looks like
        the primary action of everything else.
      */}
      <button
        type="button"
        className="btn editor__action"
        disabled={disabled}
        title={title}
        onClick={onExecute}
      >
        Execute
      </button>
      <div className="editor__hint">{title}</div>
    </div>
  );
}
