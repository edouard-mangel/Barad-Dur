import type { ActionItem as WireActionItem } from './generated/ActionItem'
import type { AnalysisReport as WireAnalysisReport } from './generated/AnalysisReport'
import type { AuthorShare as WireAuthorShare } from './generated/AuthorShare'
import type { CategoryResult as WireCategoryResult } from './generated/CategoryResult'
import type { CouplingPair as WireCouplingPair } from './generated/CouplingPair'
import type { FileAge as WireFileAge } from './generated/FileAge'
import type { FileOwnership as WireFileOwnership } from './generated/FileOwnership'
import type { HotspotFile as WireHotspotFile } from './generated/HotspotFile'
import type { MetricValue as WireMetricValue } from './generated/MetricValue'
import type { RawValue as WireRawValue } from './generated/RawValue'
import type { RemoteMeta as WireRemoteMeta } from './generated/RemoteMeta'
import type { ScoreThresholds as WireScoreThresholds } from './generated/ScoreThresholds'

export type RawValue = WireRawValue
export type MetricValue = Pick<WireMetricValue, 'name' | 'description' | 'raw_value' | 'score'>
export type CategoryResult = Pick<WireCategoryResult, 'name' | 'score'> & { metrics: MetricValue[] }
export type ActionItem = Pick<WireActionItem, 'text' | 'target_tab' | 'sort_by'>
export type RemoteMeta = WireRemoteMeta
export type ScoreThresholds = WireScoreThresholds
export type HotspotFile = Pick<
  WireHotspotFile,
  | 'path'
  | 'role'
  | 'churn_count'
  | 'bug_commit_count'
  | 'loc'
  | 'total_lines'
  | 'cyclomatic_complexity'
  | 'public_methods'
  | 'properties'
  | 'hotspot_score'
  | 'content_findings'
  | 'common_findings'
  | 'control_findings'
  | 'inheritance_findings'
>
export type CouplingPair = Pick<WireCouplingPair, 'file_a' | 'file_b' | 'co_changes' | 'coupling_pct'>
export type AuthorShare = WireAuthorShare
export type FileOwnership = Pick<WireFileOwnership, 'path'> & { authors: AuthorShare[] }
export type FileAge = WireFileAge

type DashboardFields = Pick<
  WireAnalysisReport,
  | 'repo_name'
  | 'branch'
  | 'time_window_months'
  | 'total_commits'
  | 'total_authors'
  | 'total_files'
  | 'overall_score'
>

export type DashboardReport = DashboardFields & {
  categories: CategoryResult[]
  top_actions: ActionItem[]
  coupling_actions: ActionItem[]
  remote_meta: RemoteMeta | null
  file_hotspots: HotspotFile[]
  coupling_pairs: CouplingPair[]
  author_ownership: FileOwnership[]
  file_ages: FileAge[]
  score_thresholds: ScoreThresholds
}
