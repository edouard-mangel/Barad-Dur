import type {
  ActionItem,
  AuthorShare,
  CategoryResult,
  CouplingPair,
  DashboardReport,
  FileAge,
  FileOwnership,
  HotspotFile,
  MetricValue,
  RawValue,
  RemoteMeta,
  ScoreThresholds,
} from './model'
import {
  ReportDecodeError,
  arrayAt,
  countAt,
  finiteAt,
  integerAt,
  nullableCountAt,
  nullableStringAt,
  objectAt,
  optionalStringAt,
  scoreAt,
  stringAt,
  usableDateAt,
} from './readers'

const mapArray = <T>(value: unknown, path: string, decode: (value: unknown, path: string) => T): T[] =>
  arrayAt(value, path).map((item, index) => decode(item, `${path}[${index}]`))

const decodeRawValue = (value: unknown, path: string): RawValue => {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    throw new ReportDecodeError(path, 'expected exactly one raw-value tag')
  }
  const object = value as Record<string, unknown>
  const keys = Object.keys(object)
  const tags = ['Integer', 'Float', 'Percentage', 'Count', 'Text', 'List'] as const
  if (keys.length !== 1 || !tags.includes(keys[0] as typeof tags[number])) {
    throw new ReportDecodeError(path, 'expected exactly one raw-value tag')
  }

  const tag = keys[0] as typeof tags[number]
  const payload = object[tag]
  switch (tag) {
    case 'Integer': return { Integer: integerAt(payload, `${path}.Integer`) }
    case 'Float': return { Float: finiteAt(payload, `${path}.Float`) }
    case 'Percentage': return { Percentage: finiteAt(payload, `${path}.Percentage`) }
    case 'Count': return { Count: countAt(payload, `${path}.Count`) }
    case 'Text': return { Text: stringAt(payload, `${path}.Text`) }
    case 'List': return { List: mapArray(payload, `${path}.List`, stringAt) }
  }
}

const decodeMetric = (value: unknown, path: string): MetricValue => {
  const metric = objectAt(value, path)
  return {
    name: stringAt(metric.name, `${path}.name`),
    description: stringAt(metric.description, `${path}.description`),
    raw_value: decodeRawValue(metric.raw_value, `${path}.raw_value`),
    score: scoreAt(metric.score, `${path}.score`),
  }
}

const decodeCategory = (value: unknown, path: string): CategoryResult => {
  const category = objectAt(value, path)
  return {
    name: stringAt(category.name, `${path}.name`),
    score: scoreAt(category.score, `${path}.score`),
    metrics: mapArray(category.metrics, `${path}.metrics`, decodeMetric),
  }
}

const decodeAction = (value: unknown, path: string): ActionItem => {
  const action = objectAt(value, path)
  return {
    text: stringAt(action.text, `${path}.text`),
    target_tab: optionalStringAt(action, 'target_tab', path),
    sort_by: optionalStringAt(action, 'sort_by', path),
  }
}

const ROLES = ['source', 'test', 'config', 'docs', 'other'] as const

const decodeHotspot = (value: unknown, path: string): HotspotFile => {
  const hotspot = objectAt(value, path)
  const role = hotspot.role
  if (typeof role !== 'string' || !ROLES.includes(role as typeof ROLES[number])) {
    throw new ReportDecodeError(`${path}.role`, `expected one of ${ROLES.join(', ')}`)
  }
  return {
    path: stringAt(hotspot.path, `${path}.path`),
    role: role as HotspotFile['role'],
    churn_count: countAt(hotspot.churn_count, `${path}.churn_count`),
    bug_commit_count: countAt(hotspot.bug_commit_count, `${path}.bug_commit_count`),
    loc: countAt(hotspot.loc, `${path}.loc`),
    total_lines: countAt(hotspot.total_lines, `${path}.total_lines`),
    cyclomatic_complexity: countAt(hotspot.cyclomatic_complexity, `${path}.cyclomatic_complexity`),
    public_methods: countAt(hotspot.public_methods, `${path}.public_methods`),
    properties: countAt(hotspot.properties, `${path}.properties`),
    hotspot_score: finiteAt(hotspot.hotspot_score, `${path}.hotspot_score`),
    content_findings: countAt(hotspot.content_findings, `${path}.content_findings`),
    common_findings: countAt(hotspot.common_findings, `${path}.common_findings`),
    control_findings: countAt(hotspot.control_findings, `${path}.control_findings`),
    inheritance_findings: countAt(hotspot.inheritance_findings, `${path}.inheritance_findings`),
  }
}

