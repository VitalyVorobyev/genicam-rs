/** Wrap-relative screen-space line segment for the line overlay SVG. */
export interface ScreenLine {
  x1: number;
  y1: number;
  x2: number;
  y2: number;
}

interface LineOverlayProps {
  line: ScreenLine | null;
}

/**
 * Absolutely-positioned SVG overlay that renders the current line profile
 * selection on top of the image canvas.
 *
 * Draws the line segment plus circular endpoint markers.
 * pointer-events: none — does not interfere with other interactions.
 */
export function LineOverlay({ line }: LineOverlayProps) {
  if (!line) return null;

  return (
    <svg
      style={{
        position: "absolute",
        inset: 0,
        pointerEvents: "none",
        overflow: "visible",
        width: "100%",
        height: "100%",
      }}
    >
      {/* Line segment */}
      <line
        x1={line.x1}
        y1={line.y1}
        x2={line.x2}
        y2={line.y2}
        stroke="var(--warning)"
        strokeWidth="1.5"
        strokeDasharray="6 3"
        opacity="0.9"
      />
      {/* Start endpoint dot */}
      <circle cx={line.x1} cy={line.y1} r={4} fill="var(--warning)" opacity="0.9" />
      {/* End endpoint dot */}
      <circle cx={line.x2} cy={line.y2} r={4} fill="var(--warning)" opacity="0.9" />
    </svg>
  );
}
