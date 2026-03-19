import { scoreBgClass } from '../types'

interface Props {
  score: number
  width?: string
  height?: string
  showLabel?: boolean
}

export default function ScoreBar({ score, width = '100%', height = '6px', showLabel = false }: Props) {
  const bg = scoreBgClass(score)
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
          style={{ color: pct > 70 ? '#10b981' : pct > 40 ? '#f59e0b' : '#ef4444', minWidth: '2.5rem', textAlign: 'right' }}
        >
          {pct}
        </span>
      )}
    </div>
  )
}
