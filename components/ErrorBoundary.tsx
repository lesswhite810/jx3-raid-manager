import { Component, ErrorInfo, ReactNode } from 'react';

interface ErrorBoundaryProps {
  children: ReactNode;
}

interface ErrorBoundaryState {
  hasError: boolean;
  error: Error | null;
  errorInfo: ErrorInfo | null;
}

export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  constructor(props: ErrorBoundaryProps) {
    super(props);
    this.state = {
      hasError: false,
      error: null,
      errorInfo: null
    };
  }

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return {
      hasError: true,
      error,
      errorInfo: null
    };
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo): void {
    this.setState({
      error,
      errorInfo
    });
    console.error('Error caught by ErrorBoundary:', error, errorInfo);
  }

  render() {
    if (this.state.hasError) {
      return (
        <div className="min-h-screen bg-base flex items-center justify-center p-4">
          <div className="bg-surface rounded-xl shadow-lg p-6 max-w-2xl w-full">
            <h2 className="text-2xl font-bold text-red-600 dark:text-red-400 mb-4">应用出错了</h2>
            <div className="bg-red-50 dark:bg-red-900/20 rounded-lg p-4 mb-4">
              <p className="text-red-700 dark:text-red-300 font-medium mb-2">错误信息:</p>
              <pre className="text-sm text-red-600 dark:text-red-400 overflow-auto max-h-40">{this.state.error?.toString()}</pre>
            </div>
            <div className="bg-base rounded-lg p-4 mb-4">
              <p className="text-main font-medium mb-2">错误堆栈:</p>
              <pre className="text-xs text-muted overflow-auto max-h-60">{this.state.errorInfo?.componentStack}</pre>
            </div>
            <div className="flex justify-center">
              <button
                onClick={() => window.location.reload()}
                className="px-4 py-2 bg-primary text-white rounded-lg hover:bg-primary-hover transition-colors"
              >
                重新加载应用
              </button>
            </div>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}
