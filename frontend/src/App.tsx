import { Shield } from 'lucide-react'
import { useState } from 'react'
import {
  AnonymizerWorkflowView,
  WorkflowErrorToast,
} from './components/workflow/AnonymizerWorkflowView'
import { InputModeTabs, type InputMode } from './components/InputModeTabs'
import { LocalAiTopbarControl } from './components/LocalAiTopbarControl'
import { PasteDataWorkflowView } from './components/PasteDataWorkflowView'
import { QuickDataTypeWorkflowView } from './components/QuickDataTypeWorkflowView'
import { ThemeModeToggle } from './components/ThemeModeToggle'
import { useAnonymizerWorkflow } from './hooks/useAnonymizerWorkflow'
import { normalizeThemeMode, useTheme } from './hooks/useTheme'

function App() {
  const workflow = useAnonymizerWorkflow()
  const [activeMode, setActiveMode] = useState<InputMode>('csv')
  const [localAiSettingsOpen, setLocalAiSettingsOpen] = useState(false)
  const [pasteBusy, setPasteBusy] = useState(false)
  const [quickBusy, setQuickBusy] = useState(false)
  const themeMode = normalizeThemeMode(workflow.settings.themeMode)
  const anyWorkflowBusy = workflow.isLoading || pasteBusy || quickBusy
  useTheme(themeMode)

  return (
    <div className="app-root">
      <header className="app-topbar">
        <div className="container app-topbar-inner">
          <Shield className="brand-icon" aria-hidden="true" />
          <h1>CSV Anonymizer</h1>
          <div className="app-topbar-actions">
            <LocalAiTopbarControl
              settings={workflow.settings}
              localAi={workflow.localAi}
              disabled={workflow.settingsDisabled || pasteBusy || quickBusy}
              settingsOpen={localAiSettingsOpen}
              onToggleSettings={setLocalAiSettingsOpen}
              onUpdateSetting={workflow.updateSetting}
            />
            <ThemeModeToggle
              themeMode={themeMode}
              disabled={workflow.settingsDisabled || pasteBusy || quickBusy}
              onChange={(mode) => workflow.updateSetting('themeMode', mode)}
            />
          </div>
        </div>
      </header>

      <WorkflowErrorToast error={workflow.error} onDismiss={() => workflow.setError(null)} />

      <main className="container app-main">
        <InputModeTabs activeMode={activeMode} disabled={anyWorkflowBusy} onChange={setActiveMode} />

        <section
          id="input-mode-panel-csv"
          role="tabpanel"
          aria-labelledby="input-mode-tab-csv"
          hidden={activeMode !== 'csv'}
          className="mode-panel"
        >
          <AnonymizerWorkflowView workflow={workflow} onOpenLocalAiSettings={() => setLocalAiSettingsOpen(true)} />
        </section>

        <section
          id="input-mode-panel-paste"
          role="tabpanel"
          aria-labelledby="input-mode-tab-paste"
          hidden={activeMode !== 'paste'}
          className="mode-panel"
        >
          <PasteDataWorkflowView
            settings={workflow.settings}
            settingsLoaded={workflow.settingsLoaded}
            localAi={workflow.localAi}
            onOpenLocalAiSettings={() => setLocalAiSettingsOpen(true)}
            onError={workflow.setError}
            onBusyChange={setPasteBusy}
          />
        </section>

        <section
          id="input-mode-panel-quick"
          role="tabpanel"
          aria-labelledby="input-mode-tab-quick"
          hidden={activeMode !== 'quick'}
          className="mode-panel"
        >
          <QuickDataTypeWorkflowView
            settingsLoaded={workflow.settingsLoaded}
            localAi={workflow.localAi}
            onOpenLocalAiSettings={() => setLocalAiSettingsOpen(true)}
            onError={workflow.setError}
            onBusyChange={setQuickBusy}
          />
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
