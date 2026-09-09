import { describe, expect, it } from 'vitest'
import currentReport from '../../../tests/fixtures/report-contract/current'
import { decodeReport } from './decode'

const clone = (): Record<string, unknown> =>
  structuredClone(currentReport) as unknown as Record<string, unknown>

const categories = (input: Record<string, unknown>): Record<string, unknown>[] =>
  input.categories as Record<string, unknown>[]

const metrics = (input: Record<string, unknown>): Record<string, unknown>[] =>
  categories(input)[0].metrics as Record<string, unknown>[]

function expectDecodeError(input: unknown, message: string): void {
  expect(() => decodeReport(input)).toThrow(message)
}

describe('decodeReport', () => {
  it('constructs the dashboard projection from the current producer fixture', () => {
    const input = clone()
    input.future_addition = { ignored: true }

    const report = decodeReport(input)

    expect(report.repo_name).toBe('contract-fixture')
    expect(report.overall_score).toBeNull()
    expect(report.categories[0].metrics.map(metric => Object.keys(metric.raw_value)[0]))
      .toEqual(['Integer', 'Float', 'Percentage', 'Count', 'Text', 'List'])
    expect(report.top_actions[1].target_tab).toBeUndefined()
  })

  it('rejects primitive input and missing required identity fields', () => {
    expectDecodeError('report', 'report: expected object')

    const input = clone()
    delete input.repo_name
    expectDecodeError(input, 'report.repo_name: expected string')
  })

  it('accepts nullable scores but rejects fractional and out-of-range scores', () => {
    const nullable = clone()
    categories(nullable)[0].score = null
    metrics(nullable)[0].score = null
    expect(decodeReport(nullable).categories[0].score).toBeNull()

    const fractional = clone()
    metrics(fractional)[0].score = 70.5
    expectDecodeError(fractional, 'report.categories[0].metrics[0].score: expected integer from 0 to 100 or null')

    const outOfRange = clone()
    outOfRange.overall_score = 101
    expectDecodeError(outOfRange, 'report.overall_score: expected integer from 0 to 100 or null')
  })

  it('requires one recognized raw-value tag and validates its payload', () => {
    const primitive = clone()
    metrics(primitive)[0].raw_value = 3
    expectDecodeError(primitive, 'report.categories[0].metrics[0].raw_value: expected exactly one raw-value tag')

    const multiple = clone()
    metrics(multiple)[0].raw_value = { Integer: 1, Count: 1 }
    expectDecodeError(multiple, 'report.categories[0].metrics[0].raw_value: expected exactly one raw-value tag')

    const badList = clone()
    metrics(badList)[5].raw_value = { List: ['ok', 2] }
    expectDecodeError(badList, 'report.categories[0].metrics[5].raw_value.List[1]: expected string')

    const percentage = clone()
    metrics(percentage)[2].raw_value = { Percentage: -250.25 }
    expect(decodeReport(percentage).categories[0].metrics[2].raw_value).toEqual({ Percentage: -250.25 })
  })

  it('requires nonnegative integer counts and finite numeric measurements', () => {
    const count = clone()
    count.total_files = -1
    expectDecodeError(count, 'report.total_files: expected nonnegative integer')

    const measurement = clone()
    const hotspots = measurement.file_hotspots as Record<string, unknown>[]
    hotspots[0].hotspot_score = Number.POSITIVE_INFINITY
    expectDecodeError(measurement, 'report.file_hotspots[0].hotspot_score: expected finite number')
  })

  it('requires current hotspot roles, finding counts, and structured actions', () => {
    const missingRole = clone()
    delete (missingRole.file_hotspots as Record<string, unknown>[])[0].role
    expectDecodeError(missingRole, 'report.file_hotspots[0].role: expected one of source, test, config, docs, other')

    const missingFindings = clone()
    delete (missingFindings.file_hotspots as Record<string, unknown>[])[0].content_findings
    expectDecodeError(missingFindings, 'report.file_hotspots[0].content_findings: expected nonnegative integer')

    const stringAction = clone()
    stringAction.top_actions = ['legacy action']
    expectDecodeError(stringAction, 'report.top_actions[0]: expected object')

    const malformedOptional = clone()
    ;(malformedOptional.top_actions as Record<string, unknown>[])[1].target_tab = 3
    expectDecodeError(malformedOptional, 'report.top_actions[1].target_tab: expected string when present')
  })

  it('requires valid report thresholds with warn below good', () => {
    const missing = clone()
    delete missing.score_thresholds
    expectDecodeError(missing, 'report.score_thresholds: expected object')

    const reversed = clone()
    reversed.score_thresholds = { good_min: 40, warn_min: 40 }
    expectDecodeError(reversed, 'report.score_thresholds: warn_min must be below good_min')
  })

  it('rejects malformed nested ownership, coupling, remote, and date fields', () => {
    const ownership = clone()
    const owner = (ownership.author_ownership as Record<string, unknown>[])[0]
    ;(owner.authors as Record<string, unknown>[])[0].pct = '75'
    expectDecodeError(ownership, 'report.author_ownership[0].authors[0].pct: expected finite number')

    const coupling = clone()
    ;(coupling.coupling_pairs as Record<string, unknown>[])[0].co_changes = 1.5
    expectDecodeError(coupling, 'report.coupling_pairs[0].co_changes: expected nonnegative integer')

    const remote = clone()
    ;(remote.remote_meta as Record<string, unknown>).stars = -2
    expectDecodeError(remote, 'report.remote_meta.stars: expected nonnegative integer or null')

    const age = clone()
    ;(age.file_ages as Record<string, unknown>[])[0].last_modified = 'not-a-date'
    expectDecodeError(age, 'report.file_ages[0].last_modified: expected usable date string')
  })
})
