import type React from "react";
import { HistogramOverlay } from "./HistogramOverlay";
import { LineProfilePanel } from "./LineProfilePanel";
import type { LineSegment } from "./lineProfileUtils";

interface AnalysisPanelProps {
  showHistogram: boolean;
  showLineTool: boolean;
  lineSegment: LineSegment | null;
  frameRef: React.RefObject<Uint8Array | null>;
}

export function AnalysisPanel({
  showHistogram,
  showLineTool,
  lineSegment,
  frameRef,
}: AnalysisPanelProps) {
  const visible = showHistogram || showLineTool;

  return (
    <div className={`iv-analysis-panel${visible ? " iv-analysis-panel--open" : ""}`}>
      {visible && (
        <div className="iv-analysis-panel__content">
          {showLineTool && (
            <div className="iv-analysis-panel__slot">
              <LineProfilePanel
                frameRef={frameRef}
                lineSegment={lineSegment}
                visible={showLineTool}
              />
            </div>
          )}
          {showHistogram && (
            <div className="iv-analysis-panel__slot">
              <HistogramOverlay
                frameRef={frameRef}
                visible={showHistogram}
              />
            </div>
          )}
        </div>
      )}
    </div>
  );
}
