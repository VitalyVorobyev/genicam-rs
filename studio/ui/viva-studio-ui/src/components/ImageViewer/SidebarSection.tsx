import { useState } from "react";
import type { ReactNode } from "react";

interface SidebarSectionProps {
  title: string;
  icon: string;
  children: ReactNode;
  defaultOpen?: boolean;
  onToggle?: (open: boolean) => void;
}

export function SidebarSection({
  title,
  icon,
  children,
  defaultOpen = false,
  onToggle,
}: SidebarSectionProps) {
  const [open, setOpen] = useState(defaultOpen);

  return (
    <div className="sidebar-section">
      <button
        type="button"
        className="sidebar-section__header"
        aria-expanded={open}
        onClick={() => {
          const next = !open;
          onToggle?.(next);
          setOpen(next);
        }}
      >
        <span className="sidebar-section__icon" aria-hidden="true">
          {icon}
        </span>
        <span className="sidebar-section__label">{title}</span>
        <span
          className={`sidebar-section__chevron${open ? " sidebar-section__chevron--open" : ""}`}
          aria-hidden="true"
        >
          ›
        </span>
      </button>
      <div
        className={`sidebar-section__body${open ? "" : " sidebar-section__body--hidden"}`}
      >
        {children}
      </div>
    </div>
  );
}
