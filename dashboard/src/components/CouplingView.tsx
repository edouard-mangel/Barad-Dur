import type { CouplingPair } from '../types'

interface Props {
  pairs: CouplingPair[]
}

export default function CouplingView({ pairs }: Props) {
  const sorted = [...pairs].sort((a, b) => b.coupling_pct - a.coupling_pct)

  if (sorted.length === 0) {
    return (
      <p style={{ fontFamily: 'JetBrains Mono', fontSize: '0.8rem', color: 'rgba(148,163,184,0.4)' }}>
        No coupling pairs detected (threshold: 3 co-changes).
      </p>
    )
  }

  return (
    <div style={{ border: '1px solid rgba(255,255,255,0.06)', borderRadius: 10, padding: '1rem 1.25rem', backgroundColor: 'rgba(255,255,255,0.02)', overflowX: 'auto' }}>
      <p style={{ fontFamily: 'Syne', fontSize: '0.7rem', color: 'rgba(148,163,184,0.4)', letterSpacing: '0.1em', textTransform: 'uppercase', margin: '0 0 0.75rem' }}>
        Temporal coupling — files that change together
      </p>
      <table style={{ width: '100%', borderCollapse: 'collapse', fontFamily: 'JetBrains Mono', fontSize: '0.72rem' }}>
        <thead>
          <tr style={{ borderBottom: '1px solid rgba(255,255,255,0.08)', color: 'rgba(148,163,184,0.5)', fontSize: '0.65rem', letterSpacing: '0.08em', textTransform: 'uppercase' }}>
            <th style={{ textAlign: 'left', padding: '0.4rem 0.5rem', fontWeight: 400 }}>File A</th>
            <th style={{ textAlign: 'left', padding: '0.4rem 0.5rem', fontWeight: 400 }}>File B</th>
            <th style={{ textAlign: 'right', padding: '0.4rem 0.5rem', fontWeight: 400 }}>Co-changes</th>
            <th style={{ textAlign: 'left', padding: '0.4rem 0.5rem', fontWeight: 400, minWidth: 180 }}>Coupling %</th>
          </tr>
        </thead>
        <tbody>
          {sorted.map((p, i) => {
            const pct = p.coupling_pct
            const color = pct > 70 ? '#ef4444' : pct > 40 ? '#f59e0b' : '#10b981'
            return (
              <tr key={i} style={{ borderBottom: '1px solid rgba(255,255,255,0.03)' }}>
                <td
                  style={{ padding: '0.4rem 0.5rem', color: 'rgba(226,232,240,0.8)', maxWidth: 220, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}
                  title={p.file_a}
                >
                  {p.file_a.split('/').pop()}
                  {p.file_a.includes('/') && (
                    <span style={{ color: 'rgba(148,163,184,0.3)', marginLeft: '0.3rem', fontSize: '0.62rem' }}>
                      {p.file_a.substring(0, p.file_a.lastIndexOf('/') + 1)}
                    </span>
                  )}
                </td>
                <td
                  style={{ padding: '0.4rem 0.5rem', color: 'rgba(226,232,240,0.8)', maxWidth: 220, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}
                  title={p.file_b}
                >
                  {p.file_b.split('/').pop()}
                  {p.file_b.includes('/') && (
                    <span style={{ color: 'rgba(148,163,184,0.3)', marginLeft: '0.3rem', fontSize: '0.62rem' }}>
                      {p.file_b.substring(0, p.file_b.lastIndexOf('/') + 1)}
                    </span>
                  )}
                </td>
                <td style={{ textAlign: 'right', padding: '0.4rem 0.5rem', color: 'rgba(148,163,184,0.6)' }}>
                  {p.co_changes}
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
                          boxShadow: `0 0 6px ${color}60`,
                        }}
                      />
                    </div>
                    <span style={{ color, fontWeight: 600, minWidth: '2.5rem', textAlign: 'right' }}>
                      {pct.toFixed(0)}%
                    </span>
                  </div>
                </td>
              </tr>
            )
          })}
        </tbody>
      </table>
    </div>
  )
}
