import { useEffect, useRef } from "react";
import { sampleLineProfile, renderLineProfile } from "./lineProfileUtils";
import type { LineSegment } from "./lineProfileUtils";

const REFRESH_MS = 200; // ~5 fps refresh

interface LineProfilePanelProps {
  frameRef: React.RefObject<Uint8Array | null>;
  lineSegment: LineSegment | null;
  visible: boolean;
}

export function LineProfilePanel({ frameRef, lineSegment, visible }: LineProfilePanelProps) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const wrapRef = useRef<HTMLDivElement | null>(null);

  // Track container size and resize canvas accordingly
  useEffect(() => {
    if (!visible || !lineSegment) return;
    const wrap = wrapRef.current;
    const canvas = canvasRef.current;
    if (!wrap || !canvas) return;

    const updateSize = () => {
      const w = Math.round(wrap.clientWidth);
      const h = Math.round(wrap.clientHeight);
      if (w > 0 && h > 0) {
        canvas.width = w;
        canvas.height = h;
      }
    };

    const observer = new ResizeObserver(updateSize);
    observer.observe(wrap);
    updateSize();
    return () => observer.disconnect();
  }, [visible, lineSegment]);

  useEffect(() => {
    if (!visible || !lineSegment) return;

    const tick = () => {
      const bytes = frameRef.current;
      if (!bytes) return;
      const data = sampleLineProfile(
        bytes,
        lineSegment.x0,
        lineSegment.y0,
        lineSegment.x1,
        lineSegment.y1,
      );
      if (!data) return;
      const canvas = canvasRef.current;
      if (!canvas || canvas.width === 0 || canvas.height === 0) return;
      const ctx = canvas.getContext("2d");
      if (!ctx) return;
      renderLineProfile(ctx, data, canvas.width, canvas.height);
    };

    tick();
    const id = setInterval(tick, REFRESH_MS);
    return () => clearInterval(id);
  }, [visible, lineSegment, frameRef]);

  if (!visible || !lineSegment) return null;

  return (
    <div ref={wrapRef} className="iv-line-profile" aria-label="Line profile">
      <canvas ref={canvasRef} className="iv-line-profile__canvas" />
    </div>
  );
}
