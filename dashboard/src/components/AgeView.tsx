import type { FileAge } from '../types'

interface Props {
  ages: FileAge[]
}

function ageBand(days: number): { color: string; label: string } {
  if (days <= 30)  return { color: '#10b981', label: 'fresh' }
  if (days <= 90)  return { color: '#34d399', label: '< 3mo' }
  if (days <= 180) return { color: '#f59e0b', label: '< 6mo' }
  if (days <= 365) return { color: '#fb923c', label: '< 1yr' }
  return { color: '#ef4444', label: '> 1yr' }
}

export default function AgeView({ ages }: Props) {
  if (ages.length === 0) {
    return (
      <p style={{ fontFamily: 'JetBrains Mono', fontSize: '0.8rem', color: 'rgba(148,163,184,0.4)' }}>
        No age data available.
      </p>
    )
  }

  const maxDays = ages[0]?.days_since_modified ?? 1

  return (
    <div style={{ border: '1px solid rgba(255,255,255,0.06)', borderRadius: 10, padding: '1rem 1.25rem', backgroundColor: 'rgba(255,255,255,0.02)', overflowX: 'auto' }}>
      <p style={{ fontFamily: 'Syne', fontSize: '0.7rem', color: 'rgba(148,163,184,0.4)', letterSpacing: '0.1em', textTransform: 'uppercase', margin: '0 0 0.75rem' }}>
        Code age — sorted by staleness (oldest first)
      </p>
      <table style={{ width: '100%', borderCollapse: 'collapse', fontFamily: 'JetBrains Mono', fontSize: '0.72rem' }}>
        <thead>
          <tr style={{ borderBottom: '1px solid rgba(255,255,255,0.08)', color: 'rgba(148,163,184,0.5)', fontSize: '0.65rem', letterSpacing: '0.08em', textTransform: 'uppercase' }}>
            <th style={{ textAlign: 'left', padding: '0.4rem 0.5rem', fontWeight: 400 }}>File</th>
            <th style={{ textAlign: 'left', padding: '0.4rem 0.5rem', fontWeight: 400, minWidth: 180 }}>Age</th>
            <th style={{ textAlign: 'right', padding: '0.4rem 0.5rem', fontWeight: 400 }}>Days</th>
            <th style={{ textAlign: 'right', padding: '0.4rem 0.5rem', fontWeight: 400 }}>Last modified</th>
          </tr>
        </thead>
        <tbody>
          {ages.map((f, i) => {
            const { color, label } = ageBand(f.days_since_modified)
            const pct = (f.days_since_modified / maxDays) * 100
            const date = new Date(f.last_modified).toLocaleDateString('en-US', {
              year: 'numeric',
              month: 'short',
              day: 'numeric',
            })
            const dir = f.path.includes('/') ? f.path.substring(0, f.path.lastIndexOf('/') + 1) : ''
            const name = f.path.split('/').pop() ?? f.path
            return (
              <tr key={i} style={{ borderBottom: '1px solid rgba(255,255,255,0.03)' }}>
                <td style={{ padding: '0.4rem 0.5rem', maxWidth: 260, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }} title={f.path}>
                  <span style={{ color: 'rgba(226,232,240,0.85)' }}>{name}</span>
                  {dir && <span style={{ color: 'rgba(148,163,184,0.3)', marginLeft: '0.3rem', fontSize: '0.62rem' }}>{dir}</span>}
                </td>
                <td style={{ padding: '0.4rem 0.5rem' }}>
                  <div style={{ display: 'flex', alignItems: 'center', gap: '0.5rem' }}>
                    <div style={{ flex: 1, height: 4, backgroundColor: 'rgba(255,255,255,0.06)', borderRadius: 2 }}>
                      <div
                        style={{
                          width: `${pct}%`,
                          height: '100%',
                          backgroundColor: color,
                          borderRadius: 2,
                          boxShadow: `0 0 4px ${color}40`,
                        }}
                      />
                    </div>
                    <span style={{ color, fontSize: '0.65rem', minWidth: '2.8rem' }}>{label}</span>
                  </div>
                </td>
                <td style={{ textAlign: 'right', padding: '0.4rem 0.5rem', color: 'rgba(148,163,184,0.5)' }}>
                  {f.days_since_modified}
                </td>
                <td style={{ textAlign: 'right', padding: '0.4rem 0.5rem', color: 'rgba(148,163,184,0.35)', fontSize: '0.65rem' }}>
                  {date}
                </td>
              </tr>
            )
          })}
        </tbody>
      </table>
    </div>
  )
}
