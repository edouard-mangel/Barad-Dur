import { useRef, useEffect, useState } from 'react'
import * as d3 from 'd3'
import type { HotspotFile } from '../types'

interface Props {
  files: HotspotFile[]
}

export type SortKey = 'hotspot_score' | 'churn_count' | 'cyclomatic_complexity' | 'loc'

/** Files that survive dismissal, ordered by the active column, top 50. */
export function visibleSorted(
  files: HotspotFile[],
  dismissed: Set<string>,
  sort: SortKey
): HotspotFile[] {
  return files
    .filter(f => !dismissed.has(f.path))
    .sort((a, b) => b[sort] - a[sort])
    .slice(0, 50)
}

/** Per-kind coupling badge text (Cn/Cm/Ih/Ct), empty when no findings. */
export function couplingBadge(f: HotspotFile): string {
  const cn = f.content_findings ?? 0
  const cm = f.common_findings ?? 0
  const ih = f.inheritance_findings ?? 0
  const ct = f.control_findings ?? 0
  return [cn > 0 && `Cn ${cn}`, cm > 0 && `Cm ${cm}`, ih > 0 && `Ih ${ih}`, ct > 0 && `Ct ${ct}`]
    .filter(Boolean)
    .join(' · ')
}

/** Red above 70, amber above 40, green otherwise — the score bands. */
export function riskColor(score: number): string {
  return score > 70 ? '#ef4444' : score > 40 ? '#f59e0b' : '#10b981'
}

/** Split a path into its dimmed directory prefix and highlighted name. */
export function splitPath(path: string): { dir: string; name: string } {
  const dir = path.includes('/') ? path.substring(0, path.lastIndexOf('/') + 1) : ''
  const name = path.split('/').pop() ?? path
  return { dir, name }
}

function colStyle(active: boolean): React.CSSProperties {
  return {
    cursor: 'pointer',
    color: active ? '#f59e0b' : 'rgba(148,163,184,0.5)',
    userSelect: 'none',
  }
}

/** d3 churn-vs-complexity bubble chart, drawn into `el`. */
function renderScatter(el: SVGSVGElement, files: HotspotFile[]): void {
  const svg = d3.select(el)
  svg.selectAll('*').remove()
  if (files.length === 0) return

  const W = 560, H = 260
  const M = { top: 20, right: 20, bottom: 40, left: 50 }
  const w = W - M.left - M.right
  const h = H - M.top - M.bottom

  const g = svg.append('g').attr('transform', `translate(${M.left},${M.top})`)

  const xScale = d3.scaleLinear()
    .domain([0, d3.max(files, f => f.cyclomatic_complexity) ?? 1])
    .range([0, w])

  const yScale = d3.scaleLinear()
    .domain([0, d3.max(files, f => f.churn_count) ?? 1])
    .range([h, 0])

  const rScale = d3.scaleSqrt()
    .domain([0, d3.max(files, f => f.loc) ?? 1])
    .range([2, 14])

  const colorScale = d3.scaleLinear<string>()
    .domain([0, 50, 100])
    .range(['#10b981', '#f59e0b', '#ef4444'])

  const styleAxis = (ax: d3.Selection<SVGGElement, unknown, null, undefined>) => {
    ax.select('.domain').remove()
    ax.selectAll('.tick line').attr('stroke', 'rgba(255,255,255,0.04)')
    ax.selectAll('.tick text')
      .attr('fill', 'rgba(148,163,184,0.5)')
      .attr('font-size', '9')
      .attr('font-family', 'JetBrains Mono')
  }

  g.append('g')
    .attr('transform', `translate(0,${h})`)
    .call(d3.axisBottom(xScale).ticks(5).tickSize(-h))
    .call(styleAxis)

  g.append('g')
    .call(d3.axisLeft(yScale).ticks(4).tickSize(-w))
    .call(styleAxis)

  const axisLabel = (x: number, y: number, text: string, rotate = false) => {
    const t = g.append('text')
      .attr('x', x).attr('y', y)
      .attr('text-anchor', 'middle')
      .attr('fill', 'rgba(148,163,184,0.35)')
      .attr('font-size', '9')
      .attr('font-family', 'Syne')
      .text(text)
    if (rotate) t.attr('transform', 'rotate(-90)')
  }
  axisLabel(w / 2, h + 34, 'Cyclomatic complexity →')
  axisLabel(-h / 2, -38, 'Churn count →', true)

  g.selectAll('circle')
    .data(files)
    .join('circle')
    .attr('cx', f => xScale(f.cyclomatic_complexity))
    .attr('cy', f => yScale(f.churn_count))
    .attr('r', f => rScale(f.loc))
    .attr('fill', f => colorScale(f.hotspot_score))
    .attr('fill-opacity', 0.5)
    .attr('stroke', f => colorScale(f.hotspot_score))
    .attr('stroke-width', 1)
    .attr('stroke-opacity', 0.8)
    .append('title')
    .text(f =>
      `${f.path}\nscore: ${f.hotspot_score.toFixed(0)}\nchurn: ${f.churn_count}\nbug commits: ${f.bug_commit_count}\ncc: ${f.cyclomatic_complexity}\nloc: ${f.loc}`
    )
}

