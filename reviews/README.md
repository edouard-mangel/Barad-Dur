# Merge-request review index (!107–!122)

Reviewed the exact MR deltas in sequence, inclusive of !107. Each MR has a separate note so findings can be fixed and closed independently.

| MR | Result | Highest severity |
|---:|---|---|
| [!107](mr-107.md) | 1 finding | High |
| [!108](mr-108.md) | Clean | — |
| [!109](mr-109.md) | Clean | — |
| [!110](mr-110.md) | Clean | — |
| [!111](mr-111.md) | Clean | — |
| [!112](mr-112.md) | Clean | — |
| [!113](mr-113.md) | Clean | — |
| [!114](mr-114.md) | 2 findings | Medium |
| [!115](mr-115.md) | 1 finding | High |
| [!116](mr-116.md) | 1 finding | Low |
| [!117](mr-117.md) | Clean | — |
| [!118](mr-118.md) | 2 findings | High |
| [!119](mr-119.md) | 1 finding | Medium |
| [!120](mr-120.md) | Clean | — |
| [!121](mr-121.md) | Clean | — |
| [!122](mr-122.md) | 2 findings | High |

## Totals

- 16 merge requests reviewed
- 10 actionable findings: 4 high, 5 medium, 1 low
- 9 merge requests with no actionable findings in their own delta

## Review method

The review compared each merged MR against the first parent of its merge commit; open MR !122 was compared against its target branch. Findings focus on correctness and regressions. Release/documentation-only MRs are marked clean when they introduce no separate defect; issues in bundled feature commits remain attributed to the feature MR that introduced them.
