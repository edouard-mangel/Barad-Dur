import { render } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import ScoreGauge from '../components/ScoreGauge'
import { formatRawValue, scoreClass, scoreColor } from './format'

describe('report formatting', () => {
  it('formats every tagged raw-value variant explicitly', () => {
    expect(formatRawValue({ Integer: -3 })).toBe('-3')
    expect(formatRawValue({ Float: 1.25 })).toBe('1.25')
    expect(formatRawValue({ Percentage: 12.6 })).toBe('13%')
    expect(formatRawValue({ Count: 4 })).toBe('4')
    expect(formatRawValue({ Text: 'N/A' })).toBe('N/A')
    expect(formatRawValue({ List: ['a', 'b'] })).toBe('a, b')
    expect(formatRawValue({ List: [] })).toBe('—')
  })

  it('uses explicit report thresholds for score colors and classes', () => {
    const strict = { good_min: 90, warn_min: 70 }
    const relaxed = { good_min: 60, warn_min: 30 }

    expect(scoreColor(65, strict)).toBe('#ef4444')
    expect(scoreColor(65, relaxed)).toBe('#10b981')
    expect(scoreClass(65, strict)).toBe('score-red')
    expect(scoreClass(65, relaxed)).toBe('score-green')
  })

  it('rerenders the same gauge score with colors from the new report thresholds', () => {
    const { container, rerender } = render(
      <ScoreGauge score={65} thresholds={{ good_min: 90, warn_min: 70 }} />,
    )
    const scoreArc = () => container.querySelectorAll('circle')[1]
    expect(scoreArc().getAttribute('stroke')).toBe('#ef4444')

    rerender(<ScoreGauge score={65} thresholds={{ good_min: 60, warn_min: 30 }} />)
    expect(scoreArc().getAttribute('stroke')).toBe('#10b981')
  })
})
