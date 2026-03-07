import { useRef, useEffect, useState } from 'react'
import * as d3 from 'd3'
import type { HotspotFile } from '../types'

interface Props {
  files: HotspotFile[]
}

type SortKey = 'hotspot_score' | 'churn_count' | 'cyclomatic_complexity' | 'loc'

function colStyle(active: boolean): React.CSSProperties {
  return {
    cursor: 'pointer',
    color: active ? '#f59e0b' : 'rgba(148,163,184,0.5)',
    userSelect: 'none',
  }
}

export default function HotspotsView({ files }: Props) {
  const svgRef = useRef<SVGSVGElement>(null)
  const [sort, setSort] = useState<SortKey>('hotspot_score')

  const sorted = [...files].sort((a, b) => b[sort] - a[sort]).slice(0, 50)

  useEffect(() => {
    if (!svgRef.current || files.length === 0) return
    const svg = d3.select(svgRef.current)
    svg.selectAll('*').remove()

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

    // Grid lines
    g.append('g')
      .attr('transform', `translate(0,${h})`)
      .call(d3.axisBottom(xScale).ticks(5).tickSize(-h))
      .call(ax => ax.select('.domain').remove())
      .call(ax => ax.selectAll('.tick line').attr('stroke', 'rgba(255,255,255,0.04)'))
      .call(ax => ax.selectAll('.tick text')
        .attr('fill', 'rgba(148,163,184,0.5)')
        .attr('font-size', '9')
        .attr('font-family', 'JetBrains Mono'))

    g.append('g')
      .call(d3.axisLeft(yScale).ticks(4).tickSize(-w))
      .call(ax => ax.select('.domain').remove())
      .call(ax => ax.selectAll('.tick line').attr('stroke', 'rgba(255,255,255,0.04)'))
      .call(ax => ax.selectAll('.tick text')
        .attr('fill', 'rgba(148,163,184,0.5)')
        .attr('font-size', '9')
        .attr('font-family', 'JetBrains Mono'))

    // Axis labels
    g.append('text')
      .attr('x', w / 2).attr('y', h + 34)
      .attr('text-anchor', 'middle')
      .attr('fill', 'rgba(148,163,184,0.35)')
      .attr('font-size', '9')
      .attr('font-family', 'Syne')
      .text('Cyclomatic complexity →')

    g.append('text')
      .attr('transform', 'rotate(-90)')
      .attr('x', -h / 2).attr('y', -38)
      .attr('text-anchor', 'middle')
      .attr('fill', 'rgba(148,163,184,0.35)')
      .attr('font-size', '9')
      .attr('font-family', 'Syne')
      .text('Churn count →')

    // Bubbles
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
        `${f.path}\nscore: ${f.hotspot_score.toFixed(0)}\nchurn: ${f.churn_count}\ncc: ${f.cyclomatic_complexity}\nloc: ${f.loc}`
      )
  }, [files])

  if (files.length === 0) {
    return (
      <p style={{ fontFamily: 'JetBrains Mono', fontSize: '0.8rem', color: 'rgba(148,163,184,0.4)' }}>
        No file data available.
      </p>
    )
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '1.5rem' }}>
      {/* Scatter plot */}
      <div style={{ border: '1px solid rgba(255,255,255,0.06)', borderRadius: 10, padding: '1rem 1.25rem', backgroundColor: 'rgba(255,255,255,0.02)', overflowX: 'auto' }}>
        <p style={{ fontFamily: 'Syne', fontSize: '0.7rem', color: 'rgba(148,163,184,0.4)', letterSpacing: '0.1em', textTransform: 'uppercase', marginBottom: '0.75rem', margin: '0 0 0.75rem' }}>
          Hotspot quadrant — bubble size = LOC · color = risk
        </p>
        <svg ref={svgRef} width={560} height={260} style={{ display: 'block' }} />
      </div>

      {/* Table */}
      <div style={{ border: '1px solid rgba(255,255,255,0.06)', borderRadius: 10, padding: '1rem 1.25rem', backgroundColor: 'rgba(255,255,255,0.02)', overflowX: 'auto' }}>
        <table style={{ width: '100%', borderCollapse: 'collapse', fontFamily: 'JetBrains Mono', fontSize: '0.72rem' }}>
          <thead>
            <tr style={{ borderBottom: '1px solid rgba(255,255,255,0.08)', color: 'rgba(148,163,184,0.5)', fontSize: '0.65rem', letterSpacing: '0.08em', textTransform: 'uppercase' }}>
              <th style={{ textAlign: 'left', padding: '0.4rem 0.5rem', fontWeight: 400 }}>File</th>
              <th style={{ textAlign: 'right', padding: '0.4rem 0.5rem', fontWeight: 400, ...colStyle(sort === 'hotspot_score') }} onClick={() => setSort('hotspot_score')}>Score ↕</th>
              <th style={{ textAlign: 'right', padding: '0.4rem 0.5rem', fontWeight: 400, ...colStyle(sort === 'churn_count') }} onClick={() => setSort('churn_count')}>Churn ↕</th>
              <th style={{ textAlign: 'right', padding: '0.4rem 0.5rem', fontWeight: 400, ...colStyle(sort === 'cyclomatic_complexity') }} onClick={() => setSort('cyclomatic_complexity')}>CC ↕</th>
              <th style={{ textAlign: 'right', padding: '0.4rem 0.5rem', fontWeight: 400, ...colStyle(sort === 'loc') }} onClick={() => setSort('loc')}>LOC ↕</th>
              <th style={{ textAlign: 'right', padding: '0.4rem 0.5rem', fontWeight: 400, color: 'rgba(148,163,184,0.5)' }}>Methods</th>
              <th style={{ textAlign: 'right', padding: '0.4rem 0.5rem', fontWeight: 400, color: 'rgba(148,163,184,0.5)' }}>Props</th>
            </tr>
          </thead>
          <tbody>
            {sorted.map((f, i) => {
              const score = f.hotspot_score
              const color = score > 70 ? '#ef4444' : score > 40 ? '#f59e0b' : '#10b981'
              const dir = f.path.includes('/') ? f.path.substring(0, f.path.lastIndexOf('/') + 1) : ''
              const name = f.path.split('/').pop() ?? f.path
              return (
                <tr key={i} style={{ borderBottom: '1px solid rgba(255,255,255,0.03)' }}>
                  <td style={{ padding: '0.4rem 0.5rem', maxWidth: 300, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }} title={f.path}>
                    <span style={{ color: 'rgba(226,232,240,0.85)' }}>{name}</span>
                    {dir && <span style={{ color: 'rgba(148,163,184,0.3)', marginLeft: '0.3rem', fontSize: '0.62rem' }}>{dir}</span>}
                  </td>
                  <td style={{ textAlign: 'right', padding: '0.4rem 0.5rem', color, fontWeight: 600 }}>{score.toFixed(0)}</td>
                  <td style={{ textAlign: 'right', padding: '0.4rem 0.5rem', color: 'rgba(226,232,240,0.7)' }}>{f.churn_count}</td>
                  <td style={{ textAlign: 'right', padding: '0.4rem 0.5rem', color: 'rgba(226,232,240,0.7)' }}>{f.cyclomatic_complexity}</td>
                  <td style={{ textAlign: 'right', padding: '0.4rem 0.5rem', color: 'rgba(226,232,240,0.7)' }}>{f.loc}</td>
                  <td style={{ textAlign: 'right', padding: '0.4rem 0.5rem', color: 'rgba(148,163,184,0.45)' }}>{f.public_methods}</td>
                  <td style={{ textAlign: 'right', padding: '0.4rem 0.5rem', color: 'rgba(148,163,184,0.45)' }}>{f.properties}</td>
                </tr>
              )
            })}
          </tbody>
        </table>
      </div>
    </div>
  )
}
