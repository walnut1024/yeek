import { Component, type ReactNode } from "react";
import i18n from "@/i18n";
import { Button } from "@/components/ui/button";

interface Props {
  children: ReactNode;
  fallback?: ReactNode;
}

interface State {
  hasError: boolean;
  error: Error | null;
}

export class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: React.ErrorInfo) {
    console.error("ErrorBoundary caught:", error, errorInfo);
  }

  render() {
    if (this.state.hasError) {
      if (this.props.fallback) {
        return this.props.fallback;
      }
      return (
        <div className="flex h-dvh items-center justify-center bg-background text-foreground">
          <div className="max-w-md space-y-4 text-center">
            <h2 className="text-sm font-semibold">{i18n.t("error.title")}</h2>
            <p className="text-xs text-muted-foreground">
              {this.state.error?.message || i18n.t("error.fallback")}
            </p>
            <Button
              type="button"
              variant="primary"
              size="sm"
              onClick={() => window.location.reload()}
            >
              {i18n.t("error.reload")}
            </Button>
          </div>
        </div>
      );
    }
    return this.props.children;
  }
}
