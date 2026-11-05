import { Component, type ErrorInfo, type ReactNode } from "react";
import { logError } from "./report";

interface Props {
  readonly children: ReactNode;
}

interface State {
  readonly failed: boolean;
}

export class ErrorBoundary extends Component<Props, State> {
  state: State = { failed: false };

  static getDerivedStateFromError(): State {
    return { failed: true };
  }

  componentDidCatch(error: Error, _info: ErrorInfo): void {
    logError("web.render_error", "render failed", error);
  }

  render(): ReactNode {
    if (!this.state.failed) return this.props.children;
    return (
      <div role="alert" style={{ padding: "2rem" }}>
        <h1>Something went wrong</h1>
        <p>The error was logged. Reload the window to continue.</p>
        <button type="button" onClick={() => window.location.reload()}>
          Reload
        </button>
      </div>
    );
  }
}
