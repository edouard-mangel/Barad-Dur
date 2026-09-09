import { describe, expect, it } from 'vitest'
import fixture from '../../../tests/fixtures/report-contract/current'
import type { AnalysisReport } from './generated/AnalysisReport'

const fixtureMustMatchGeneratedContract: AnalysisReport = fixture

describe('generated report contract', () => {
  it('type-checks the producer-owned fixture', () => {
    expect(fixtureMustMatchGeneratedContract.repo_name).toBe('contract-fixture')
  })
})
