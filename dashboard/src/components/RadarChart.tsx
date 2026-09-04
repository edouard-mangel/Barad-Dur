import { useEffect, useRef } from 'react'
import * as d3 from 'd3'
import type { CategoryResult } from '../types'
import { scoreColor } from '../types'

interface Props {
  categories: CategoryResult[]
  size?: number
}

export default function RadarChart({ categories, size = 240 }: Props) {
  const svgRef = useRef<SVGSVGElement>(null)

  useEffect(() => {
    if (!svgRef.current || categories.length === 0) return

    const svg = d3.select(svgRef.current)
    svg.selectAll('*').remove()

    const margin = 48
    const cx = size / 2
    const cy = size / 2
    const radius = (size / 2) - margin
    const n = categories.length
    const levels = 4

    const angleSlice = (Math.PI * 2) / n
    const scale = d3.scaleLinear().domain([0, 100]).range([0, radius])

    const g = svg.append('g').attr('transform', `translate(${cx},${cy})`)

    // Background rings
    for (let i = 1; i <= levels; i++) {
      const r = (radius / levels) * i
      const points = Array.from({ length: n }, (_, j) => {
        const angle = angleSlice * j - Math.PI / 2
        return [r * Math.cos(angle), r * Math.sin(angle)] as [number, number]
      })
      g.append('polygon')
        .attr('points', points.map(p => p.join(',')).join(' '))
        .attr('fill', 'none')
        .attr('stroke', 'rgba(255,255,255,0.06)')
        .attr('stroke-width', 1)
    }

    // Axis lines
    categories.forEach((_, i) => {
      const angle = angleSlice * i - Math.PI / 2
      g.append('line')
        .attr('x1', 0).attr('y1', 0)
        .attr('x2', radius * Math.cos(angle))
        .attr('y2', radius * Math.sin(angle))
        .attr('stroke', 'rgba(255,255,255,0.08)')
        .attr('stroke-width', 1)
    })

    // Data polygon
    // An unscored category sits at the centre: no claim in either direction.
    const dataPoints = categories.map((cat, i) => {
      const angle = angleSlice * i - Math.PI / 2
      const r = scale(cat.score ?? 0)
      return [r * Math.cos(angle), r * Math.sin(angle)] as [number, number]
    })

    g.append('polygon')
      .attr('points', dataPoints.map(p => p.join(',')).join(' '))
      .attr('fill', 'rgba(245, 158, 11, 0.08)')
      .attr('stroke', 'rgba(245, 158, 11, 0.6)')
      .attr('stroke-width', 1.5)
      .attr('stroke-linejoin', 'round')

    // Data points
    categories.forEach((cat, i) => {
      const angle = angleSlice * i - Math.PI / 2
      const r = scale(cat.score ?? 0)
      const color = cat.score === null ? 'rgba(148, 163, 184, 0.8)' : scoreColor(cat.score)
      g.append('circle')
        .attr('cx', r * Math.cos(angle))
        .attr('cy', r * Math.sin(angle))
        .attr('r', 4)
        .attr('fill', color)
        .attr('stroke', '#080a0f')
        .attr('stroke-width', 2)
        .style('filter', `drop-shadow(0 0 4px ${color})`)
    })

    // Labels
    const labelRadius = radius + 28
    categories.forEach((cat, i) => {
      const angle = angleSlice * i - Math.PI / 2
      const x = labelRadius * Math.cos(angle)
      const y = labelRadius * Math.sin(angle)

      g.append('text')
        .attr('x', x)
        .attr('y', y)
        .attr('text-anchor', 'middle')
        .attr('dominant-baseline', 'middle')
        .attr('fill', 'rgba(226, 232, 240, 0.7)')
        .attr('font-size', '10')
        .attr('font-family', 'Syne, sans-serif')
        .attr('font-weight', '600')
        .text(cat.name)

      g.append('text')
        .attr('x', x)
        .attr('y', y + 13)
        .attr('text-anchor', 'middle')
        .attr('dominant-baseline', 'middle')
        .attr('fill', cat.score === null ? 'rgba(148, 163, 184, 0.8)' : scoreColor(cat.score))
        .attr('font-size', '9')
        .attr('font-family', 'JetBrains Mono, monospace')
        .attr('font-weight', '500')
        .text(cat.score === null ? '—' : cat.score)
    })
  }, [categories, size])

  return (
    <svg
      ref={svgRef}
      width={size}
      height={size}
      style={{ overflow: 'visible' }}
    />
  )
}
