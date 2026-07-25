import { useCallback, useEffect, useState } from "react";
import type { ConnectionState, DeviceInfo, DisconnectReason } from "../../device/types";
import { DeviceCard } from "./DeviceCard";

// T7.4 — recent device storage shape
interface RecentDevice {
  id: string;
  name: string;
  model: string;
}

const RECENT_KEY = "viva-studio:recent-devices";
const MAX_RECENT = 5;

function loadRecent(): RecentDevice[] {
  try {
    const raw = localStorage.getItem(RECENT_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw) as unknown;
    if (!Array.isArray(parsed)) return [];
    return parsed.filter(
      (item): item is RecentDevice =>
        typeof item === "object" &&
        item !== null &&
        typeof (item as RecentDevice).id === "string" &&
        typeof (item as RecentDevice).name === "string"
    );
  } catch {
    return [];
  }
}

function saveRecent(devices: RecentDevice[]): void {
  try {
    localStorage.setItem(RECENT_KEY, JSON.stringify(devices));
  } catch {
    // localStorage unavailable — silently ignore
  }
}

function pushRecent(device: DeviceInfo): RecentDevice[] {
  const prev = loadRecent().filter((r) => r.id !== device.id);
  const next: RecentDevice[] = [
    { id: device.id, name: device.name, model: device.model },
    ...prev,
  ].slice(0, MAX_RECENT);
  saveRecent(next);
  return next;
}

interface DeviceSidebarProps {
  devices: DeviceInfo[];
  connectionState: ConnectionState;
  disconnectReason?: DisconnectReason | null;
  lastConnectedDeviceId?: string | null;
  onConnect: (deviceId: string) => void;
  onDisconnect: () => void;
}

export function DeviceSidebar({
  devices,
  connectionState,
  disconnectReason = null,
  lastConnectedDeviceId = null,
  onConnect,
  onDisconnect,
}: DeviceSidebarProps) {
  const [recentDevices, setRecentDevices] = useState<RecentDevice[]>(() => loadRecent());

  const statusLabel = connectionStateLabel(connectionState);
  const statusClass = connectionStateClass(connectionState);

  // When a connection succeeds, push the device to recent list.
  useEffect(() => {
    if (connectionState.kind !== "connected") return;
    const { device_id, device_name, model } = connectionState;
    const updated = pushRecent({ id: device_id, name: device_name, model, serial: "" });
    setRecentDevices(updated);
  }, [connectionState]);

  const handleRecentConnect = useCallback(
    (id: string) => {
      onConnect(id);
    },
    [onConnect]
  );

  // Devices discovered but not in the live list that are still in recent
  const liveIds = new Set(devices.map((d) => d.id));
  const recentOnly = recentDevices.filter((r) => !liveIds.has(r.id));

  return (
    <div className="device-sidebar">
      <div className="device-sidebar__section-header">
        <span>
          Devices
          {devices.length > 0 && (
            <span className="device-sidebar__count">{devices.length}</span>
          )}
        </span>
        <span className={`conn-badge conn-badge--${statusClass}`}>{statusLabel}</span>
      </div>

      {connectionState.kind === "reconnecting" && (
        <div className="device-sidebar__disconnect-banner device-sidebar__disconnect-banner--reconnecting">
          <span className="disconnect-message">
            {connectionState.reason}
          </span>
          <span className="disconnect-message" style={{ fontSize: "var(--text-xs)", opacity: 0.7 }}>
            Attempt {connectionState.attempt}/{connectionState.max_attempts}
          </span>
        </div>
      )}

      {connectionState.kind === "error" && (
        <div className="device-sidebar__disconnect-banner">
          <span className="disconnect-message">
            {disconnectReason?.message ?? connectionState.message}
          </span>
          {lastConnectedDeviceId !== null && (
            <button
              type="button"
              className="reconnect-btn"
              onClick={() => onConnect(lastConnectedDeviceId)}
            >
              Reconnect
            </button>
          )}
        </div>
      )}

      {/* Live discovered devices */}
      <div className="device-sidebar__list">
        {devices.length === 0 ? (
          <div className="device-sidebar__scanning">
            <div className="device-sidebar__scanning-dots" aria-hidden="true">
              <span className="device-sidebar__scanning-dot" />
              <span className="device-sidebar__scanning-dot" />
              <span className="device-sidebar__scanning-dot" />
            </div>
            <span>Scanning for devices…</span>
          </div>
        ) : (
          devices.map((device) => (
            <DeviceCard
              key={device.id}
              device={device}
              connectionState={connectionState}
              onConnect={onConnect}
              onDisconnect={onDisconnect}
            />
          ))
        )}
      </div>

      {/* Recent devices (only those not currently visible in the live list) */}
      {recentOnly.length > 0 && (
        <>
          <div className="device-sidebar__divider" />
          <div className="device-sidebar__section-header">
            <span>
              Recent
              <span className="device-sidebar__count">{recentOnly.length}</span>
            </span>
          </div>
          <div className="device-sidebar__list">
            {recentOnly.map((recent) => (
              <div key={recent.id} className="device-card device-card--recent">
                <div className="device-card__header">
                  <span className="device-card__name">{recent.name}</span>
                  <span
                    className="device-card__status-dot device-card__status-dot--disconnected"
                    aria-label="offline"
                  />
                </div>
                {recent.model && (
                  <div className="device-card__meta">
                    <span>{recent.model}</span>
                  </div>
                )}
                <div className="device-card__id">{recent.id}</div>
                <div className="device-card__actions">
                  <button
                    type="button"
                    className="btn"
                    disabled={connectionState.kind === "connecting" || connectionState.kind === "connected"}
                    onClick={() => handleRecentConnect(recent.id)}
                  >
                    Connect
                  </button>
                </div>
              </div>
            ))}
          </div>
        </>
      )}
    </div>
  );
}

function connectionStateLabel(state: ConnectionState): string {
  switch (state.kind) {
    case "disconnected":  return "Disconnected";
    case "connecting":    return "Connecting\u2026";
    case "connected":     return "Connected";
    case "reconnecting":  return `Reconnecting (${state.attempt}/${state.max_attempts})\u2026`;
    case "error":         return "Error";
  }
}

function connectionStateClass(state: ConnectionState): string {
  switch (state.kind) {
    case "disconnected":  return "disconnected";
    case "connecting":    return "connecting";
    case "connected":     return "connected";
    case "reconnecting":  return "connecting";
    case "error":         return "error";
  }
}
