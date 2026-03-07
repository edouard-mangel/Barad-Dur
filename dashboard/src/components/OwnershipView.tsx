import type { FileOwnership } from '../types'

interface Props {
  ownership: FileOwnership[]
}

const PALETTE = [
  '#f59e0b', '#10b981', '#3b82f6', '#a78bfa',
  '#f472b6', '#34d399', '#fb923c', '#60a5fa',
]

function authorColor(name: string, allAuthors: string[]): string {
  const idx = allAuthors.indexOf(name)
  return PALETTE[idx % PALETTE.length] ?? '#4a5568'
}

export default function OwnershipView({ ownership }: Props) {
  const allAuthors = Array.from(new Set(ownership.flatMap(f => f.authors.map(a => a.name))))

  const interesting = [...ownership]
    .sort((a, b) => {
      if (b.authors.length !== a.authors.length) return b.authors.length - a.authors.length
      return (a.authors[0]?.pct ?? 100) - (b.authors[0]?.pct ?? 100)
    })
    .slice(0, 60)

  if (interesting.length === 0) {
    return (
      <p style={{ fontFamily: 'JetBrains Mono', fontSize: '0.8rem', color: 'rgba(148,163,184,0.4)' }}>
        No ownership data available.
      </p>
    )
  }

  return (
    <div style={{ border: '1px solid rgba(255,255,255,0.06)', borderRadius: 10, padding: '1rem 1.25rem', backgroundColor: 'rgba(255,255,255,0.02)', overflowX: 'auto' }}>
      <p style={{ fontFamily: 'Syne', fontSize: '0.7rem', color: 'rgba(148,163,184,0.4)', letterSpacing: '0.1em', textTransform: 'uppercase', margin: '0 0 0.75rem' }}>
        Author ownership — blame distribution per file
      </p>

      {/* Legend */}
      <div style={{ display: 'flex', flexWrap: 'wrap', gap: '0.5rem', marginBottom: '1rem' }}>
        {allAuthors.slice(0, 8).map(name => (
          <div key={name} style={{ display: 'flex', alignItems: 'center', gap: '0.3rem' }}>
            <div style={{ width: 8, height: 8, borderRadius: '50%', backgroundColor: authorColor(name, allAuthors) }} />
            <span style={{ fontFamily: 'JetBrains Mono', fontSize: '0.65rem', color: 'rgba(148,163,184,0.6)' }}>{name}</span>
          </div>
        ))}
      </div>

      <table style={{ width: '100%', borderCollapse: 'collapse', fontFamily: 'JetBrains Mono', fontSize: '0.72rem' }}>
        <thead>
          <tr style={{ borderBottom: '1px solid rgba(255,255,255,0.08)', color: 'rgba(148,163,184,0.5)', fontSize: '0.65rem', letterSpacing: '0.08em', textTransform: 'uppercase' }}>
            <th style={{ textAlign: 'left', padding: '0.4rem 0.5rem', fontWeight: 400 }}>File</th>
            <th style={{ textAlign: 'left', padding: '0.4rem 0.5rem', fontWeight: 400 }}>Ownership</th>
            <th style={{ textAlign: 'right', padding: '0.4rem 0.5rem', fontWeight: 400 }}>Authors</th>
          </tr>
        </thead>
        <tbody>
          {interesting.map((f, i) => {
            const dir = f.path.includes('/') ? f.path.substring(0, f.path.lastIndexOf('/') + 1) : ''
            const name = f.path.split('/').pop() ?? f.path
            return (
              <tr key={i} style={{ borderBottom: '1px solid rgba(255,255,255,0.03)' }}>
                <td style={{ padding: '0.4rem 0.5rem', maxWidth: 220, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }} title={f.path}>
                  <span style={{ color: 'rgba(226,232,240,0.85)' }}>{name}</span>
                  {dir && <span style={{ color: 'rgba(148,163,184,0.3)', marginLeft: '0.3rem', fontSize: '0.62rem' }}>{dir}</span>}
                </td>
                <td style={{ padding: '0.4rem 0.5rem', minWidth: 200 }}>
                  <div style={{ display: 'flex', height: 8, borderRadius: 4, overflow: 'hidden', gap: '1px' }}>
                    {f.authors.map((a, j) => (
                      <div
                        key={j}
                        title={`${a.name}: ${a.pct.toFixed(0)}%`}
                        style={{ width: `${a.pct}%`, backgroundColor: authorColor(a.name, allAuthors), flexShrink: 0 }}
                      />
                    ))}
                  </div>
                  <div style={{ marginTop: '0.2rem', fontSize: '0.62rem', color: 'rgba(148,163,184,0.4)' }}>
                    {f.authors[0] ? `${f.authors[0].name} ${f.authors[0].pct.toFixed(0)}%` : ''}
                  </div>
                </td>
                <td style={{ textAlign: 'right', padding: '0.4rem 0.5rem', color: f.authors.length > 3 ? '#f59e0b' : 'rgba(148,163,184,0.5)' }}>
                  {f.authors.length}
                </td>
              </tr>
            )
          })}
        </tbody>
      </table>
    </div>
  )
}
