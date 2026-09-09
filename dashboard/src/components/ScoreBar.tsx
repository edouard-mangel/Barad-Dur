import { scoreBgClass, scoreColor } from '../report/format'
import type { ScoreThresholds } from '../report/model'

interface Props {
  score: number
  thresholds: ScoreThresholds
  width?: string
  height?: string
  showLabel?: boolean
}

export default function ScoreBar({ score, thresholds, width = '100%', height = '6px', showLabel = false }: Props) {
  const bg = scoreBgClass(score, thresholds)
  const pct = Math.max(0, Math.min(100, score))

  return (
    <div className="flex items-center gap-2" style={{ width }}>
      <div
        className="relative flex-1 rounded-full overflow-hidden"
        style={{ height, backgroundColor: 'rgba(255,255,255,0.06)' }}
      >
        <div
          className={`h-full rounded-full transition-all duration-700 ${bg}`}
          style={{
            width: `${pct}%`,
            boxShadow: pct > 0 ? `0 0 8px currentColor` : 'none',
          }}
        />
      </div>
      {showLabel && (
        <span
          className={`font-mono text-xs font-semibold tabular-nums`}
          style={{ color: scoreColor(pct, thresholds), minWidth: '2.5rem', textAlign: 'right' }}
        >
          {pct}
        </span>
      )}
    </div>
  )
}
