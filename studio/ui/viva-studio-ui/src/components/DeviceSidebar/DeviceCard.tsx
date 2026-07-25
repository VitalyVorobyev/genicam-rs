import type { DeviceInfo, ConnectionState } from "../../device/types";

interface DeviceCardProps {
  device: DeviceInfo;
  connectionState: ConnectionState;
  onConnect: (id: string) => void;
  onDisconnect: () => void;
}

export function DeviceCard({ device, connectionState, onConnect, onDisconnect }: DeviceCardProps) {
  const isConnecting =
    connectionState.kind === "connecting" &&
    connectionState.device_id === device.id;
  const isConnected =
    connectionState.kind === "connected" &&
    connectionState.device_id === device.id;
  const isReconnecting =
    connectionState.kind === "reconnecting" &&
    connectionState.device_id === device.id;
  const isBusy =
    connectionState.kind === "connecting" || connectionState.kind === "connected" || connectionState.kind === "reconnecting";

  const dotState = isConnected
    ? "connected"
    : isConnecting || isReconnecting
    ? "connecting"
    : "disconnected";

  let cardClass = "device-card";
  if (isConnected) cardClass += " device-card--connected";
  if (isConnecting) cardClass += " device-card--connecting";

  return (
    <div className={cardClass}>
      <div className="device-card__header">
        <span className="device-card__name">{device.name}</span>
        <span
          className={`device-card__status-dot device-card__status-dot--${dotState}`}
          aria-label={dotState}
        />
      </div>
      <div className="device-card__meta">
        {device.model && <span>{device.model}</span>}
        {device.serial && <span className="muted">{device.serial}</span>}
      </div>
      <div className="device-card__id">{device.id}</div>
      <div className="device-card__actions">
        {isConnected ? (
          <button type="button" className="btn--secondary" onClick={onDisconnect}>
            Disconnect
          </button>
        ) : (
          <button
            type="button"
            className="btn"
            disabled={isBusy || isConnecting}
            onClick={() => onConnect(device.id)}
          >
            {(isConnecting || isReconnecting) && <span className="spinner" />}
            {isConnecting ? "Connecting\u2026" : isReconnecting ? "Reconnecting\u2026" : "Connect"}
          </button>
        )}
      </div>
    </div>
  );
}
