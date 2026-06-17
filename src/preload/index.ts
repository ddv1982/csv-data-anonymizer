import { contextBridge, ipcRenderer } from 'electron'
import type {
  AnonymizeParams,
  AppSettingsPatch,
  CsvAnonymizerApi,
  GetHeadersParams,
  GetPreviewParams,
  OutputPathDialogParams,
  ShowItemParams
} from '../shared/contracts'

const api: CsvAnonymizerApi = {
  getHealth: () => ipcRenderer.invoke('app:health'),
  getSettings: () => ipcRenderer.invoke('settings:get'),
  updateSettings: (input: AppSettingsPatch) => ipcRenderer.invoke('settings:update', input),
  selectCsvFile: () => ipcRenderer.invoke('dialog:select-csv'),
  selectOutputFile: (input?: OutputPathDialogParams) => ipcRenderer.invoke('dialog:select-output', input ?? {}),
  showOutputInFolder: (input: ShowItemParams) => ipcRenderer.invoke('shell:show-output', input),
  getHeaders: (input: GetHeadersParams) => ipcRenderer.invoke('csv:headers', input),
  getPreview: (input: GetPreviewParams) => ipcRenderer.invoke('csv:preview', input),
  anonymizeFile: (input: AnonymizeParams) => ipcRenderer.invoke('csv:anonymize', input)
}

contextBridge.exposeInMainWorld('csvAnonymizer', api)
