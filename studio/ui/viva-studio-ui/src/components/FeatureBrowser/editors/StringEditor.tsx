import type { NodeValue, ValueError } from "../../../xml_model/values";
import { ValidationErrors } from "./ValidationErrors";

interface StringEditorProps {
  value: NodeValue | undefined;
  errors: ValueError[];
  onChange: (value: NodeValue) => void;
}

// String editor writes into the shared draft store (offline mode).
export function StringEditor({ value, errors, onChange }: StringEditorProps) {
  const textValue = typeof value === "string" ? value : "";

  return (
    <div className="editor">
      <label className="editor__label">Value</label>
      <input
        className="editor__input"
        type="text"
        value={textValue}
        placeholder="unset (offline)"
        onChange={(event) => onChange(event.target.value)}
      />
      <ValidationErrors errors={errors} />
    </div>
  );
}
