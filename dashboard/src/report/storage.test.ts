import { beforeEach, describe, expect, it } from 'vitest'
import currentReport from '../../../tests/fixtures/report-contract/current'
import { ReportLoadError, restoreReport, storeUploadedReport } from './storage'

const validText = JSON.stringify(currentReport)

describe('report storage', () => {
  beforeEach(() => sessionStorage.clear())

  it('validates an upload before storing its original JSON', () => {
    const restored = storeUploadedReport(sessionStorage, validText)

    expect(restored.report.repo_name).toBe('contract-fixture')
    expect(restored.text).toBe(validText)
    expect(sessionStorage.getItem('barad-dur-report')).toBe(validText)
  })

  it('does not overwrite a valid saved report with rejected input', () => {
    sessionStorage.setItem('barad-dur-report', validText)

    expect(() => storeUploadedReport(sessionStorage, '{"repo_name":3}')).toThrow(ReportLoadError)
    expect(sessionStorage.getItem('barad-dur-report')).toBe(validText)
  })

  it('returns null when no session report exists and revalidates one when present', () => {
    expect(restoreReport(sessionStorage)).toBeNull()
    sessionStorage.setItem('barad-dur-report', validText)

    expect(restoreReport(sessionStorage)?.report.repo_name).toBe('contract-fixture')
  })

  it('distinguishes JSON syntax and incompatible current-contract errors', () => {
    expectLoadError(() => storeUploadedReport(sessionStorage, '{'), 'json', 'Invalid JSON syntax')
    expectLoadError(
      () => storeUploadedReport(sessionStorage, '{"repo_name":"old"}'),
      'decode',
      'report.branch: expected string',
    )
  })

  it('distinguishes unavailable storage from quota failures', () => {
    const unavailable = storageThatThrows(new DOMException('blocked', 'SecurityError'))
    expectLoadError(() => restoreReport(unavailable), 'storage-unavailable', 'Browser session storage is unavailable')

    const quota = storageThatThrows(new DOMException('full', 'QuotaExceededError'), true)
    expectLoadError(() => storeUploadedReport(quota, validText), 'storage-quota', 'Browser session storage quota was exceeded')
  })
})

function expectLoadError(run: () => unknown, kind: ReportLoadError['kind'], message: string): void {
  try {
    run()
    throw new Error('expected ReportLoadError')
  } catch (error) {
    expect(error).toBeInstanceOf(ReportLoadError)
    expect((error as ReportLoadError).kind).toBe(kind)
    expect((error as Error).message).toContain(message)
  }
}

function storageThatThrows(error: DOMException, onlyOnSet = false): Storage {
  return {
    getItem: () => {
      if (!onlyOnSet) throw error
      return null
    },
    setItem: () => { throw error },
    removeItem: () => undefined,
    clear: () => undefined,
    key: () => null,
    length: 0,
  }
}
