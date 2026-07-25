import type { UiGraph, UiNode } from "../../xml_model/uigraph";
import type { NodeValueEntry } from "../../device/types";
import {
  isUnknownKind,
  nodeDisplayName,
  nodeKindLabel,
  nodeKindCssKey,
  nodeKindIcon,
} from "../../xml_model/helpers";
import { visibilityPassesFilter, type VisibilityFilter } from "./FeatureBrowserPage";
import { formatLiveValue } from "./treeUtils";

export type FlatRowType = "category" | "leaf" | "favorite-header" | "favorite" | "missing";

export interface FlatRow {
  key: string;
  name: string;
  depth: number;
  type: FlatRowType;
  displayName: string;
  kindKey: string;
  icon: string;
  kindLabel: string;
  accessMode: string | null;
  liveText: string | null;
  isFavorited: boolean;
  isExpanded: boolean;
  title: string;
  /** Count shown on headers (favorites count, category child count) */
  metaCount?: number;
}

export const ROW_HEIGHT = 28;

interface FlattenOptions {
  graph: UiGraph;
  rootCategory: string;
  expanded: Set<string>;
  hideUnknown: boolean;
  visibilityFilter: VisibilityFilter;
  favorites: Set<string>;
  favExpanded: boolean;
  liveValues?: Map<string, NodeValueEntry>;
}

/**
 * Flatten the category tree + favorites into a single array suitable for
 * virtual scrolling. Each element carries all data needed to render a row.
 */
export function flattenVisibleTree(opts: FlattenOptions): FlatRow[] {
  const { graph, rootCategory, expanded, hideUnknown, visibilityFilter, favorites, favExpanded, liveValues } = opts;
  const rows: FlatRow[] = [];

  // Favorites section
  const activeFavorites = [...favorites].filter((n) => graph.nodes_by_name[n] !== undefined);
  if (activeFavorites.length > 0) {
    rows.push({
      key: "__fav-header__",
      name: "__favorites__",
      depth: 0,
      type: "favorite-header",
      displayName: "Favorites",
      kindKey: "favorites",
      icon: "\u2605",
      kindLabel: "",
      accessMode: null,
      liveText: null,
      isFavorited: false,
      isExpanded: favExpanded,
      title: "Favorites",
      metaCount: activeFavorites.length,
    });
    if (favExpanded) {
      for (const name of activeFavorites) {
        const node = graph.nodes_by_name[name];
        if (!node) continue;
        const liveEntry = liveValues?.get(name);
        const liveText = liveEntry !== undefined ? formatLiveValue(liveEntry) : null;
        const effectiveAccess = liveEntry?.access_mode ?? node.access_mode ?? null;
        rows.push({
          key: `fav:${name}`,
          name,
          depth: 1,
          type: "favorite",
          displayName: nodeDisplayName(node),
          kindKey: nodeKindCssKey(node.kind),
          icon: nodeKindIcon(node.kind),
          kindLabel: nodeKindLabel(node.kind),
          accessMode: effectiveAccess,
          liveText,
          isFavorited: true,
          isExpanded: false,
          title: liveText !== null ? `${nodeDisplayName(node)} = ${liveText}` : nodeDisplayName(node),
        });
      }
    }
  }

  // Main tree
  walkCategory(graph, rootCategory, 0, new Set(), rows, expanded, hideUnknown, visibilityFilter, favorites, liveValues);

  return rows;
}

function walkCategory(
  graph: UiGraph,
  categoryName: string,
  depth: number,
  path: Set<string>,
  rows: FlatRow[],
  expanded: Set<string>,
  hideUnknown: boolean,
  visibilityFilter: VisibilityFilter,
  favorites: Set<string>,
  liveValues?: Map<string, NodeValueEntry>,
) {
  const category = graph.categories[categoryName];
  if (!category) {
    rows.push({
      key: `missing:${categoryName}:${depth}`,
      name: categoryName,
      depth,
      type: "missing",
      displayName: `Missing: ${categoryName}`,
      kindKey: "unknown",
      icon: "?",
      kindLabel: "",
      accessMode: null,
      liveText: null,
      isFavorited: false,
      isExpanded: false,
      title: `Missing category: ${categoryName}`,
    });
    return;
  }

  if (path.has(categoryName)) {
    rows.push({
      key: `cycle:${categoryName}:${depth}`,
      name: categoryName,
      depth,
      type: "missing",
      displayName: `Cycle: ${categoryName}`,
      kindKey: "unknown",
      icon: "!",
      kindLabel: "",
      accessMode: null,
      liveText: null,
      isFavorited: false,
      isExpanded: false,
      title: `Cycle detected: ${categoryName}`,
    });
    return;
  }

  const isExpanded = expanded.has(categoryName);
  const catTitle = category.tooltip ?? category.comment ?? category.display_name;

  rows.push({
    key: `cat:${categoryName}`,
    name: categoryName,
    depth,
    type: "category",
    displayName: category.display_name,
    kindKey: "category",
    icon: "\u25A6",
    kindLabel: categoryName,
    accessMode: null,
    liveText: null,
    isFavorited: false,
    isExpanded,
    title: catTitle,
  });

  if (!isExpanded) return;

  const nextPath = new Set(path);
  nextPath.add(categoryName);

  for (const featureName of category.features) {
    const node = graph.nodes_by_name[featureName];
    if (!node) {
      rows.push({
        key: `missing-ref:${featureName}:${depth + 1}`,
        name: featureName,
        depth: depth + 1,
        type: "missing",
        displayName: "MissingRef",
        kindKey: "unknown",
        icon: "?",
        kindLabel: featureName,
        accessMode: null,
        liveText: null,
        isFavorited: false,
        isExpanded: false,
        title: `Missing reference: ${featureName}`,
      });
      continue;
    }

    const isCategory = typeof node.kind === "string" && node.kind === "Category";

    if (!isCategory) {
      if (hideUnknown && isUnknownKind(node.kind)) continue;
      if (!visibilityPassesFilter(node.visibility, visibilityFilter)) continue;
    }

    if (isCategory) {
      walkCategory(graph, featureName, depth + 1, nextPath, rows, expanded, hideUnknown, visibilityFilter, favorites, liveValues);
    } else {
      const liveEntry = liveValues?.get(featureName);
      const liveText = liveEntry !== undefined ? formatLiveValue(liveEntry) : null;
      const nodeTitle = node.tooltip ?? node.comment ?? nodeDisplayName(node);
      const effectiveAccess = liveEntry?.access_mode ?? node.access_mode ?? null;

      rows.push({
        key: `leaf:${featureName}`,
        name: featureName,
        depth: depth + 1,
        type: "leaf",
        displayName: nodeDisplayName(node),
        kindKey: nodeKindCssKey(node.kind),
        icon: nodeKindIcon(node.kind),
        kindLabel: nodeKindLabel(node.kind),
        accessMode: effectiveAccess,
        liveText,
        isFavorited: favorites.has(featureName),
        isExpanded: false,
        title: liveText !== null ? `${nodeTitle} = ${liveText}` : nodeTitle,
      });
    }
  }
}
