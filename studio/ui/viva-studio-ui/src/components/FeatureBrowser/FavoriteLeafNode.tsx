import type { UiGraph } from "../../xml_model/uigraph";
import type { NodeValueEntry } from "../../device/types";
import {
  nodeDisplayName,
  nodeKindCssKey,
  nodeKindIcon,
} from "../../xml_model/helpers";
import { formatLiveValue } from "./treeUtils";

interface FavoriteLeafNodeProps {
  featureName: string;
  graph: UiGraph;
  selectedNodeName: string | null;
  onSelectNode: (name: string) => void;
  liveValues?: Map<string, NodeValueEntry>;
  isFavorited: boolean;
  onToggleFavorite: (name: string) => void;
}

export function FavoriteLeafNode({
  featureName,
  graph,
  selectedNodeName,
  onSelectNode,
  liveValues,
  isFavorited,
  onToggleFavorite,
}: FavoriteLeafNodeProps) {
  const node = graph.nodes_by_name[featureName];
  if (!node) return null;

  const kindKey = nodeKindCssKey(node.kind);
  const icon = nodeKindIcon(node.kind);
  const liveEntry = liveValues?.get(featureName);
  const liveText = liveEntry !== undefined ? formatLiveValue(liveEntry) : null;
  const nodeTitle = node.tooltip ?? node.comment ?? nodeDisplayName(node);

  return (
    <button
      type="button"
      className={
        featureName === selectedNodeName
          ? "tree-item tree-item--active"
          : "tree-item"
      }
      style={{ paddingLeft: "24px" }}
      title={liveText !== null ? `${nodeTitle} = ${liveText}` : nodeTitle}
      onClick={() => onSelectNode(featureName)}
    >
      <button
        type="button"
        className="tree-item__favorite-btn"
        title={isFavorited ? "Remove from favorites" : "Add to favorites"}
        onClick={(e) => {
          e.stopPropagation();
          onToggleFavorite(featureName);
        }}
      >
        {isFavorited ? "\u2605" : "\u2606"}
      </button>
      <span className={`tree-item__icon tree-item__icon--${kindKey}`}>
        {icon}
      </span>
      <span className="tree-item__label">{nodeDisplayName(node)}</span>
      {liveText !== null ? (
        <span className="tree-item__live" title={liveText}>{liveText}</span>
      ) : (
        <span className="tree-item__meta">{node.name}</span>
      )}
    </button>
  );
}
