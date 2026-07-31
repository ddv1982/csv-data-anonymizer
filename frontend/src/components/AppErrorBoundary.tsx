import { Component, type ErrorInfo, type ReactNode } from 'react'

type Props = { children: ReactNode }
type State = { failed: boolean; diagnosticCode: string | null }

export class AppErrorBoundary extends Component<Props, State> {
  state: State = { failed: false, diagnosticCode: null }

  static getDerivedStateFromError(error: Error): State {
    return { failed: true, diagnosticCode: diagnosticCodeFor(error) }
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error('The application UI could not be rendered.', error, info.componentStack)
  }

  render() {
    if (!this.state.failed) return this.props.children

    return (
      <main className="container app-main" role="alert">
        <div className="card app-recovery-card">
          <h1>CSV Anonymizer could not display this analysis</h1>
          <p>
            Your source file was not changed. Reload the application and select the file again.
          </p>
          {this.state.diagnosticCode ? (
            <p className="mono text-sm">Diagnostic: {this.state.diagnosticCode}</p>
          ) : null}
          <button type="button" className="button button-primary" onClick={() => window.location.reload()}>
            Reload application
          </button>
        </div>
      </main>
    )
  }
}

function diagnosticCodeFor(error: Error) {
  const input = `${error.name}:${error.message}:${error.stack ?? ''}`
  let hash = 2166136261
  for (let index = 0; index < input.length; index += 1) {
    hash ^= input.charCodeAt(index)
    hash = Math.imul(hash, 16777619)
  }
  return `UI-${(hash >>> 0).toString(16).padStart(8, '0').toUpperCase()}`
}
