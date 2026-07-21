export type RawValue = number | string | string[]

export interface MetricValue {
  name: string
  description: string
  raw_value: RawValue
  /** 0–100, or null when the repo lacks the data to judge this metric. */
  score: number | null
}

export interface CategoryResult {
  name: string
  score: number
  metrics: MetricValue[]
}

export interface RemoteMeta {
  url: string
  stars: number | null
  description: string | null
  language: string | null
  open_issues: number | null
}

export interface HotspotFile {
  path: string
  churn_count: number
  bug_commit_count: number
  loc: number
  total_lines: number
  cyclomatic_complexity: number
  public_methods: number
  properties: number
  hotspot_score: number
  // Per-kind Pressman coupling finding counts. Optional: reports generated
  // before M4 don't carry them.
  content_findings?: number
  common_findings?: number
  control_findings?: number
  inheritance_findings?: number
  // What kind of file this is. Optional: reports generated before the
  // role-classification release don't carry it — treat missing as 'source'.
  role?: 'source' | 'test' | 'config' | 'docs' | 'other'
}

export interface CouplingPair {
  file_a: string
  file_b: string
  co_changes: number
  coupling_pct: number
}

export interface AuthorShare {
  name: string
  pct: number
}

export interface FileOwnership {
  path: string
  authors: AuthorShare[]
}

export interface FileAge {
  path: string
  last_modified: string
  days_since_modified: number
}

export interface ScoreThresholds {
  good_min: number
  warn_min: number
}

export interface ActionItem {
  text: string
  target_tab?: string
  sort_by?: string
}

export interface AnalysisReport {
  repo_name: string
  branch: string
  time_window_months: number
  total_commits: number
  total_authors: number
  total_files: number
  overall_score: number
  categories: CategoryResult[]
  // ActionItem since the scorer started serializing structured actions;
  // bare strings in reports generated before that.
  top_actions: (ActionItem | string)[]
  coupling_actions?: ActionItem[]
  remote_meta: RemoteMeta | null
  file_hotspots: HotspotFile[]
  coupling_pairs: CouplingPair[]
  author_ownership: FileOwnership[]
  file_ages: FileAge[]
  score_thresholds?: ScoreThresholds
}

export function isAnalysisReport(obj: unknown): obj is AnalysisReport {
  if (typeof obj !== 'object' || obj === null) return false
  const r = obj as Record<string, unknown>
  return (
    typeof r['repo_name'] === 'string' &&
    typeof r['branch'] === 'string' &&
    typeof r['overall_score'] === 'number' &&
    Array.isArray(r['categories']) &&
    Array.isArray(r['top_actions']) &&
    Array.isArray(r['file_hotspots']) &&
    Array.isArray(r['coupling_pairs'])
  )
}

// Band thresholds are defined once in the Rust scorer (scorer/types.rs) and
// shipped inside every report as `score_thresholds`. The defaults below only
// cover reports generated before that field existed.
let bands: ScoreThresholds = { good_min: 71, warn_min: 41 }

export function applyScoreThresholds(t: ScoreThresholds | undefined): void {
  if (t && typeof t.good_min === 'number' && typeof t.warn_min === 'number') {
    bands = t
  }
}

type Band = 'good' | 'warn' | 'danger'

function scoreBand(score: number): Band {
  if (score >= bands.good_min) return 'good'
  if (score >= bands.warn_min) return 'warn'
  return 'danger'
}

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

export function scoreColor(score: number): string {
  return BAND_COLORS[scoreBand(score)]
}

export function scoreClass(score: number): string {
  return BAND_CLASSES[scoreBand(score)]
}

export function scoreBgClass(score: number): string {
  return `bg-${BAND_CLASSES[scoreBand(score)]}`
}

export function formatRawValue(raw: RawValue): string {
  if (Array.isArray(raw)) return raw.join(', ') || '—'
  return String(raw)
}
