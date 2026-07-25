import { useCallback, useEffect, useMemo, useState } from "react";
import type { UiGraph } from "../../xml_model/uigraph";
import type { NodeValueEntry } from "../../device/types";
import type { VisibilityFilter } from "./FeatureBrowserPage";
import { flattenVisibleTree } from "./flattenTree";
import { VirtualTree } from "./VirtualTree";

interface CategoryTreeProps {
  graph: UiGraph | null;
  hideUnknown: boolean;
  visibilityFilter: VisibilityFilter;
  selectedNodeName: string | null;
  onSelectNode: (name: string) => void;
  liveValues?: Map<string, NodeValueEntry>;
  favorites: Set<string>;
  onToggleFavorite: (name: string) => void;
}

export function CategoryTree({
  graph,
  hideUnknown,
  visibilityFilter,
  selectedNodeName,
  onSelectNode,
  liveValues,
  favorites,
  onToggleFavorite,
}: CategoryTreeProps) {
  const rootCategory = graph?.root_category || "";
  const [expanded, setExpanded] = useState<Set<string>>(() => new Set());
  const [favExpanded, setFavExpanded] = useState(true);

  useEffect(() => {
    if (rootCategory && !expanded.has(rootCategory)) {
      setExpanded((prev) => new Set(prev).add(rootCategory));
    }
  }, [expanded, rootCategory]);

  const toggleCategory = useCallback((name: string) => {
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(name)) next.delete(name);
      else next.add(name);
      return next;
    });
  }, []);

  const toggleFavExpanded = useCallback(() => {
    setFavExpanded((prev) => !prev);
  }, []);

  const rows = useMemo(() => {
    if (!graph || !rootCategory) return [];
    return flattenVisibleTree({
      graph,
      rootCategory,
      expanded,
      hideUnknown,
      visibilityFilter,
      favorites,
      favExpanded,
      liveValues,
    });
  }, [graph, rootCategory, expanded, hideUnknown, visibilityFilter, favorites, favExpanded, liveValues]);

  if (!graph) {
    return <div className="tree-empty">Load an XML file to browse the feature tree.</div>;
  }

  if (!rootCategory) {
    return <div className="tree-empty">No root category found.</div>;
  }

  return (
    <div className="category-tree" style={{ display: "flex", flexDirection: "column", flex: 1, minHeight: 0 }}>
      <div className="category-tree__header">Categories</div>
      <VirtualTree
        rows={rows}
        selectedNodeName={selectedNodeName}
        onSelectNode={onSelectNode}
        onToggleCategory={toggleCategory}
        onToggleFavorite={onToggleFavorite}
        onToggleFavExpanded={toggleFavExpanded}
      />
    </div>
  );
}
