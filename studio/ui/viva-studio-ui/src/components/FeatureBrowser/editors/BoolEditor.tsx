import type { NodeValue, ValueError } from "../../../xml_model/values";
import { ValidationErrors } from "./ValidationErrors";

interface BoolEditorProps {
  value: NodeValue | undefined;
  errors: ValueError[];
  onChange: (value: NodeValue) => void;
}

// Boolean editor writes into the shared draft store (offline mode).
export function BoolEditor({ value, errors, onChange }: BoolEditorProps) {
  const checked = typeof value === "boolean" ? value : false;

  return (
    <div className="editor">
      <label className="editor editor--inline">
        <input
          type="checkbox"
          checked={checked}
          onChange={(event) => onChange(event.target.checked)}
        />
        Enabled
      </label>
      <ValidationErrors errors={errors} />
    </div>
  );
}