const decodeCouplingPair = (value: unknown, path: string): CouplingPair => {
  const pair = objectAt(value, path)
  return {
    file_a: stringAt(pair.file_a, `${path}.file_a`),
    file_b: stringAt(pair.file_b, `${path}.file_b`),
    co_changes: countAt(pair.co_changes, `${path}.co_changes`),
    coupling_pct: finiteAt(pair.coupling_pct, `${path}.coupling_pct`),
  }
}

const decodeAuthorShare = (value: unknown, path: string): AuthorShare => {
  const author = objectAt(value, path)
  return {
    name: stringAt(author.name, `${path}.name`),
    pct: finiteAt(author.pct, `${path}.pct`),
  }
}

const decodeOwnership = (value: unknown, path: string): FileOwnership => {
  const ownership = objectAt(value, path)
  return {
    path: stringAt(ownership.path, `${path}.path`),
    authors: mapArray(ownership.authors, `${path}.authors`, decodeAuthorShare),
  }
}

const decodeFileAge = (value: unknown, path: string): FileAge => {
  const age = objectAt(value, path)
  return {
    path: stringAt(age.path, `${path}.path`),
    last_modified: usableDateAt(age.last_modified, `${path}.last_modified`),
    days_since_modified: integerAt(age.days_since_modified, `${path}.days_since_modified`),
  }
}

const decodeRemoteMeta = (value: unknown, path: string): RemoteMeta | null => {
  if (value === null) return null
  const remote = objectAt(value, path)
  return {
    url: stringAt(remote.url, `${path}.url`),
    stars: nullableCountAt(remote.stars, `${path}.stars`),
    description: nullableStringAt(remote.description, `${path}.description`),
    language: nullableStringAt(remote.language, `${path}.language`),
    open_issues: nullableCountAt(remote.open_issues, `${path}.open_issues`),
  }
}

const decodeThresholds = (value: unknown, path: string): ScoreThresholds => {
  const thresholds = objectAt(value, path)
  const good_min = scoreAt(thresholds.good_min, `${path}.good_min`)
  const warn_min = scoreAt(thresholds.warn_min, `${path}.warn_min`)
  if (good_min === null || warn_min === null) {
    throw new ReportDecodeError(path, 'thresholds cannot be null')
  }
  if (warn_min >= good_min) {
    throw new ReportDecodeError(path, 'warn_min must be below good_min')
  }
  return { good_min, warn_min }
}

export function decodeReport(value: unknown): DashboardReport {
  const report = objectAt(value, 'report')
  return {
    repo_name: stringAt(report.repo_name, 'report.repo_name'),
    branch: stringAt(report.branch, 'report.branch'),
    time_window_months: countAt(report.time_window_months, 'report.time_window_months'),
    total_commits: countAt(report.total_commits, 'report.total_commits'),
    total_authors: countAt(report.total_authors, 'report.total_authors'),
    total_files: countAt(report.total_files, 'report.total_files'),
    overall_score: scoreAt(report.overall_score, 'report.overall_score'),
    categories: mapArray(report.categories, 'report.categories', decodeCategory),
    top_actions: mapArray(report.top_actions, 'report.top_actions', decodeAction),
    coupling_actions: mapArray(report.coupling_actions, 'report.coupling_actions', decodeAction),
    remote_meta: decodeRemoteMeta(report.remote_meta, 'report.remote_meta'),
    file_hotspots: mapArray(report.file_hotspots, 'report.file_hotspots', decodeHotspot),
    coupling_pairs: mapArray(report.coupling_pairs, 'report.coupling_pairs', decodeCouplingPair),
    author_ownership: mapArray(report.author_ownership, 'report.author_ownership', decodeOwnership),
    file_ages: mapArray(report.file_ages, 'report.file_ages', decodeFileAge),
    score_thresholds: decodeThresholds(report.score_thresholds, 'report.score_thresholds'),
  }
}
