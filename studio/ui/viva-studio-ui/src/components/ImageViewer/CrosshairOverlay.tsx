interface CrosshairOverlayProps {
  mouseX: number | null;
  mouseY: number | null;
}

/**
 * Absolutely-positioned SVG crosshair that follows the mouse over the image canvas.
 * Renders two dashed lines (horizontal + vertical) at the current mouse position.
 * pointer-events: none — does not interfere with zoom/pan handlers.
 */
export function CrosshairOverlay({ mouseX, mouseY }: CrosshairOverlayProps) {
  if (mouseX === null || mouseY === null) return null;

  return (
    <svg
      className="iv-crosshair"
      style={{
        position: "absolute",
        inset: 0,
        pointerEvents: "none",
        overflow: "visible",
        width: "100%",
        height: "100%",
      }}
    >
      <line
        x1="0"
        x2="100%"
        y1={mouseY}
        y2={mouseY}
        stroke="var(--accent)"
        strokeWidth="1"
        opacity="0.7"
        strokeDasharray="4 3"
      />
      <line
        x1={mouseX}
        x2={mouseX}
        y1="0"
        y2="100%"
        stroke="var(--accent)"
        strokeWidth="1"
        opacity="0.7"
        strokeDasharray="4 3"
      />
    </svg>
  );
}
