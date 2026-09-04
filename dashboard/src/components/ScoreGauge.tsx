import { scoreColor } from '../types'

interface Props {
  /** null: nothing was measurable — empty track and a dash, never a zero. */
  score: number | null
  size?: number
  label?: string
}

export default function ScoreGauge({ score, size = 160, label = 'Overall Score' }: Props) {
  const unscored = score === null
  const clampedScore = unscored ? 0 : Math.max(0, Math.min(100, score))
  const color = unscored ? 'rgba(148, 163, 184, 0.8)' : scoreColor(clampedScore)

  // Arc parameters — 270° sweep starting from bottom-left
  const radius = 54
  const cx = 80
  const cy = 80
  const strokeWidth = 8
  const circumference = 2 * Math.PI * radius
  const arcFraction = 0.75 // 270 degrees out of 360
  const arcLength = circumference * arcFraction
  const offset = arcLength - (clampedScore / 100) * arcLength

  // Rotation so arc starts from bottom-left (225°)
  const rotation = 135

  return (
    <div className="flex flex-col items-center gap-1">
      <div className="relative" style={{ width: size, height: size }}>
        <svg
          width={size}
          height={size}
          viewBox="0 0 160 160"
          style={{ overflow: 'visible' }}
        >
          {/* Background track */}
          <circle
            cx={cx}
            cy={cy}
            r={radius}
            fill="none"
            stroke="rgba(255,255,255,0.06)"
            strokeWidth={strokeWidth}
            strokeDasharray={`${arcLength} ${circumference}`}
            strokeDashoffset={0}
            strokeLinecap="round"
            transform={`rotate(${rotation} ${cx} ${cy})`}
          />
          {/* Score arc */}
          <circle
            cx={cx}
            cy={cy}
            r={radius}
            fill="none"
            stroke={color}
            strokeWidth={strokeWidth}
            strokeDasharray={`${arcLength} ${circumference}`}
            strokeDashoffset={offset}
            strokeLinecap="round"
            transform={`rotate(${rotation} ${cx} ${cy})`}
            style={{
              filter: `drop-shadow(0 0 6px ${color})`,
              transition: 'stroke-dashoffset 1s cubic-bezier(0.4, 0, 0.2, 1)',
            }}
          />
          {/* Score number */}
          <text
            x={cx}
            y={cy - 4}
            textAnchor="middle"
            dominantBaseline="middle"
            fill={color}
            fontSize="36"
            fontFamily="JetBrains Mono, monospace"
            fontWeight="600"
            style={{ filter: `drop-shadow(0 0 8px ${color})` }}
          >
            {unscored ? "—" : clampedScore}
          </text>
          <text
            x={cx}
            y={cy + 24}
            textAnchor="middle"
            fill="rgba(148, 163, 184, 0.7)"
            fontSize="10"
            fontFamily="Syne, sans-serif"
            letterSpacing="0.08em"
          >
            / 100
          </text>
        </svg>
      </div>
      <span
        className="text-xs font-semibold tracking-widest uppercase"
        style={{ color: 'rgba(148, 163, 184, 0.6)', fontFamily: 'Syne, sans-serif', letterSpacing: '0.15em' }}
      >
        {label}
      </span>
    </div>
  )
}
