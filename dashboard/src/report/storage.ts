import { decodeReport } from './decode'
import type { DashboardReport } from './model'

export const REPORT_STORAGE_KEY = 'barad-dur-report'

type ReportLoadErrorKind =
  | 'json'
  | 'decode'
  | 'file-read'
  | 'storage-unavailable'
  | 'storage-quota'

export class ReportLoadError extends Error {
  constructor(public readonly kind: ReportLoadErrorKind, message: string) {
    super(message)
    this.name = 'ReportLoadError'
  }
}

export interface StoredReport {
  report: DashboardReport
  text: string
}

export function parseReportText(text: string): StoredReport {
  let input: unknown
  try {
    input = JSON.parse(text) as unknown
  } catch {
    throw new ReportLoadError('json', 'Invalid JSON syntax. Make sure the report file is complete.')
  }

  try {
    return { report: decodeReport(input), text }
  } catch (error) {
    const reason = error instanceof Error ? error.message : 'unknown report shape'
    throw new ReportLoadError(
      'decode',
      `${reason}. Regenerate the report with the installed CLI: barad-dur analyze . --json`,
    )
  }
}

export function storeUploadedReport(storage: Storage, text: string): StoredReport {
  const parsed = parseReportText(text)
  try {
    storage.setItem(REPORT_STORAGE_KEY, text)
  } catch (error) {
    throw storageError(error)
  }
  return parsed
}

export function restoreReport(storage: Storage): StoredReport | null {
  let text: string | null
  try {
    text = storage.getItem(REPORT_STORAGE_KEY)
  } catch (error) {
    throw storageError(error)
  }
  return text === null ? null : parseReportText(text)
}

export const fileReadError = (): ReportLoadError =>
  new ReportLoadError('file-read', 'Failed to read the selected report file.')

const storageError = (error: unknown): ReportLoadError => {
  if (error instanceof DOMException && error.name === 'QuotaExceededError') {
    return new ReportLoadError('storage-quota', 'Browser session storage quota was exceeded.')
  }
  return new ReportLoadError('storage-unavailable', 'Browser session storage is unavailable.')
}
