import { Component, type ErrorInfo, type ReactNode } from "react";

interface Props {
  children: ReactNode;
}

interface State {
  hasError: boolean;
  error: Error | null;
  errorInfo: ErrorInfo | null;
}

export class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false, error: null, errorInfo: null };
  }

  static getDerivedStateFromError(error: Error): Partial<State> {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    this.setState({ errorInfo });
    console.error("ErrorBoundary caught:", error, errorInfo);
  }

  handleCopy = () => {
    const { error, errorInfo } = this.state;
    const text = [
      `Error: ${error?.message ?? "Unknown error"}`,
      "",
      error?.stack ?? "",
      "",
      "Component Stack:",
      errorInfo?.componentStack ?? "(unavailable)",
    ].join("\n");
    navigator.clipboard.writeText(text).catch(() => {});
  };

  handleReload = () => {
    window.location.reload();
  };

  render() {
    if (this.state.hasError) {
      return (
        <div className="error-boundary">
          <div className="error-boundary__card">
            <h1 className="error-boundary__title">Something went wrong</h1>
            <p className="error-boundary__message">
              An unexpected error occurred. You can copy the details for a bug report or reload the app.
            </p>
            <pre className="error-boundary__stack">
              {this.state.error?.message ?? "Unknown error"}
            </pre>
            <div className="error-boundary__actions">
              <button type="button" className="btn--secondary" onClick={this.handleCopy}>
                Copy error details
              </button>
              <button type="button" className="btn" onClick={this.handleReload}>
                Reload
              </button>
            </div>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}
