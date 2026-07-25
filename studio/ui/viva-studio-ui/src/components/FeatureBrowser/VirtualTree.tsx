import { memo, useCallback, useRef, useState, useEffect } from "react";
import { ROW_HEIGHT, type FlatRow } from "./flattenTree";

/** Extra rows rendered above/below the viewport for smooth scrolling. */
const OVERSCAN = 5;

interface VirtualTreeProps {
  rows: FlatRow[];
  selectedNodeName: string | null;
  onSelectNode: (name: string) => void;
  onToggleCategory: (name: string) => void;
  onToggleFavorite: (name: string) => void;
  onToggleFavExpanded: () => void;
}

export function VirtualTree({
  rows,
  selectedNodeName,
  onSelectNode,
  onToggleCategory,
  onToggleFavorite,
  onToggleFavExpanded,
}: VirtualTreeProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [scrollTop, setScrollTop] = useState(0);
  const [containerHeight, setContainerHeight] = useState(600);

  // Track container height via ResizeObserver
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const ro = new ResizeObserver(([entry]) => {
      setContainerHeight(entry.contentRect.height);
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const handleScroll = useCallback(() => {
    if (containerRef.current) {
      setScrollTop(containerRef.current.scrollTop);
    }
  }, []);

  const totalHeight = rows.length * ROW_HEIGHT;
  const startIdx = Math.max(0, Math.floor(scrollTop / ROW_HEIGHT) - OVERSCAN);
  const endIdx = Math.min(rows.length, Math.ceil((scrollTop + containerHeight) / ROW_HEIGHT) + OVERSCAN);
  const visibleRows = rows.slice(startIdx, endIdx);
  const offsetY = startIdx * ROW_HEIGHT;

  return (
    <div
      ref={containerRef}
      className="virtual-tree"
      role="tree"
      onScroll={handleScroll}
      style={{ overflow: "auto", flex: 1 }}
    >
      <div style={{ height: totalHeight, position: "relative" }}>
        <div style={{ position: "absolute", top: offsetY, left: 0, right: 0 }}>
          {visibleRows.map((row) => (
            <TreeRow
              key={row.key}
              row={row}
              isSelected={row.name === selectedNodeName}
              onSelectNode={onSelectNode}
              onToggleCategory={onToggleCategory}
              onToggleFavorite={onToggleFavorite}
              onToggleFavExpanded={onToggleFavExpanded}
            />
          ))}
        </div>
      </div>
    </div>
  );
}

// ── Memoized tree row ───────────────────────────────────────────────────────

interface TreeRowProps {
  row: FlatRow;
  isSelected: boolean;
  onSelectNode: (name: string) => void;
  onToggleCategory: (name: string) => void;
  onToggleFavorite: (name: string) => void;
  onToggleFavExpanded: () => void;
}

const TreeRow = memo(function TreeRow({
  row,
  isSelected,
  onSelectNode,
  onToggleCategory,
  onToggleFavorite,
  onToggleFavExpanded,
}: TreeRowProps) {
  const indent = { paddingLeft: `${8 + row.depth * 16}px` };

  // Favorite header row
  if (row.type === "favorite-header") {
    return (
      <button
        type="button"
        role="treeitem"
        aria-expanded={row.isExpanded}
        className="tree-item tree-item--category tree-item--favorites-header"
        style={indent}
        onClick={onToggleFavExpanded}
      >
        <span className="tree-item__caret">{row.isExpanded ? "\u25BE" : "\u25B8"}</span>
        <span className="tree-item__icon tree-item__icon--favorites">{"\u2605"}</span>
        <span className="tree-item__label">Favorites</span>
        <span className="tree-item__meta">{row.metaCount}</span>
      </button>
    );
  }

  // Missing/cycle row
  if (row.type === "missing") {
    return (
      <div className="tree-item tree-item--missing" style={indent} role="treeitem" aria-level={row.depth + 1}>
        <span className="tree-item__label">{row.displayName}</span>
        <span className="tree-item__meta">{row.kindLabel}</span>
      </div>
    );
  }

  // Category row
  if (row.type === "category") {
    return (
      <button
        type="button"
        role="treeitem"
        aria-expanded={row.isExpanded}
        aria-level={row.depth + 1}
        aria-selected={isSelected}
        className={isSelected ? "tree-item tree-item--category tree-item--active" : "tree-item tree-item--category"}
        style={indent}
        title={row.title}
        onClick={() => {
          onToggleCategory(row.name);
          onSelectNode(row.name);
        }}
      >
        <span className="tree-item__caret">{row.isExpanded ? "\u25BE" : "\u25B8"}</span>
        <span className="tree-item__icon tree-item__icon--category">{"\u25A6"}</span>
        <span className="tree-item__label">{row.displayName}</span>
        <span className="tree-item__meta">{row.name}</span>
      </button>
    );
  }

  // Leaf row (regular or favorite)
  const showFavBtn = row.type === "leaf" || row.type === "favorite";
  return (
    <button
      type="button"
      role="treeitem"
      aria-level={row.depth + 1}
      aria-selected={isSelected}
      className={isSelected ? "tree-item tree-item--active" : "tree-item"}
      style={indent}
      title={row.title}
      onClick={() => onSelectNode(row.name)}
    >
      {showFavBtn ? (
        <button
          type="button"
          className={`tree-item__favorite-btn${row.isFavorited ? " tree-item__favorite-btn--active" : ""}`}
          title={row.isFavorited ? "Remove from favorites" : "Add to favorites"}
          onClick={(e) => {
            e.stopPropagation();
            onToggleFavorite(row.name);
          }}
        >
          {row.isFavorited ? "\u2605" : "\u2606"}
        </button>
      ) : (
        <span className="tree-item__caret" />
      )}
      <span className={`tree-item__icon tree-item__icon--${row.kindKey}`}>{row.icon}</span>
      {row.accessMode === "RO" && (
        <span className="tree-item__access-badge" title="Read-only">RO</span>
      )}
      <span className="tree-item__label">{row.displayName}</span>
      {row.liveText !== null ? (
        <span className="tree-item__live" title={row.liveText}>{row.liveText}</span>
      ) : (
        <span className="tree-item__meta">{row.kindLabel}</span>
      )}
    </button>
  );
}, (prev, next) => {
  // Custom comparator — only re-render if these change
  return (
    prev.row.key === next.row.key &&
    prev.isSelected === next.isSelected &&
    prev.row.isExpanded === next.row.isExpanded &&
    prev.row.liveText === next.row.liveText &&
    prev.row.isFavorited === next.row.isFavorited &&
    prev.row.accessMode === next.row.accessMode
  );
});
