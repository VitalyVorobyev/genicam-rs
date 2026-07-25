import { useState } from "react";

const STORAGE_KEY = "iv-sidebar-collapsed";

function readStoredCollapsed(): boolean {
  try {
    return localStorage.getItem(STORAGE_KEY) === "true";
  } catch {
    return false;
  }
}

function writeStoredCollapsed(value: boolean): void {
  try {
    localStorage.setItem(STORAGE_KEY, value ? "true" : "false");
  } catch {
    // Private-browsing or quota exceeded — ignore
  }
}

export interface ViewerLayoutState {
  sidebarCollapsed: boolean;
  toggleSidebar: () => void;
}

export function useViewerLayout(): ViewerLayoutState {
  const [sidebarCollapsed, setSidebarCollapsed] = useState<boolean>(
    readStoredCollapsed,
  );

  function toggleSidebar(): void {
    setSidebarCollapsed((prev) => {
      const next = !prev;
      writeStoredCollapsed(next);
      return next;
    });
  }

  return { sidebarCollapsed, toggleSidebar };
}
