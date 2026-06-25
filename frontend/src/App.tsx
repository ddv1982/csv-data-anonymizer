import { Shield } from 'lucide-react'
import {
  AnonymizerWorkflowView,
  WorkflowErrorToast,
} from './components/workflow/AnonymizerWorkflowView'
import { ThemeModeToggle } from './components/ThemeModeToggle'
import { useAnonymizerWorkflow } from './hooks/useAnonymizerWorkflow'
import { normalizeThemeMode, useTheme } from './hooks/useTheme'

function App() {
  const workflow = useAnonymizerWorkflow()
  const themeMode = normalizeThemeMode(workflow.settings.themeMode)
  useTheme(themeMode)

  return (
    <div className="app-root">
      <header className="app-topbar">
        <div className="container app-topbar-inner">
          <Shield className="brand-icon" aria-hidden="true" />
          <h1>CSV Anonymizer</h1>
          <ThemeModeToggle themeMode={themeMode} onChange={(mode) => workflow.updateSetting('themeMode', mode)} />
        </div>
      </header>

      <WorkflowErrorToast error={workflow.error} onDismiss={() => workflow.setError(null)} />

      <main className="container app-main">
        <AnonymizerWorkflowView workflow={workflow} />
      </main>

      <footer className="app-footer">
        <div className="container">
          <p>CSV Anonymizer - Transform sensitive fields in CSV files</p>
        </div>
      </footer>
    </div>
  )
}

export default App
