import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import TopActions from './TopActions'

describe('TopActions', () => {
  it('renders ActionItem objects as serialized by the Rust scorer', () => {
    render(
      <TopActions
        thresholds={{ good_min: 71, warn_min: 41 }}
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

})
