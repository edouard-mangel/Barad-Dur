import { useEffect, useState } from 'react'
import { useNavigate } from 'react-router'
import type { AnalysisReport } from '../types'
import { applyScoreThresholds, isAnalysisReport } from '../types'
import ScoreGauge from '../components/ScoreGauge'
import RadarChart from '../components/RadarChart'
import CategoryCard from '../components/CategoryCard'
import TopActions from '../components/TopActions'
import RemoteMetaCard from '../components/RemoteMetaCard'
import HotspotsView from '../components/HotspotsView'
import CouplingView from '../components/CouplingView'
import OwnershipView from '../components/OwnershipView'
import AgeView from '../components/AgeView'

const STORAGE_KEY = 'barad-dur-report'

type Tab = 'overview' | 'hotspots' | 'coupling' | 'ownership' | 'age'

const TABS: [Tab, string][] = [
  ['overview', 'Overview'],
  ['hotspots', 'Hotspots'],
  ['coupling', 'Coupling'],
  ['ownership', 'Ownership'],
  ['age', 'Age'],
]

export default function Report() {
  const navigate = useNavigate()
  const [report, setReport] = useState<AnalysisReport | null>(null)
  const [activeTab, setActiveTab] = useState<Tab>('overview')

  useEffect(() => {
    const raw = sessionStorage.getItem(STORAGE_KEY)
    if (!raw) {
      void navigate('/')
      return
    }
    try {
      const data = JSON.parse(raw) as unknown
      if (isAnalysisReport(data)) {
        applyScoreThresholds(data.score_thresholds)
        setReport(data)
      } else {
        void navigate('/')
      }
    } catch {
      void navigate('/')
    }
  }, [navigate])

  if (!report) return null

  return (
    <div style={{ minHeight: '100vh', padding: '2rem', maxWidth: 1200, margin: '0 auto' }}>
      {/* Header */}
      <header style={{ marginBottom: '2rem' }}>
        <div style={{ display: 'flex', alignItems: 'flex-start', justifyContent: 'space-between', flexWrap: 'wrap', gap: '1rem' }}>
          <div>
            <div style={{ display: 'flex', alignItems: 'center', gap: '0.75rem', marginBottom: '0.25rem' }}>
              <button
                onClick={() => void navigate('/')}
                style={{
                  background: 'none',
                  border: '1px solid rgba(255,255,255,0.08)',
                  borderRadius: 6,
                  padding: '3px 10px',
                  color: 'rgba(148, 163, 184, 0.5)',
                  fontFamily: 'JetBrains Mono, monospace',
                  fontSize: '0.65rem',
                  cursor: 'pointer',
                  letterSpacing: '0.05em',
                }}
              >
                ← back
              </button>
            </div>

            <h1
              style={{
                fontFamily: 'Syne, sans-serif',
                fontWeight: 800,
                fontSize: 'clamp(1.5rem, 4vw, 2.5rem)',
                color: '#f1f5f9',
                letterSpacing: '-0.02em',
                lineHeight: 1,
                margin: 0,
              }}
            >
              {report.repo_name}
            </h1>

            <div style={{ display: 'flex', flexWrap: 'wrap', gap: '0.5rem', marginTop: '0.75rem' }}>
              {[
                { label: 'branch', value: report.branch },
                { label: 'window', value: `${report.time_window_months}mo` },
                { label: 'commits', value: report.total_commits },
                { label: 'authors', value: report.total_authors },
                { label: 'files', value: report.total_files },
              ].map(({ label, value }) => (
                <span
                  key={label}
                  style={{
                    fontFamily: 'JetBrains Mono, monospace',
                    fontSize: '0.7rem',
                    color: 'rgba(148, 163, 184, 0.6)',
                    border: '1px solid rgba(255,255,255,0.07)',
                    borderRadius: 999,
                    padding: '2px 10px',
                    backgroundColor: 'rgba(255,255,255,0.02)',
                  }}
                >
                  <span style={{ color: 'rgba(148, 163, 184, 0.4)' }}>{label} </span>
                  {value}
                </span>
              ))}
            </div>
          </div>

          <div
            style={{
              fontFamily: 'Syne, sans-serif',
              fontWeight: 700,
              fontSize: '0.8rem',
              letterSpacing: '0.15em',
              color: 'rgba(245, 158, 11, 0.6)',
              textTransform: 'uppercase',
              display: 'flex',
              alignItems: 'center',
              gap: '0.4rem',
            }}
          >
            <div
              style={{
                width: 8,
                height: 8,
                borderRadius: '50%',
                backgroundColor: '#f59e0b',
                boxShadow: '0 0 8px rgba(245, 158, 11, 0.8)',
              }}
            />
            Barad-dûr
          </div>
        </div>

        {report.remote_meta && (
          <div style={{ marginTop: '1rem' }}>
            <RemoteMetaCard meta={report.remote_meta} />
          </div>
        )}
      </header>

      {/* Divider */}
      <div style={{ height: 1, backgroundColor: 'rgba(255,255,255,0.05)', marginBottom: '0' }} />

      {/* Tab bar */}
      <div
        style={{
          display: 'flex',
          gap: '0.25rem',
          borderBottom: '1px solid rgba(255,255,255,0.06)',
          marginBottom: '1.5rem',
        }}
      >
        {TABS.map(([tab, label]) => (
          <button
            key={tab}
            onClick={() => setActiveTab(tab)}
            style={{
              background: 'none',
              border: 'none',
              borderBottom: activeTab === tab ? '2px solid #f59e0b' : '2px solid transparent',
              padding: '0.6rem 1rem',
              cursor: 'pointer',
              fontFamily: 'Syne, sans-serif',
              fontWeight: activeTab === tab ? 700 : 400,
              fontSize: '0.82rem',
              color: activeTab === tab ? '#f59e0b' : 'rgba(148, 163, 184, 0.5)',
              transition: 'all 0.15s ease',
              marginBottom: '-1px',
              letterSpacing: '0.04em',
              whiteSpace: 'nowrap',
            }}
          >
            {label}
          </button>
        ))}
      </div>

      {/* Overview tab */}
      {activeTab === 'overview' && (
        <>
          <div
            style={{
              display: 'grid',
              gridTemplateColumns: 'clamp(200px, 25%, 280px) 1fr',
              gap: '2rem',
              marginBottom: '2rem',
              alignItems: 'start',
            }}
          >
            <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: '2rem' }}>
              <ScoreGauge score={report.overall_score} size={200} label="Overall Score" />
              <RadarChart categories={report.categories} size={240} />
            </div>
            <div style={{ display: 'flex', flexDirection: 'column', gap: '0.75rem' }}>
              {report.categories.map((cat, i) => (
                <CategoryCard key={i} category={cat} />
              ))}
            </div>
          </div>
          {report.top_actions.length > 0 && <TopActions actions={report.top_actions} />}
        </>
      )}

      {activeTab === 'hotspots'  && <HotspotsView  files={report.file_hotspots} />}
      {activeTab === 'coupling'  && <CouplingView  pairs={report.coupling_pairs} />}
      {activeTab === 'ownership' && <OwnershipView ownership={report.author_ownership} />}
      {activeTab === 'age'       && <AgeView       ages={report.file_ages} />}

      {/* Footer */}
      <footer
        style={{
          marginTop: '3rem',
          paddingTop: '1rem',
          borderTop: '1px solid rgba(255,255,255,0.04)',
          textAlign: 'center',
          fontFamily: 'JetBrains Mono, monospace',
          fontSize: '0.65rem',
          color: 'rgba(148, 163, 184, 0.25)',
          letterSpacing: '0.08em',
        }}
      >
        barad-dûr repository intelligence
      </footer>
    </div>
  )
}
