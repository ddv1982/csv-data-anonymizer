import { Shield } from 'lucide-react'
import { useState } from 'react'
import {
  AnonymizerWorkflowView,
  WorkflowErrorToast,
} from './components/workflow/AnonymizerWorkflowView'
import { InputModeTabs, type InputMode } from './components/InputModeTabs'
import { PasteDataWorkflowView } from './components/PasteDataWorkflowView'
import { QuickDataTypeWorkflowView } from './components/QuickDataTypeWorkflowView'
import { ThemeModeToggle } from './components/ThemeModeToggle'
import { useAnonymizerWorkflow } from './hooks/useAnonymizerWorkflow'
import { normalizeThemeMode, useTheme } from './hooks/useTheme'

function App() {
  const workflow = useAnonymizerWorkflow()
  const [activeMode, setActiveMode] = useState<InputMode>('csv')
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
        <InputModeTabs activeMode={activeMode} onChange={setActiveMode} />

        <section
          id="input-mode-panel-csv"
          role="tabpanel"
          aria-labelledby="input-mode-tab-csv"
          hidden={activeMode !== 'csv'}
          className="mode-panel"
        >
          {activeMode === 'csv' ? <AnonymizerWorkflowView workflow={workflow} /> : null}
        </section>

        <section
          id="input-mode-panel-paste"
          role="tabpanel"
          aria-labelledby="input-mode-tab-paste"
          hidden={activeMode !== 'paste'}
          className="mode-panel"
        >
          {activeMode === 'paste' ? (
            <PasteDataWorkflowView
              settings={workflow.settings}
              localAi={workflow.localAi}
              onUpdateSetting={workflow.updateSetting}
              onError={workflow.setError}
            />
          ) : null}
        </section>

        <section
          id="input-mode-panel-quick"
          role="tabpanel"
          aria-labelledby="input-mode-tab-quick"
          hidden={activeMode !== 'quick'}
          className="mode-panel"
        >
          {activeMode === 'quick' ? (
            <QuickDataTypeWorkflowView
              settings={workflow.settings}
              localAi={workflow.localAi}
              onUpdateSetting={workflow.updateSetting}
              onError={workflow.setError}
            />
          ) : null}
        </section>
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
