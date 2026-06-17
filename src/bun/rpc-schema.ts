import type {
  ActionData,
  AnonymizeData,
  AnonymizeParams,
  ApiResult,
  AppSettings,
  AppSettingsPatch,
  FileDialogData,
  GetHeadersParams,
  GetPreviewParams,
  HeadersData,
  HealthData,
  OutputPathDialogParams,
  PreviewData,
  ShowItemParams
} from '../shared/contracts'

type NoMessages = Record<never, never>
type NoRequests = Record<never, never>

export interface CsvAnonymizerRpcSchema {
  bun: {
    requests: {
      getHealth: { params: undefined; response: ApiResult<HealthData> }
      getSettings: { params: undefined; response: ApiResult<AppSettings> }
      updateSettings: { params: AppSettingsPatch; response: ApiResult<AppSettings> }
      selectCsvFile: { params: undefined; response: ApiResult<FileDialogData> }
      selectOutputFile: { params: OutputPathDialogParams | undefined; response: ApiResult<FileDialogData> }
      showOutputInFolder: { params: ShowItemParams; response: ApiResult<ActionData> }
      getHeaders: { params: GetHeadersParams; response: ApiResult<HeadersData> }
      getPreview: { params: GetPreviewParams; response: ApiResult<PreviewData> }
      anonymizeFile: { params: AnonymizeParams; response: ApiResult<AnonymizeData> }
    }
    messages: NoMessages
  }
  webview: {
    requests: NoRequests
    messages: NoMessages
  }
}
