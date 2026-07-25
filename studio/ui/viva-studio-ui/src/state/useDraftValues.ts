import { useCallback, useState } from "react";
import type { UiNode } from "../xml_model/uigraph";
import type { NodeValue, ValueError } from "../xml_model/values";
import { validateDraft } from "../xml_model/validate";

// Draft values live outside the UiGraph contract. We only store per-node edits
// and validation errors keyed by node name to avoid duplicating full node data.
export function useDraftValues() {
  const [drafts, setDrafts] = useState<Record<string, NodeValue>>({});
  const [errors, setErrors] = useState<Record<string, ValueError[]>>({});

  const setDraft = useCallback((node: UiNode, value: NodeValue) => {
    setDrafts((prev) => ({ ...prev, [node.name]: value }));
    setErrors((prev) => ({ ...prev, [node.name]: validateDraft(node, value) }));
  }, []);

  const resetDraft = useCallback((name: string) => {
    setDrafts((prev) => {
      if (!(name in prev)) {
        return prev;
      }
      const next = { ...prev };
      delete next[name];
      return next;
    });
    setErrors((prev) => {
      if (!(name in prev)) {
        return prev;
      }
      const next = { ...prev };
      delete next[name];
      return next;
    });
  }, []);

  const clearAllDrafts = useCallback(() => {
    setDrafts({});
    setErrors({});
  }, []);

  return {
    drafts,
    errors,
    setDraft,
    resetDraft,
    clearAllDrafts,
  };
}
