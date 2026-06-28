import { describe, it, expect } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import HotspotsView from './HotspotsView'
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
