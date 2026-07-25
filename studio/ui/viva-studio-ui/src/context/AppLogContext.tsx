import {
  createContext,
  useCallback,
  useContext,
  useReducer,
  useRef,
  type ReactNode,
} from "react";

// ── Types ────────────────────────────────────────────────────────────────────

export type LogLevel = "info" | "success" | "warning" | "error";

export interface LogEntry {
  id: number;
  ts: Date;
  level: LogLevel;
  message: string;
  node?: string;
}

interface AppLogState {
  entries: LogEntry[];
}

type AppLogAction =
  | { type: "APPEND"; entry: LogEntry }
  | { type: "CLEAR" };

// ── Reducer ──────────────────────────────────────────────────────────────────

const MAX_ENTRIES = 500;

function logReducer(state: AppLogState, action: AppLogAction): AppLogState {
  switch (action.type) {
    case "APPEND":
      return {
        entries: [action.entry, ...state.entries].slice(0, MAX_ENTRIES),
      };
    case "CLEAR":
      return { entries: [] };
    default:
      return state;
  }
}

// ── Context ──────────────────────────────────────────────────────────────────

interface AppLogContextValue {
  entries: LogEntry[];
  log: (level: LogLevel, message: string, node?: string) => void;
  clear: () => void;
}

const AppLogContext = createContext<AppLogContextValue | null>(null);

// ── Provider ─────────────────────────────────────────────────────────────────

interface AppLogProviderProps {
  children: ReactNode;
}

export function AppLogProvider({ children }: AppLogProviderProps) {
  const [state, dispatch] = useReducer(logReducer, { entries: [] });
  const nextId = useRef(0);

  const log = useCallback(
    (level: LogLevel, message: string, node?: string) => {
      const entry: LogEntry = {
        id: nextId.current++,
        ts: new Date(),
        level,
        message,
        node,
      };
      dispatch({ type: "APPEND", entry });
    },
    []
  );

  const clear = useCallback(() => {
    dispatch({ type: "CLEAR" });
  }, []);

  return (
    <AppLogContext.Provider value={{ entries: state.entries, log, clear }}>
      {children}
    </AppLogContext.Provider>
  );
}

// ── Hook ─────────────────────────────────────────────────────────────────────

export function useAppLog(): AppLogContextValue {
  const ctx = useContext(AppLogContext);
  if (!ctx) {
    throw new Error("useAppLog must be used within AppLogProvider");
  }
  return ctx;
}
