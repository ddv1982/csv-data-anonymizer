import { Electroview } from 'electrobun/view'
import type { CsvAnonymizerApi } from '../shared/contracts'
import type { CsvAnonymizerRpcSchema } from '../bun/rpc-schema'

declare global {
  interface Window {
    csvAnonymizer?: CsvAnonymizerApi
  }
}

const electrobunWindow = window as Window & { __electrobun?: unknown }

if (electrobunWindow.__electrobun && !window.csvAnonymizer) {
  const rpc = Electroview.defineRPC<CsvAnonymizerRpcSchema>({
    maxRequestTime: 5 * 60 * 1000,
    handlers: {}
  })

  new Electroview({ rpc })

  window.csvAnonymizer = {
    getHealth: () => rpc.request.getHealth(),
    getSettings: () => rpc.request.getSettings(),
    updateSettings: (input) => rpc.request.updateSettings(input),
    selectCsvFile: () => rpc.request.selectCsvFile(),
    selectOutputFile: (input) => rpc.request.selectOutputFile(input),
    showOutputInFolder: (input) => rpc.request.showOutputInFolder(input),
    getHeaders: (input) => rpc.request.getHeaders(input),
    getPreview: (input) => rpc.request.getPreview(input),
    anonymizeFile: (input) => rpc.request.anonymizeFile(input)
  }
}
