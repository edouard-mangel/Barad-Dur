import type { RawValue, ScoreThresholds } from './model'

type Band = 'good' | 'warn' | 'danger'

const BAND_COLORS: Record<Band, string> = {
  good: '#10b981',
  warn: '#f59e0b',
  danger: '#ef4444',
}

const BAND_CLASSES: Record<Band, string> = {
  good: 'score-green',
  warn: 'score-yellow',
  danger: 'score-red',
}

export function scoreBand(score: number, thresholds: ScoreThresholds): Band {
  if (score >= thresholds.good_min) return 'good'
  if (score >= thresholds.warn_min) return 'warn'
  return 'danger'
}

export const scoreColor = (score: number, thresholds: ScoreThresholds): string =>
  BAND_COLORS[scoreBand(score, thresholds)]

export const scoreClass = (score: number, thresholds: ScoreThresholds): string =>
  BAND_CLASSES[scoreBand(score, thresholds)]

export const scoreBgClass = (score: number, thresholds: ScoreThresholds): string =>
  `bg-${scoreClass(score, thresholds)}`

export function formatRawValue(raw: RawValue): string {
  if ('Integer' in raw) return String(raw.Integer)
  if ('Float' in raw) return raw.Float.toFixed(2)
  if ('Percentage' in raw) return `${raw.Percentage.toFixed(0)}%`
  if ('Count' in raw) return String(raw.Count)
  if ('Text' in raw) return raw.Text
  return raw.List.join(', ') || '—'
}
