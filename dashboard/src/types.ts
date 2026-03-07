export type RawValue = number | string | string[]

export interface MetricValue {
  name: string
  description: string
  raw_value: RawValue
  score: number
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
  loc: number
  total_lines: number
  cyclomatic_complexity: number
  public_methods: number
  properties: number
  hotspot_score: number
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

export interface AnalysisReport {
  repo_name: string
  branch: string
  time_window_months: number
  total_commits: number
  total_authors: number
  total_files: number
  overall_score: number
  categories: CategoryResult[]
  top_actions: string[]
  remote_meta: RemoteMeta | null
  file_hotspots: HotspotFile[]
  coupling_pairs: CouplingPair[]
  author_ownership: FileOwnership[]
  file_ages: FileAge[]
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

export function scoreColor(score: number): string {
  if (score > 70) return '#10b981'
  if (score > 40) return '#f59e0b'
  return '#ef4444'
}

export function scoreClass(score: number): string {
  if (score > 70) return 'score-green'
  if (score > 40) return 'score-yellow'
  return 'score-red'
}

export function scoreBgClass(score: number): string {
  if (score > 70) return 'bg-score-green'
  if (score > 40) return 'bg-score-yellow'
  return 'bg-score-red'
}

export function formatRawValue(raw: RawValue): string {
  if (Array.isArray(raw)) return raw.join(', ') || '—'
  return String(raw)
}
