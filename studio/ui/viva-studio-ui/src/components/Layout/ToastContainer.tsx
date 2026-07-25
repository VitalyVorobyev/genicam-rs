import { useToast } from "../../context/ToastContext";

/**
 * Fixed-position overlay that renders the active toast stack.
 *
 * - Positioned in the top-right corner, below the app header.
 * - aria-live="polite" announces toasts to screen readers.
 * - Each toast has a manual dismiss button in addition to auto-dismiss.
 * - pointer-events: none on the container so clicks pass through; the
 *   individual toast cards re-enable pointer-events.
 */
export function ToastContainer() {
  const { toasts, removeToast } = useToast();

  if (toasts.length === 0) return null;

  return (
    <div className="toast-container" aria-live="polite" aria-atomic="false">
      {toasts.map((toast) => (
        <div
          key={toast.id}
          className={`toast toast--${toast.level}`}
          role="alert"
        >
          <span className="toast__message">{toast.message}</span>
          <button
            type="button"
            className="toast__dismiss"
            aria-label="Dismiss notification"
            onClick={() => removeToast(toast.id)}
          >
            ×
          </button>
        </div>
      ))}
    </div>
  );
}
