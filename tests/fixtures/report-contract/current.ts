import type { AnalysisReport } from "../../../dashboard/src/report/generated/AnalysisReport"

const currentReport: AnalysisReport = {
  "repo_name": "contract-fixture",
  "branch": "main",
  "time_window_months": 6,
  "total_commits": 12,
  "total_authors": 2,
  "total_files": 3,
  "overall_score": null,
  "categories": [
    {
      "name": "Health",
      "score": 70,
      "metrics": [
        {
          "name": "integer",
          "description": "integer fixture",
          "raw_value": {
            "Integer": -3
          },
          "score": 90
        },
        {
          "name": "float",
          "description": "float fixture",
          "raw_value": {
            "Float": 1.25
          },
          "score": 80
        },
        {
          "name": "percentage",
          "description": "percentage fixture",
          "raw_value": {
            "Percentage": 125.5
          },
          "score": 70
        },
        {
          "name": "count",
          "description": "count fixture",
          "raw_value": {
            "Count": 4
          },
          "score": 60
        },
        {
          "name": "text",
          "description": "text fixture",
          "raw_value": {
            "Text": "N/A"
          },
          "score": null
        },
        {
          "name": "list",
          "description": "list fixture",
          "raw_value": {
            "List": [
              "a.rs",
              "b.rs"
            ]
          },
          "score": 50
        }
      ]
    }
  ],
  "top_actions": [
    {
      "text": "[Health] integer (score: 90) — Inspect the hotspot",
      "target_tab": "hotspots",
      "sort_by": "complexity"
    },
    {
      "text": "[Health] list (score: 50) — Split responsibilities"
    }
  ],
  "coupling_actions": [
    {
      "text": "Reduce cross-boundary coupling",
      "target_tab": "coupling"
    }
  ],
  "remote_meta": {
    "url": "https://example.test/owner/repo",
    "stars": 42,
    "description": null,
    "language": "Rust",
    "open_issues": null
  },
  "file_hotspots": [
    {
      "path": "src/lib.rs",
      "role": "source",
      "churn_count": 7,
      "bug_commit_count": 2,
      "loc": 120,
      "total_lines": 150,
      "cyclomatic_complexity": 11,
      "public_methods": 4,
      "properties": 3,
      "hotspot_score": 87.5,
      "coupling_trend": {
        "first_half_partners": 2,
        "second_half_partners": 5
      },
      "content_findings": 1,
      "common_findings": 2,
      "control_findings": 3,
      "inheritance_findings": 4,
      "churn_timeline": [
        0,
        2,
        1
      ]
    }
  ],
  "coupling_pairs": [
    {
      "file_a": "src/lib.rs",
      "file_b": "tests/lib_test.rs",
      "co_changes": 5,
      "coupling_pct": 62.5,
      "cross_boundary": true,
      "is_test_pair": true,
      "growth_a": 10,
      "growth_b": -2
    }
  ],
  "author_ownership": [
    {
      "path": "src/lib.rs",
      "authors": [
        {
          "name": "Ada",
          "pct": 75.0
        }
      ]
    }
  ],
  "file_ages": [
    {
      "path": "src/lib.rs",
      "last_modified": "2025-01-02T03:04:05Z",
      "days_since_modified": 9
    }
  ],
  "author_cards": [
    {
      "name": "Ada",
      "email": "ada@example.test",
      "commit_count": 8,
      "files_owned": 1,
      "lines_owned": 90,
      "avg_commit_quality": 0.75,
      "top_files": [
        "src/lib.rs"
      ],
      "last_active": "2025-01-02T03:04:05Z",
      "days_since_active": 9,
      "directories_touched": 2
    }
  ],
  "history": [
    {
      "timestamp": "2025-01-02T03:04:05Z",
      "head": "0123456789abcdef",
      "overall_score": 70,
      "category_scores": {
        "Health": 70
      },
      "metrics": {
        "integer": 90
      },
      "counts": {
        "commits": 12,
        "files": 3,
        "authors": 2,
        "content_coupling": 1,
        "control_coupling": 3
      },
      "branch": "main",
      "schema_version": 5,
      "source": "fixture"
    }
  ],
  "dep_ecosystem_reports": [
    {
      "ecosystem": "Cargo",
      "total_deps": 1,
      "mean_drift_years": 2.5,
      "total_drift_years": 2.5,
      "critical_deps": [
        {
          "name": "example",
          "ecosystem": "Cargo",
          "current_version": "1.0.0",
          "drift_years": 2.5,
          "tier": "Stale",
          "vulnerabilities": [
            {
              "id": "CVE-2025-0001",
              "severity": "high",
              "description": "fixture vulnerability"
            }
          ]
        }
      ]
    }
  ],
  "audit": {
    "crisis_files": [
      {
        "path": "src/lib.rs",
        "crisis_commit_count": 1,
        "total_commit_count": 7,
        "crisis_ratio": 0.14285714285714285
      }
    ],
    "dir_concentration": [
      {
        "dir": "src",
        "file_count": 2,
        "loc": 200,
        "pct_of_total": 80.0
      }
    ],
    "dead_files": [
      {
        "path": "src/old.rs",
        "days_since_modified": 730,
        "churn_count": 1
      }
    ],
    "velocity_buckets": [
      {
        "week_start": "2024-12-30",
        "commit_count": 4,
        "author_count": 2
      }
    ]
  },
  "per_file_coupling": [
    {
      "path": "src/lib.rs",
      "ca": 2,
      "ce": 3,
      "instability": 0.6
    }
  ],
  "import_edges": [
    {
      "from": "src/main.rs",
      "to": "src/lib.rs"
    }
  ],
  "import_cycles": [
    [
      "src/a.rs",
      "src/b.rs"
    ]
  ],
  "coupling_finding_counts": {
    "content": 1,
    "common": 2,
    "inheritance": 4,
    "control": 3
  },
  "call_graph": {
    "resolution_rate": 0.8,
    "edges_resolved": 8,
    "edges_same_file": 1,
    "edges_unresolved": 2,
    "call_resolution_floor": 0.5,
    "function_hubs": [
      {
        "path": "src/lib.rs",
        "name": "run",
        "resolved_in_degree": 4
      }
    ]
  },
  "churn_timeline": {
    "bucket_days": 1,
    "merge_commits_excluded": true,
    "buckets": [
      {
        "date": "2025-01-02",
        "added": 12,
        "deleted": 3
      }
    ]
  },
  "score_thresholds": {
    "good_min": 71,
    "warn_min": 41
  },
  "long_method_thresholds": {
    "cc": 10,
    "cc_floor": 3,
    "loc": 50,
    "ui_loc": 100
  }
}

export default currentReport
