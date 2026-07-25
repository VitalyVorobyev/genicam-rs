import { useMemo } from "react";
import type { UiGraph } from "../../xml_model/uigraph";
import type { NodeValueEntry } from "../../device/types";
import type { VisibilityFilter } from "./FeatureBrowserPage";
import {
  isUnknownKind,
  nodeDisplayName,
  nodeKindCssKey,
  nodeKindIcon,
} from "../../xml_model/helpers";
import { visibilityPassesFilter } from "./FeatureBrowserPage";
import { formatLiveValue } from "./treeUtils";

interface FeatureListProps {
  graph: UiGraph | null;
  categoryName: string | null;
  visibilityFilter: VisibilityFilter;
  selectedNodeName: string | null;
  onSelectNode: (name: string) => void;
  onSelectCategory: (name: string) => void;
  liveValues?: Map<string, NodeValueEntry>;
}

interface FeatureRow {
  name: string;
  displayName: string;
  kindKey: string;
  icon: string;
  liveText: string | null;
  accessMode: string | null;
  isCategory: boolean;
  subFeatureCount?: number;
}

/** Middle column: features within the selected category. */
export function FeatureList({
  graph,
  categoryName,
  visibilityFilter,
  selectedNodeName,
  onSelectNode,
  onSelectCategory,
  liveValues,
}: FeatureListProps) {
  const { categoryTitle, features } = useMemo(() => {
    if (!graph || !categoryName) return { categoryTitle: "", features: [] as FeatureRow[] };
    const cat = graph.categories[categoryName];
    if (!cat) return { categoryTitle: categoryName, features: [] as FeatureRow[] };

    const rows: FeatureRow[] = [];
    for (const featureName of cat.features) {
      const node = graph.nodes_by_name[featureName];
      if (!node) continue;

      const isCategory = node.kind === "Category";

      if (!isCategory) {
        if (isUnknownKind(node.kind)) continue;
        if (!visibilityPassesFilter(node.visibility, visibilityFilter)) continue;
      }

      const liveEntry = liveValues?.get(featureName);
      const liveText = liveEntry !== undefined ? formatLiveValue(liveEntry) : null;
      const effectiveAccess = liveEntry?.access_mode ?? node.access_mode ?? null;

      const subCat = graph.categories[featureName];
      const subFeatureCount = subCat ? subCat.features.length : undefined;

      rows.push({
        name: featureName,
        displayName: nodeDisplayName(node),
        kindKey: isCategory ? "category" : nodeKindCssKey(node.kind),
        icon: isCategory ? "\u25A6" : nodeKindIcon(node.kind),
        liveText,
        accessMode: effectiveAccess,
        isCategory,
        subFeatureCount,
      });
    }
    return { categoryTitle: cat.display_name, features: rows };
  }, [graph, categoryName, visibilityFilter, liveValues]);

  if (!graph) {
    return (
      <div className="feat-list feat-list--empty">
        <span className="feat-list__hint">No model loaded</span>
      </div>
    );
  }

  if (!categoryName) {
    return (
      <div className="feat-list feat-list--empty">
        <span className="feat-list__hint">Select a category</span>
      </div>
    );
  }

  return (
    <div className="feat-list" role="listbox" aria-label={categoryTitle}>
      <div className="feat-list__header">{categoryTitle}</div>
      <div className="feat-list__scroll">
        {features.length === 0 ? (
          <div className="feat-list__hint">No visible features</div>
        ) : (
          features.map((feat) => {
            const isActive = feat.name === selectedNodeName;
            return (
              <button
                key={feat.name}
                type="button"
                role="option"
                aria-selected={isActive}
                className={`feat-list__item${isActive ? " feat-list__item--active" : ""}${feat.isCategory ? " feat-list__item--category" : ""}`}
                onClick={() =>
                  feat.isCategory
                    ? onSelectCategory(feat.name)
                    : onSelectNode(feat.name)
                }
                title={feat.name}
              >
                <span className={`feat-list__icon feat-list__icon--${feat.kindKey}`}>
                  {feat.icon}
                </span>
                {feat.accessMode === "RO" && (
                  <span className="feat-list__ro">RO</span>
                )}
                <span className="feat-list__name">{feat.displayName}</span>
                {feat.liveText !== null ? (
                  <span className="feat-list__live">{feat.liveText}</span>
                ) : feat.isCategory && feat.subFeatureCount !== undefined ? (
                  <span className="feat-list__meta">{feat.subFeatureCount}</span>
                ) : null}
                {feat.isCategory && (
                  <span className="feat-list__chevron">{"\u203A"}</span>
                )}
              </button>
            );
          })
        )}
      </div>
    </div>
  );
}