const numCell: React.CSSProperties = { textAlign: 'right', padding: '0.4rem 0.5rem' }

function HotspotRow({ f, onDismiss }: { f: HotspotFile; onDismiss: (path: string) => void }) {
  const score = f.hotspot_score
  const color = riskColor(score)
  const { dir, name } = splitPath(f.path)
  const badge = couplingBadge(f)
  const cn = f.content_findings ?? 0
  const cm = f.common_findings ?? 0
  return (
    <tr style={{ borderBottom: '1px solid rgba(255,255,255,0.03)' }}>
      <td style={{ padding: '0.4rem 0.5rem', maxWidth: 300, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }} title={f.path}>
        <span style={{ color: 'rgba(226,232,240,0.85)' }}>{name}</span>
        {dir && <span style={{ color: 'rgba(148,163,184,0.3)', marginLeft: '0.3rem', fontSize: '0.62rem' }}>{dir}</span>}
      </td>
      <td style={{ ...numCell, color, fontWeight: 600 }}>{score.toFixed(0)}</td>
      <td style={{ ...numCell, color: 'rgba(226,232,240,0.7)' }}>{f.churn_count}</td>
      <td style={{ ...numCell, color: 'rgba(226,232,240,0.7)' }}>{f.cyclomatic_complexity}</td>
      <td style={{ ...numCell, color: 'rgba(226,232,240,0.7)' }}>{f.loc}</td>
      <td style={{ ...numCell, color: f.bug_commit_count > 0 ? '#f87171' : 'rgba(148,163,184,0.45)' }}>{f.bug_commit_count > 0 ? f.bug_commit_count : '—'}</td>
      <td style={{ ...numCell, color: 'rgba(148,163,184,0.45)' }}>{f.public_methods}</td>
      <td style={{ ...numCell, color: 'rgba(148,163,184,0.45)' }}>{f.properties}</td>
      <td style={{ ...numCell, color: cn + cm > 0 ? '#f87171' : 'rgba(148,163,184,0.45)', fontWeight: cn + cm > 0 ? 600 : 400 }}>{badge || '—'}</td>
      <td style={numCell}>
        <button
          aria-label={`Dismiss ${f.path}`}
          title="Dismiss this file from the list"
          onClick={() => onDismiss(f.path)}
          style={{ background: 'none', border: 'none', color: 'rgba(148,163,184,0.6)', cursor: 'pointer', fontSize: '0.95rem', lineHeight: 1, padding: '0 0.25rem' }}
        >
          ×
        </button>
      </td>
    </tr>
  )
}

const SORTABLE_COLUMNS: { key: SortKey; label: string }[] = [
  { key: 'hotspot_score', label: 'Score' },
  { key: 'churn_count', label: 'Churn' },
  { key: 'cyclomatic_complexity', label: 'CC' },
  { key: 'loc', label: 'LOC' },
]

