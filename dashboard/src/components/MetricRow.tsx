import { useState } from 'react'
import type { MetricValue } from '../types'
import { scoreClass, formatRawValue } from '../types'
import ScoreBar from './ScoreBar'

interface Props {
  metric: MetricValue
}

export default function MetricRow({ metric }: Props) {
  const [showTooltip, setShowTooltip] = useState(false)

  return (
    <div
      style={{
        display: 'grid',
        gridTemplateColumns: '1fr 120px 2.5rem',
        alignItems: 'center',
        gap: '0.75rem',
        padding: '0.5rem 0',
        borderBottom: '1px solid rgba(255,255,255,0.04)',
        position: 'relative',
      }}
    >
      {/* Name + raw value */}
      <div>
        <div className="flex items-center gap-2">
          <span
            style={{
              fontFamily: 'Syne, sans-serif',
              fontWeight: 600,
              fontSize: '0.8rem',
              color: 'rgba(226, 232, 240, 0.9)',
            }}
          >
            {metric.name}
          </span>
          {/* Info button */}
          <button
            onMouseEnter={() => setShowTooltip(true)}
            onMouseLeave={() => setShowTooltip(false)}
            style={{
              background: 'none',
              border: '1px solid rgba(255,255,255,0.12)',
              borderRadius: '50%',
              width: 14,
              height: 14,
              fontSize: 9,
              color: 'rgba(148, 163, 184, 0.5)',
              cursor: 'help',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              flexShrink: 0,
              padding: 0,
            }}
          >
            ?
          </button>
          {showTooltip && (
            <div
              style={{
                position: 'absolute',
                left: 0,
                top: '100%',
                zIndex: 10,
                backgroundColor: '#131921',
                border: '1px solid rgba(255,255,255,0.1)',
                borderRadius: 8,
                padding: '0.6rem 0.8rem',
                maxWidth: 280,
                fontFamily: 'Syne, sans-serif',
                fontSize: '0.72rem',
                color: 'rgba(226, 232, 240, 0.7)',
                lineHeight: 1.5,
                boxShadow: '0 8px 32px rgba(0,0,0,0.6)',
                pointerEvents: 'none',
              }}
            >
              {metric.description}
            </div>
          )}
        </div>
        <span
          style={{
            fontFamily: 'JetBrains Mono, monospace',
            fontSize: '0.67rem',
            color: 'rgba(148, 163, 184, 0.45)',
          }}
        >
          {formatRawValue(metric.raw_value)}
        </span>
      </div>

      {/* Score bar */}
      <ScoreBar score={metric.score} height="4px" />

      {/* Score number */}
      <span
        className={scoreClass(metric.score)}
        style={{
          fontFamily: 'JetBrains Mono, monospace',
          fontSize: '0.75rem',
          fontWeight: 600,
          textAlign: 'right',
          tabularNums: 'true',
        } as React.CSSProperties}
      >
        {metric.score}
      </span>
    </div>
  )
}
