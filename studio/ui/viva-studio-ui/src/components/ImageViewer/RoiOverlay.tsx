/** Wrap-relative screen-space rectangle for ROI overlay rendering. */
export interface ScreenRect {
  x1: number;
  y1: number;
  x2: number;
  y2: number;
}

interface RoiOverlayProps {
  rect: ScreenRect | null;
}

/**
 * Absolutely-positioned SVG overlay that renders the current ROI selection
 * rectangle on top of the image canvas.
 *
 * Coordinates are in wrap-relative screen pixels.
 * pointer-events: none — does not interfere with zoom/pan or ROI drag handlers.
 */
export function RoiOverlay({ rect }: RoiOverlayProps) {
  if (!rect) return null;

  const x = Math.min(rect.x1, rect.x2);
  const y = Math.min(rect.y1, rect.y2);
  const w = Math.abs(rect.x2 - rect.x1);
  const h = Math.abs(rect.y2 - rect.y1);

  return (
    <svg
      className="iv-roi-overlay"
      style={{
        position: "absolute",
        inset: 0,
        pointerEvents: "none",
        overflow: "visible",
        width: "100%",
        height: "100%",
      }}
    >
      {/* Semi-transparent fill */}
      <rect
        x={x}
        y={y}
        width={w}
        height={h}
        fill="rgba(47, 129, 247, 0.08)"
        stroke="none"
      />
      {/* Dashed border */}
      <rect
        x={x}
        y={y}
        width={w}
        height={h}
        fill="none"
        stroke="var(--accent)"
        strokeWidth="1.5"
        strokeDasharray="5 3"
        opacity="0.9"
      />
      {/* Corner handles */}
      {[
        [x, y],
        [x + w, y],
        [x, y + h],
        [x + w, y + h],
      ].map(([cx, cy], i) => (
        <rect
          key={i}
          x={(cx ?? 0) - 3}
          y={(cy ?? 0) - 3}
          width={6}
          height={6}
          fill="var(--accent)"
          opacity="0.9"
        />
      ))}
    </svg>
  );
}
