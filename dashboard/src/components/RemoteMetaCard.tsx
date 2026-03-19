import type { RemoteMeta } from '../types'

interface Props {
  meta: RemoteMeta
}

function StatPill({ label, value }: { label: string; value: string | number }) {
  return (
    <div
      style={{
        display: 'flex',
        alignItems: 'center',
        gap: '0.4rem',
        padding: '0.3rem 0.75rem',
        borderRadius: 999,
        border: '1px solid rgba(255,255,255,0.08)',
        backgroundColor: 'rgba(255,255,255,0.03)',
      }}
    >
      <span style={{ fontFamily: 'JetBrains Mono, monospace', fontSize: '0.72rem', color: 'rgba(148, 163, 184, 0.5)' }}>
        {label}
      </span>
      <span style={{ fontFamily: 'JetBrains Mono, monospace', fontSize: '0.8rem', fontWeight: 600, color: 'rgba(226, 232, 240, 0.85)' }}>
        {value}
      </span>
    </div>
  )
}

export default function RemoteMetaCard({ meta }: Props) {
  return (
    <div
      style={{
        display: 'flex',
        flexWrap: 'wrap',
        alignItems: 'center',
        gap: '0.5rem',
        padding: '0.75rem 1rem',
        borderRadius: 8,
        border: '1px solid rgba(255,255,255,0.06)',
        backgroundColor: 'rgba(255,255,255,0.02)',
      }}
    >
      {meta.description && (
        <span
          style={{
            fontFamily: 'Syne, sans-serif',
            fontSize: '0.8rem',
            color: 'rgba(148, 163, 184, 0.6)',
            flex: '1 1 100%',
            marginBottom: '0.25rem',
          }}
        >
          {meta.description}
        </span>
      )}

      {meta.language && <StatPill label="lang" value={meta.language} />}
      {meta.stars !== null && <StatPill label="⭐" value={meta.stars.toLocaleString()} />}
      {meta.open_issues !== null && <StatPill label="issues" value={meta.open_issues} />}

      <a
        href={meta.url}
        target="_blank"
        rel="noopener noreferrer"
        style={{
          fontFamily: 'JetBrains Mono, monospace',
          fontSize: '0.7rem',
          color: 'rgba(245, 158, 11, 0.6)',
          textDecoration: 'none',
          marginLeft: 'auto',
          borderBottom: '1px solid rgba(245, 158, 11, 0.2)',
          paddingBottom: 1,
        }}
      >
        {meta.url.replace(/^https?:\/\//, '')}
      </a>
    </div>
  )
}
