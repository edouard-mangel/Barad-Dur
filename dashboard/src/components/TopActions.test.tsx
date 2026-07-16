import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import TopActions from './TopActions'

describe('TopActions', () => {
  it('renders ActionItem objects as serialized by the Rust scorer', () => {
    render(
      <TopActions
        actions={[
          {
            text: '[Complexity] Cyclomatic Complexity (score: 35) — Break down large functions',
            target_tab: 'hotspots',
            sort_by: 'complexity',
          },
        ]}
      />,
    )
    expect(screen.queryByText('Complexity')).not.toBeNull()
    expect(screen.queryByText('Cyclomatic Complexity')).not.toBeNull()
    expect(screen.queryByText('Break down large functions')).not.toBeNull()
  })

  it('still renders plain strings from reports generated before ActionItem', () => {
    render(
      <TopActions
        actions={['[Health] Bus Factor (score: 20) — Spread ownership across the team']}
      />,
    )
    expect(screen.queryByText('Health')).not.toBeNull()
    expect(screen.queryByText('Bus Factor')).not.toBeNull()
    expect(screen.queryByText('Spread ownership across the team')).not.toBeNull()
  })
})
