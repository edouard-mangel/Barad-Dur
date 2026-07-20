import { describe, it, expect } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import HotspotsView, { couplingBadge, riskColor, splitPath, visibleSorted } from './HotspotsView'
import type { HotspotFile } from '../types'

function file(path: string, score: number, churn = 10): HotspotFile {
  return {
    path,
    churn_count: churn,
    bug_commit_count: 0,
    loc: 100,
    total_lines: 120,
    cyclomatic_complexity: 5,
    public_methods: 2,
    properties: 1,
    hotspot_score: score,
  }
}

describe('HotspotsView helpers', () => {
  it('visibleSorted filters dismissals, sorts by the active column, caps at 50', () => {
    const many = Array.from({ length: 60 }, (_, i) => file(`f${i}.ts`, i, 100 - i))
    const byScore = visibleSorted(many, new Set(), 'hotspot_score')
    expect(byScore.length).toBe(50)
    expect(byScore[0].path).toBe('f59.ts')
    const byChurn = visibleSorted(many, new Set(['f0.ts']), 'churn_count')
    expect(byChurn[0].path).toBe('f1.ts') // f0 (churn 100) dismissed
  })

  it('couplingBadge joins only the non-zero kinds', () => {
    expect(couplingBadge(file('a', 1))).toBe('')
    expect(couplingBadge({ ...file('a', 1), content_findings: 1, control_findings: 3 })).toBe('Cn 1 · Ct 3')
  })

  it('riskColor follows the 40/70 bands', () => {
    expect(riskColor(30)).toBe('#10b981')
    expect(riskColor(50)).toBe('#f59e0b')
    expect(riskColor(80)).toBe('#ef4444')
  })

  it('splitPath separates directory prefix from file name', () => {
    expect(splitPath('src/a/b.rs')).toEqual({ dir: 'src/a/', name: 'b.rs' })
    expect(splitPath('README.md')).toEqual({ dir: '', name: 'README.md' })
  })
})

describe('HotspotsView dismiss', () => {
  it('removes a file from the table when its dismiss control is clicked', () => {
    render(<HotspotsView files={[file('package.json', 90), file('src/main.ts', 80)]} />)

    // both files are present initially (queried by the row's title = full path)
    expect(screen.queryByTitle('package.json')).not.toBeNull()
    expect(screen.queryByTitle('src/main.ts')).not.toBeNull()

    // dismiss the first (highest-score) row — package.json
    const dismissButtons = screen.getAllByRole('button', { name: /dismiss/i })
    fireEvent.click(dismissButtons[0])

    expect(screen.queryByTitle('package.json')).toBeNull()
    expect(screen.queryByTitle('src/main.ts')).not.toBeNull()
  })

  it('restores dismissed files when "Reset dismissed" is clicked', () => {
    render(<HotspotsView files={[file('package.json', 90), file('src/main.ts', 80)]} />)

    fireEvent.click(screen.getAllByRole('button', { name: /dismiss/i })[0])
    expect(screen.queryByTitle('package.json')).toBeNull()

    fireEvent.click(screen.getByRole('button', { name: /reset dismissed/i }))
    expect(screen.queryByTitle('package.json')).not.toBeNull()
  })

  // AC-D3: dismissal is keyed by file path, so re-sorting the table by a different
  // column must keep the same file hidden — not whatever row now sits in its old slot.
  it('keeps a dismissed file hidden after the table is re-sorted by another column', () => {
    // score order and churn order are deliberately reversed
    render(<HotspotsView files={[file('high-score.ts', 90, 1), file('high-churn.ts', 10, 99)]} />)

    // default sort is score → the first dismiss button is high-score.ts (top row)
    fireEvent.click(screen.getAllByRole('button', { name: /dismiss/i })[0])
    expect(screen.queryByTitle('high-score.ts')).toBeNull()
    expect(screen.queryByTitle('high-churn.ts')).not.toBeNull()

    // re-sort by churn — order would flip; dismissal-by-path must persist
    fireEvent.click(screen.getByRole('columnheader', { name: /churn/i }))

    expect(screen.queryByTitle('high-score.ts')).toBeNull() // still dismissed
    expect(screen.queryByTitle('high-churn.ts')).not.toBeNull() // unaffected
  })
})

describe('HotspotsView coupling badge', () => {
  it('shows per-kind counts when findings are present', () => {
    const flagged: HotspotFile = { ...file('src/glob.ts', 80), common_findings: 2, control_findings: 1 }
    render(<HotspotsView files={[flagged]} />)
    expect(screen.queryByText('Cm 2 · Ct 1')).not.toBeNull()
  })

  it('renders an em dash for files without findings (and for pre-M4 reports)', () => {
    render(<HotspotsView files={[file('src/clean.ts', 70)]} />)
    // one em dash from Bugs (0 bugs) + one from Coupling
    expect(screen.getAllByText('—').length).toBe(2)
  })

  it('shows the inheritance badge; reports without the field render unchanged', () => {
    const deep: HotspotFile = { ...file('src/deep.ts', 80), inheritance_findings: 2 }
    const old: HotspotFile = file('src/old.ts', 70) // pre-M7 report shape
    render(<HotspotsView files={[deep, old]} />)
    expect(screen.queryByText('Ih 2')).not.toBeNull()
    expect(screen.queryByTitle('src/old.ts')).not.toBeNull()
  })
})
