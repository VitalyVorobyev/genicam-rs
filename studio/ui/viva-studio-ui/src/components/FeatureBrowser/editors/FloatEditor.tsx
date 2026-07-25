import type { UiNode } from "../../../xml_model/uigraph";
import type { NodeValue, ValueError } from "../../../xml_model/values";
import type { FeatureState, NumericRange } from "../../../device/types";
import { ValidationErrors } from "./ValidationErrors";

interface FloatEditorProps {
  node: UiNode;
  value: NodeValue | undefined;
  errors: ValueError[];
  onChange: (value: NodeValue) => void;
  /**
   * Live device state. When its `numeric` range is present, we prefer it over
   * the static XML `node.constraints`. See `IntegerEditor` for the rationale
   * on rendering "range unknown" when neither is available.
   */
  liveState?: FeatureState;
}

function resolveRange(node: UiNode, liveState?: FeatureState): NumericRange | null {
  if (liveState?.numeric) return liveState.numeric;
  const { min, max, inc } = node.constraints ?? {};
  if (min === undefined && max === undefined && inc === undefined) return null;
  return {
    min: min ?? Number.NEGATIVE_INFINITY,
    max: max ?? Number.POSITIVE_INFINITY,
    inc,
  };
}

// Float editor writes into the shared draft store (offline mode).
export function FloatEditor({ node, value, errors, onChange, liveState }: FloatEditorProps) {
  let numericValue: number | "" = "";
  if (typeof value === "number") {
    numericValue = value;
  } else if (value === undefined && typeof liveState?.value === "number") {
    numericValue = liveState.value;
  }
  const range = resolveRange(node, liveState);
  const inputMin =
    range && Number.isFinite(range.min) ? range.min : undefined;
  const inputMax =
    range && Number.isFinite(range.max) ? range.max : undefined;
  const step: number | "any" = range?.inc ?? "any";
  const unit = liveState?.unit ?? node.unit;

  return (
    <div className="editor">
      <label className="editor__label">Value</label>
      <div className="editor__input-row">
        <input
          className="editor__input"
          type="number"
          step={step}
          min={inputMin}
          max={inputMax}
          value={numericValue}
          placeholder="unset (offline)"
          onChange={(event) => {
            const raw = event.target.value;
            if (raw === "") {
              onChange(null);
              return;
            }
            const parsed = Number(raw);
            onChange(Number.isFinite(parsed) ? parsed : null);
          }}
        />
        {unit && <span className="editor__unit">{unit}</span>}
      </div>
      <ValidationErrors errors={errors} />
      <ConstraintDetails range={range} />
    </div>
  );
}

function ConstraintDetails({ range }: { range: NumericRange | null }) {
  if (!range) {
    return <div className="editor__constraints editor__constraints--unknown">range unknown</div>;
  }
  const showMin = Number.isFinite(range.min);
  const showMax = Number.isFinite(range.max);
  if (!showMin && !showMax && range.inc === undefined) {
    return <div className="editor__constraints editor__constraints--unknown">range unknown</div>;
  }

  return (
    <div className="editor__constraints">
      {showMin && <span>min: {range.min}</span>}
      {showMax && <span>max: {range.max}</span>}
      {range.inc !== undefined && <span>inc: {range.inc}</span>}
    </div>
  );
}
