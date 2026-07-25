import {
  createContext,
  useCallback,
  useContext,
  useRef,
  useState,
  type ReactNode,
} from "react";

// ── Types ─────────────────────────────────────────────────────────────────────

export type ToastLevel = "info" | "success" | "warning" | "error";

export interface Toast {
  id: number;
  level: ToastLevel;
  message: string;
}

// ── Pure helpers ──────────────────────────────────────────────────────────────

/**
 * Returns the auto-dismiss duration in milliseconds for a given toast level.
 *
 * - info / success: 3 s (brief, non-critical)
 * - warning:        5 s (needs attention)
 * - error:          6 s (important, give time to read)
 */
export function formatToastDuration(level: ToastLevel): number {
  switch (level) {
    case "info":    return 3000;
    case "success": return 3000;
    case "warning": return 5000;
    case "error":   return 6000;
  }
}

// ── Context ───────────────────────────────────────────────────────────────────

interface ToastContextValue {
  toasts: Toast[];
  addToast: (level: ToastLevel, message: string) => void;
  removeToast: (id: number) => void;
}

const ToastContext = createContext<ToastContextValue | null>(null);

const MAX_TOASTS = 5;

// ── Provider ──────────────────────────────────────────────────────────────────

interface ToastProviderProps {
  children: ReactNode;
}

export function ToastProvider({ children }: ToastProviderProps) {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const nextId = useRef(0);
  // Map of toast id → timer id so we can clear the timer on manual dismiss.
  const timers = useRef<Map<number, ReturnType<typeof setTimeout>>>(new Map());

  const removeToast = useCallback((id: number) => {
    const timer = timers.current.get(id);
    if (timer !== undefined) {
      clearTimeout(timer);
      timers.current.delete(id);
    }
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, []);

  const addToast = useCallback(
    (level: ToastLevel, message: string) => {
      const id = nextId.current++;
      setToasts((prev) => {
        // Keep only the most recent (MAX_TOASTS - 1) entries to make room.
        const trimmed = prev.slice(-(MAX_TOASTS - 1));
        return [...trimmed, { id, level, message }];
      });
      const duration = formatToastDuration(level);
      const timer = setTimeout(() => removeToast(id), duration);
      timers.current.set(id, timer);
    },
    [removeToast],
  );

  return (
    <ToastContext.Provider value={{ toasts, addToast, removeToast }}>
      {children}
    </ToastContext.Provider>
  );
}

// ── Hook ──────────────────────────────────────────────────────────────────────

export function useToast(): ToastContextValue {
  const ctx = useContext(ToastContext);
  if (!ctx) {
    throw new Error("useToast must be used within ToastProvider");
  }
  return ctx;
}
