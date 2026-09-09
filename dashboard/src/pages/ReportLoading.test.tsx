import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { beforeEach, describe, expect, it } from 'vitest'
import { MemoryRouter } from 'react-router'
import App from '../App'
import currentReport from '../../../tests/fixtures/report-contract/current'

const validText = JSON.stringify(currentReport)

describe('report loading', () => {
  beforeEach(() => sessionStorage.clear())

  it('validates, stores, restores, and renders an uploaded report through the router', async () => {
    const { container } = render(
      <MemoryRouter initialEntries={['/']}>
        <App />
      </MemoryRouter>,
    )
    const input = container.querySelector('input[type="file"]') as HTMLInputElement
    const file = new File([validText], 'report.json', { type: 'application/json' })

    fireEvent.change(input, { target: { files: [file] } })

    expect(await screen.findByText('contract-fixture')).not.toBeNull()
    expect(sessionStorage.getItem('barad-dur-report')).toBe(validText)
  })

  it('restores a valid current report from session storage', async () => {
    sessionStorage.setItem('barad-dur-report', validText)

    render(
      <MemoryRouter initialEntries={['/report']}>
        <App />
      </MemoryRouter>,
    )

    expect(await screen.findByText('contract-fixture')).not.toBeNull()
  })

  it('returns corrupt restored data to upload with guidance and preserves it', async () => {
    const rejected = '{"repo_name":"old"}'
    sessionStorage.setItem('barad-dur-report', rejected)

    render(
      <MemoryRouter initialEntries={['/report']}>
        <App />
      </MemoryRouter>,
    )

    await waitFor(() => {
      expect(screen.queryByText(/Regenerate the report with the installed CLI/)).not.toBeNull()
    })
    expect(sessionStorage.getItem('barad-dur-report')).toBe(rejected)
  })

  it('returns missing session data to the upload flow', async () => {
    render(
      <MemoryRouter initialEntries={['/report']}>
        <App />
      </MemoryRouter>,
    )

    expect(await screen.findByText('Drop your report.json here')).not.toBeNull()
  })
})
