/** 全局错误边界：渲染异常兜底，避免整窗白屏；提供"重启界面"自救入口 */
import { Component, type ReactNode } from "react";

interface Props {
  children: ReactNode;
}

interface State {
  error: Error | null;
}

export class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error, info: { componentStack?: string | null }) {
    // 渲染崩溃属严重故障：console 留痕（不写 localStorage，避免损坏后无法清理）
    console.error("[ErrorBoundary] 渲染异常", error, info.componentStack);
  }

  render() {
    if (this.state.error) {
      return (
        <div className="flex h-screen flex-col items-center justify-center gap-3 bg-background p-6 text-center text-sm">
          <div className="text-2xl">⚠️</div>
          <p className="font-medium">界面出现异常</p>
          <p className="max-w-sm break-all text-xs text-muted-foreground">
            {this.state.error.message}
          </p>
          <div className="flex gap-2">
            <button
              type="button"
              className="rounded-md border px-3 py-1.5 hover:bg-accent"
              onClick={() => this.setState({ error: null })}
            >
              重试
            </button>
            <button
              type="button"
              className="rounded-md border px-3 py-1.5 hover:bg-accent"
              onClick={() => window.location.reload()}
            >
              重载界面
            </button>
          </div>
        </div>
      );
    }
    return this.props.children;
  }
}
