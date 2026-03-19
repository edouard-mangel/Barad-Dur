interface Props {
  actions: string[]
}

// Parse format: "[Category] Metric (score: N) — recommendation"
function parseAction(action: string): { category: string; metric: string; score: number | null; recommendation: string } {
  const categoryMatch = action.match(/^\[([^\]]+)\]/)
  const scoreMatch = action.match(/\(score:\s*(\d+)\)/)
  const dashIndex = action.indexOf('—')

  const category = categoryMatch ? categoryMatch[1] : ''
  const score = scoreMatch ? parseInt(scoreMatch[1], 10) : null
  const recommendation = dashIndex !== -1 ? action.slice(dashIndex + 1).trim() : action
  const metric = action
    .replace(/^\[[^\]]+\]\s*/, '')
    .replace(/\s*\(score:\s*\d+\).*$/, '')
    .trim()

  return { category, metric, score, recommendation }
}

function borderColor(score: number | null): string {
  if (score === null) return '#f59e0b'
  if (score <= 40) return '#ef4444'
  if (score <= 70) return '#f59e0b'
  return '#10b981'
}

export default function TopActions({ actions }: Props) {
  if (actions.length === 0) return null

  return (
    <div
      style={{
        borderRadius: 10,
        border: '1px solid rgba(255,255,255,0.06)',
        backgroundColor: 'rgba(255,255,255,0.02)',
        padding: '1.25rem',
      }}
    >
      <h3
        style={{
          fontFamily: 'Syne, sans-serif',
          fontWeight: 700,
          fontSize: '0.85rem',
          color: 'rgba(226, 232, 240, 0.5)',
          letterSpacing: '0.12em',
          textTransform: 'uppercase',
          marginBottom: '1rem',
        }}
      >
        Top Actions
      </h3>

      <div style={{ display: 'flex', flexDirection: 'column', gap: '0.6rem' }}>
        {actions.map((action, i) => {
          const { category, metric, score, recommendation } = parseAction(action)
          const bc = borderColor(score)

          return (
            <div
              key={i}
              style={{
                display: 'flex',
                gap: '1rem',
                alignItems: 'flex-start',
                padding: '0.85rem 1rem',
                borderRadius: 8,
                borderLeft: `3px solid ${bc}`,
                backgroundColor: 'rgba(255,255,255,0.02)',
              }}
            >
              {/* Number */}
              <span
                style={{
                  fontFamily: 'JetBrains Mono, monospace',
                  fontWeight: 600,
                  fontSize: '0.8rem',
                  color: bc,
                  flexShrink: 0,
                  opacity: 0.8,
                  minWidth: '1.2rem',
                }}
              >
                {String(i + 1).padStart(2, '0')}
              </span>

              <div style={{ flex: 1, minWidth: 0 }}>
                {/* Category + metric */}
                <div className="flex items-center gap-2 flex-wrap" style={{ marginBottom: '0.3rem' }}>
                  {category && (
                    <span
                      style={{
                        fontFamily: 'JetBrains Mono, monospace',
                        fontSize: '0.65rem',
                        color: bc,
                        opacity: 0.7,
                        border: `1px solid ${bc}`,
                        borderRadius: 4,
                        padding: '1px 6px',
                        letterSpacing: '0.05em',
                      }}
                    >
                      {category}
                    </span>
                  )}
                  <span
                    style={{
                      fontFamily: 'Syne, sans-serif',
                      fontWeight: 700,
                      fontSize: '0.82rem',
                      color: 'rgba(226, 232, 240, 0.9)',
                    }}
                  >
                    {metric}
                  </span>
                  {score !== null && (
                    <span
                      style={{
                        fontFamily: 'JetBrains Mono, monospace',
                        fontSize: '0.7rem',
                        color: bc,
                        opacity: 0.8,
                      }}
                    >
                      {score}
                    </span>
                  )}
                </div>

                <p
                  style={{
                    fontFamily: 'Syne, sans-serif',
                    fontSize: '0.78rem',
                    color: 'rgba(148, 163, 184, 0.7)',
                    lineHeight: 1.5,
                    margin: 0,
                  }}
                >
                  {recommendation}
                </p>
              </div>
            </div>
          )
        })}
      </div>
    </div>
  )
}
