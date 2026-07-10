import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import CouplingView from './CouplingView'

describe('CouplingView coupling actions panel', () => {
  it('renders a Coupling Actions panel listing action text when present', () => {
    render(
      <CouplingView
        pairs={[]}
        actions={[
          { text: '[Coupling] src/globals.rs — 1 finding(s) (worst: common) — injected state advice' },
        ]}
      />,
    )
    expect(screen.queryByText('Coupling Actions')).not.toBeNull()
    expect(
      screen.queryByText(/\[Coupling\] src\/globals\.rs/),
    ).not.toBeNull()
  })

  it('renders no actions panel when actions are absent', () => {
    render(<CouplingView pairs={[]} />)
    expect(screen.queryByText('Coupling Actions')).toBeNull()
    // the existing empty-pairs message still shows
    expect(screen.queryByText(/No coupling pairs detected/)).not.toBeNull()
  })
})
