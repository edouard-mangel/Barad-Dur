import { useState } from 'react'
import type { CategoryResult } from '../types'
import { scoreClass, scoreColor } from '../types'
import ScoreBar from './ScoreBar'
import MetricRow from './MetricRow'

interface Props {
  category: CategoryResult
}

export default function CategoryCard({ category }: Props) {
  // Auto-expand if score is low
  const [expanded, setExpanded] = useState(category.score < 70)

  return (
    <div
      style={{
        borderRadius: 10,
        border: '1px solid rgba(255,255,255,0.06)',
        backgroundColor: 'rgba(255,255,255,0.02)',
        overflow: 'hidden',
        transition: 'border-color 0.2s ease',
      }}
      onMouseEnter={e => {
        ;(e.currentTarget as HTMLDivElement).style.borderColor = 'rgba(255,255,255,0.1)'
      }}
      onMouseLeave={e => {
        ;(e.currentTarget as HTMLDivElement).style.borderColor = 'rgba(255,255,255,0.06)'
      }}
    >
      {/* Header */}
      <button
        onClick={() => setExpanded(e => !e)}
        style={{
          width: '100%',
          display: 'flex',
          alignItems: 'center',
          gap: '1rem',
          padding: '1rem 1.25rem',
          background: 'none',
          border: 'none',
          cursor: 'pointer',
          textAlign: 'left',
        }}
      >
        {/* Collapse indicator */}
        <span
          style={{
            color: scoreColor(category.score),
            fontFamily: 'JetBrains Mono, monospace',
            fontSize: '0.6rem',
            opacity: 0.7,
            transform: expanded ? 'rotate(90deg)' : 'none',
            transition: 'transform 0.2s ease',
            display: 'inline-block',
            flexShrink: 0,
          }}
        >
          ▶
        </span>

        <span
          style={{
            fontFamily: 'Syne, sans-serif',
            fontWeight: 700,
            fontSize: '0.9rem',
            color: 'rgba(226, 232, 240, 0.95)',
            flex: 1,
          }}
        >
          {category.name}
        </span>

        {/* Score */}
        <span
          className={scoreClass(category.score)}
          style={{
            fontFamily: 'JetBrains Mono, monospace',
            fontWeight: 600,
            fontSize: '1.1rem',
            flexShrink: 0,
          }}
        >
          {category.score}
        </span>
      </button>

      {/* Score bar */}
      <div style={{ paddingLeft: '1.25rem', paddingRight: '1.25rem', paddingBottom: '0.75rem' }}>
        <ScoreBar score={category.score} height="5px" />
      </div>

      {/* Metrics */}
      {expanded && (
        <div
          style={{
            padding: '0.25rem 1.25rem 1rem',
            borderTop: '1px solid rgba(255,255,255,0.05)',
          }}
        >
          {category.metrics.map((m, i) => (
            <MetricRow key={i} metric={m} />
          ))}
        </div>
      )}
    </div>
  )
}
