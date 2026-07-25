import type { RawNode } from "../../../xml_model/uigraph";

interface UnknownDebugViewProps {
  raw: RawNode;
}

// Unknown nodes must remain visible. This view surfaces lightweight RawNode data
// without dumping the entire XML (keeps UI responsive for large files).
export function UnknownDebugView({ raw }: UnknownDebugViewProps) {
  return (
    <div className="unknown-debug">
      <div className="unknown-debug__section">
        <h4>Raw Tag</h4>
        <code>{raw.tag}</code>
      </div>
      <div className="unknown-debug__section">
        <h4>Attributes</h4>
        {Object.keys(raw.attributes).length === 0 ? (
          <div className="unknown-debug__empty">No attributes</div>
        ) : (
          <ul>
            {Object.entries(raw.attributes).map(([key, value]) => (
              <li key={key}>
                <span className="unknown-debug__key">{key}</span>
                <span className="unknown-debug__value">{value}</span>
              </li>
            ))}
          </ul>
        )}
      </div>
      <div className="unknown-debug__section">
        <h4>Child Text Fields</h4>
        {Object.keys(raw.children_text).length === 0 ? (
          <div className="unknown-debug__empty">No child fields</div>
        ) : (
          <ul>
            {Object.entries(raw.children_text).map(([key, value]) => (
              <li key={key}>
                <span className="unknown-debug__key">{key}</span>
                <span className="unknown-debug__value">{value}</span>
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}
