/// <reference types="vite/client" />

import type { CsvAnonymizerApi } from '../../shared/contracts'

declare global {
  interface Window {
    csvAnonymizer?: CsvAnonymizerApi
  }
}