export default function HotspotsView({ files }: Props) {
  const svgRef = useRef<SVGSVGElement>(null)
  const [sort, setSort] = useState<SortKey>('hotspot_score')
  // Client-side, ephemeral dismissal — mirrors the coupling tab. Keyed by path so
  // sorting stays correct.
  const [dismissed, setDismissed] = useState<Set<string>>(new Set())

  const visible = files.filter(f => !dismissed.has(f.path))
  const sorted = visibleSorted(files, dismissed, sort)

  useEffect(() => {
    if (svgRef.current) renderScatter(svgRef.current, visible)
  }, [files, dismissed])

  if (files.length === 0) {
    return (
      <p style={{ fontFamily: 'JetBrains Mono', fontSize: '0.8rem', color: 'rgba(148,163,184,0.4)' }}>
        No file data available.
      </p>
    )
  }

  const dismiss = (path: string) => setDismissed(prev => new Set(prev).add(path))

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '1.5rem' }}>
      {/* Scatter plot */}
      <div style={{ border: '1px solid rgba(255,255,255,0.06)', borderRadius: 10, padding: '1rem 1.25rem', backgroundColor: 'rgba(255,255,255,0.02)', overflowX: 'auto' }}>
        <p style={{ fontFamily: 'Syne', fontSize: '0.7rem', color: 'rgba(148,163,184,0.4)', letterSpacing: '0.1em', textTransform: 'uppercase', marginBottom: '0.75rem', margin: '0 0 0.75rem' }}>
          Hotspot quadrant — bubble size = LOC · color = risk
        </p>
        <svg ref={svgRef} width={560} height={260} style={{ display: 'block' }} />
      </div>

      {/* Dismissed-rows controls — ephemeral, mirrors the coupling tab */}
      {dismissed.size > 0 && (
        <div style={{ display: 'flex', alignItems: 'center', gap: '0.75rem', fontFamily: 'JetBrains Mono', fontSize: '0.7rem', color: 'rgba(148,163,184,0.6)' }}>
          <span>{dismissed.size} dismissed</span>
          <button
            onClick={() => setDismissed(new Set())}
            style={{ background: 'rgba(255,255,255,0.04)', border: '1px solid rgba(255,255,255,0.1)', borderRadius: 6, padding: '0.25rem 0.6rem', color: 'inherit', cursor: 'pointer', fontFamily: 'inherit', fontSize: 'inherit' }}
          >
            Reset dismissed
          </button>
        </div>
      )}

      {/* Table */}
      <div style={{ border: '1px solid rgba(255,255,255,0.06)', borderRadius: 10, padding: '1rem 1.25rem', backgroundColor: 'rgba(255,255,255,0.02)', overflowX: 'auto' }}>
        <table style={{ width: '100%', borderCollapse: 'collapse', fontFamily: 'JetBrains Mono', fontSize: '0.72rem' }}>
          <thead>
            <tr style={{ borderBottom: '1px solid rgba(255,255,255,0.08)', color: 'rgba(148,163,184,0.5)', fontSize: '0.65rem', letterSpacing: '0.08em', textTransform: 'uppercase' }}>
              <th style={{ textAlign: 'left', padding: '0.4rem 0.5rem', fontWeight: 400 }}>File</th>
              {SORTABLE_COLUMNS.map(col => (
                <th
                  key={col.key}
                  style={{ textAlign: 'right', padding: '0.4rem 0.5rem', fontWeight: 400, ...colStyle(sort === col.key) }}
                  onClick={() => setSort(col.key)}
                >
                  {col.label} ↕
                </th>
              ))}
              <th style={{ textAlign: 'right', padding: '0.4rem 0.5rem', fontWeight: 400, color: 'rgba(148,163,184,0.5)' }}>Bugs</th>
              <th style={{ textAlign: 'right', padding: '0.4rem 0.5rem', fontWeight: 400, color: 'rgba(148,163,184,0.5)' }}>Methods</th>
              <th style={{ textAlign: 'right', padding: '0.4rem 0.5rem', fontWeight: 400, color: 'rgba(148,163,184,0.5)' }}>Props</th>
              <th style={{ textAlign: 'right', padding: '0.4rem 0.5rem', fontWeight: 400, color: 'rgba(148,163,184,0.5)' }}>Coupling</th>
              <th style={{ width: 28 }} aria-label="Dismiss" />
            </tr>
          </thead>
          <tbody>
            {sorted.map(f => (
              <HotspotRow key={f.path} f={f} onDismiss={dismiss} />
            ))}
          </tbody>
        </table>
      </div>
    </div>
  )
}
