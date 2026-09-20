# P2 responsibility recommendation audit — 2026-09-15

## Scope and evidence

Audited the final `field-test/archive/<repository>-0.json` reports against the
committed `field-test/baselines/*.surface.json` for all 11 pins in
`field-test/corpus.toml`. No baselines were changed by this review.

There are **39 added/changed displayed responsibility rows, containing 136
owner/prefix groups**, and **five withdrawn paths**. All 39 rows were checked,
including newly surfaced paths. The five existing recommendations below are one
corpus-wide allowance. `payp-app-front` and
`evolutionary-architecture-by-example` have no responsibility rows.

For each displayed path, the exact source was read with
`git -C /home/edouard/WS/<repository> show <pin>:<path>`. A disposable executable
calls the public `analyse_file` from the final release library
`libbarad_dur-74f627504bb226a0.rlib` (built after the final resolution changes).
Owner identities, member names, test flags and typed direct dependency identities
were saved, regrouped, and compared with the raw report text: **39/39 exact
owner/prefix/count/dependency matches**. The member ledger below records all
136 groups. Counts are function declarations; overloads and cfg alternatives
can repeat names. They are not claimed to be unique runtime methods.

Pinned source and machine-readable evidence remain under
`target/structural-verification/sources/`, `audit-evidence.json`,
`audit-members.log`, `provenance.rs`, and `collect-audit.py`. These are local
reproduction artifacts; the completed evidence ledger is committed here.
An initial debug-library probe was superseded by the final release-library
probe for every path; only the latter contributes to the final member ledger.

An independent surface comparison found **no numeric or hotspot drift in all
11 repositories**: overall/category/metric scores and unscored states, file,
commit and author totals, score thresholds, coupling finding counts, and the
ordered top-20 hotspot paths match the committed baselines. Every action-text
change is a responsibility suggestion. Local transcript:
`target/structural-verification/audit-independent-surface-comparison.json`.
The root review records the separate two-pass determinism result.

## Interpretation of the rubric

“True” below means the displayed structural evidence is present at the pin:
correct owner, declaration count and, for predicates, identical direct field or
callee. This does not prove that a refactor is worth doing. “Safe” assesses
whether an implementation can preserve the relevant API and behavior; no row
requires removing public methods, dropping synchronization or changing stored
schemas. The specific constraints are noted below. “Actionable” asks whether the
application maintainer has a meaningful local action. Generated/vendor files
fail that question even though their structural evidence is true.

## Displayed rows

Each entry below has its own True/Safe/Actionable assessment. The full member
ledger follows, keyed by the same repository and path.

### barad-dur (`dc23fbd6`)

| Path | True? | Safe? | Actionable? | Source assessment |
|---|---|---|---|---|
| `src/renderer/cli/mod.rs` | yes | yes | yes | Report-section rendering functions; the eight names stay in the file owner. Section helpers can be moved while retaining the render entry point. |
| `src/scorer/audit.rs` | yes | yes | yes | Audit construction and its crisis/dead-file/concentration/velocity facets; all five are file-level production functions. |
| `src/snapshot/mod.rs` | yes | yes | yes | Four index-building methods in the same RepoSnapshot impl at line 327; index construction can be isolated behind its existing API. |
| `src/cmd/analyze.rs` | yes | yes | partial (L2) | Two orchestration functions compute selected metrics and update trend/history. Owner evidence is correct; the generic verb remains a weak design hint (L2). |
| `src/scorer.rs` | yes | yes | yes | Newly surfaced: build_report and build_history_entry, both file-level. Report construction and history projection are concrete review entry points. |

### ripgrep (`3fce3b5b`)

| Path | True? | Safe? | Actionable? | Source assessment |
|---|---|---|---|---|
| `crates/ignore/src/walk.rs` | yes | yes | yes | WalkBuilder build_ignore/build_matchers/build_parallel stay together. DirEntry and Work each directly use their own dent field; their same field spelling does not join the owners. |
| `crates/regex/src/literal.rs` | yes | yes | yes | Newly surfaced: TSeq sequence-state queries share seq; its two quality predicates directly call the local len. Separate state/quality helper responsibilities are visible. |
| `crates/globset/src/glob.rs` | yes | yes | yes | Newly surfaced: Parser parse_backslash/parse_class/parse_comma/parse_star describe syntax handlers in one impl. |
| `crates/ignore/src/incremental.rs` | yes | yes | partial (L2) | Newly surfaced: IncrementalMatch is_ignore/is_none/is_whitelist all directly inspect mat. A match-result query boundary is concrete, though these are already short forwarding methods (L2). |
| `crates/index/src/literal.rs` | yes | yes | yes | Only GramQueryBuilder build_analysis/build_literal_or_hir survives. Unrelated owners and predicates with only transitive shared state no longer contribute. |

### helix (`079a789e`)

| Path | True? | Safe? | Actionable? | Source assessment |
|---|---|---|---|---|
| `helix-term/src/ui/statusline.rs` | yes | yes | yes | 24 statusline section renderers and two file-level lookup helpers; preserving the existing dispatch table permits focused renderer modules. |
| `helix-tui/src/buffer.rs` | yes | yes | yes | Buffer has eleven drawing/style setters; Cell has six character/style setters. Counts and owners remain separate. |
| `helix-view/src/document.rs` | yes | yes | yes | Three revision/view getters and thirteen document/view/language setters in the same Document impl. Review configuration, view-state and persistence facets while preserving document behavior. |
| `helix-view/src/editor.rs` | yes | yes | partial (L2) | Editor has three getters and eight setters. The two file-level get_terminal_provider declarations are cfg alternatives, not two runtime providers: that sub-group has weak applicability (L2). |
| `helix-term/src/application.rs` | yes | yes | yes | Ten handler declarations in Application, including two cfg-specific handle_signals declarations. Configuration, document, LSP and terminal handlers offer concrete extraction points; ten counts declarations, not unique spellings. |

### starship (`e939a19a`)

| Path | True? | Safe? | Actionable? | Source assessment |
|---|---|---|---|---|
| `src/modules/package.rs` | yes | yes | yes | 26 ecosystem/version readers and three dynamic-version parsers are file-owned. Language-specific manifest readers provide concrete module boundaries. |
| `src/modules/git_status.rs` | yes | yes | yes | GitStatusInfo has eighteen getters; RepoStatus has two ahead/behind setters; three helpers remain file-owned. Keeping status collection separate from presentation is actionable. |
| `src/context/mod.rs` | yes | yes | partial (L2) | Context getters, ScanDir setters, ScanAncestors setters and file helpers are now separate. The eight Context getters still cover several concerns (L2); directory-scanning owners give concrete boundaries. |
| `src/modules/aws.rs` | yes | yes | yes | Ten config/credential getters; has_credential_process_or_sso and has_source_profile both call get_config directly. has_defined_credentials is excluded from the selected predicate group. |
| `src/config.rs` | yes | yes | yes | Five StarshipConfig lookups and two file-level color/style parsers; get_palette is a separate owner singleton and no longer inflates the getter group. |

### dotnet-starter-kit (`b21bdd93`)

| Path | True? | Safe? | Actionable? | Source assessment |
|---|---|---|---|---|
| `src/apps/blazor/infrastructure/Api/ApiClient.cs` | yes | yes | **no (L1)** | Generated NSwag client: interface and concrete client each contain 30 Get declarations (15 cancellation-token overload pairs). The auto-generated header is at lines 1–5. Change the generator/spec, not this emitted client. See L1. |

### eShopModernizing (`63bc9ec4`)

| Path | True? | Safe? | Actionable? | Source assessment |
|---|---|---|---|---|
| `eShopLegacyMVCSolution/eShopPorted/wwwroot/Scripts/bootstrap.bundle.js` | yes | yes | **no (L1)** | Vendored Bootstrap/Popper distribution. Every owner and count matches this pinned copy; preserve package exports and modify upstream source/build configuration if necessary. Application-level extraction advice is inapplicable. See L1. |
| `eShopLegacyMVCSolution/src/eShopLegacyMVC/Scripts/bootstrap.bundle.js` | yes | yes | **no (L1)** | Vendored Bootstrap/Popper distribution. Every owner and count matches this pinned copy; preserve package exports and modify upstream source/build configuration if necessary. Application-level extraction advice is inapplicable. See L1. |
| `eShopLegacyWebFormsSolution/src/eShopLegacyWebForms/Scripts/bootstrap.bundle.js` | yes | yes | **no (L1)** | Vendored Bootstrap/Popper distribution. Every owner and count matches this pinned copy; preserve package exports and modify upstream source/build configuration if necessary. Application-level extraction advice is inapplicable. See L1. |
| `eShopModernizedMVCSolution/src/eShopModernizedMVC/Scripts/bootstrap.bundle.js` | yes | yes | **no (L1)** | Vendored Bootstrap/Popper distribution. Every owner and count matches this pinned copy; preserve package exports and modify upstream source/build configuration if necessary. Application-level extraction advice is inapplicable. See L1. |
| `eShopModernizedWebFormsSolution/src/eShopModernizedWebForms/Scripts/bootstrap.bundle.js` | yes | yes | **no (L1)** | Vendored Bootstrap/Popper distribution. Every owner and count matches this pinned copy; preserve package exports and modify upstream source/build configuration if necessary. Application-level extraction advice is inapplicable. See L1. |

### App-Serveat (`6fcfa756`)

| Path | True? | Safe? | Actionable? | Source assessment |
|---|---|---|---|---|
| `templates/dotnet-clean-architecture/BlazorClient/BlazorClient/wwwroot/lib/bootstrap/dist/js/bootstrap.bundle.js` | yes | yes | **no (L1)** | Vendored Bootstrap/Popper distribution. Every owner and count matches this pinned copy; preserve package exports and modify upstream source/build configuration if necessary. Application-level extraction advice is inapplicable. See L1. |
| `templates/dotnet-clean-architecture/BlazorClient/BlazorClient/wwwroot/lib/bootstrap/dist/js/bootstrap.esm.js` | yes | yes | **no (L1)** | Vendored Bootstrap/Popper distribution. Every owner and count matches this pinned copy; preserve package exports and modify upstream source/build configuration if necessary. Application-level extraction advice is inapplicable. See L1. |
| `templates/dotnet-clean-architecture/BlazorClient/BlazorClient/wwwroot/lib/bootstrap/dist/js/bootstrap.js` | yes | yes | **no (L1)** | Vendored Bootstrap/Popper distribution. Every owner and count matches this pinned copy; preserve package exports and modify upstream source/build configuration if necessary. Application-level extraction advice is inapplicable. See L1. |

### kairis-crm (`663493ef`)

| Path | True? | Safe? | Actionable? | Source assessment |
|---|---|---|---|---|
| `app/contacts/[id]/page.tsx` | yes | yes | yes | Fifteen handlers are nested in ContactDetailPage: contact edits, notes, tasks, queue/campaign actions and AI actions. Extract page-local action hooks/components while preserving React hook order and captured state. |
| `app/contacts/page.tsx` | yes | yes | yes | Fourteen handlers are nested in ContactsPage; bulk/contact/tag/sync actions are concrete page-local extraction candidates. |
| `app/pipeline/page.tsx` | yes | yes | yes | Seven PipelinePage handlers remain local. Three file-level deal predicates directly call isActiveDeal; isActiveDeal itself and isWonDealIncomplete do not join this group. |
| `app/campagnes/[id]/page.tsx` | yes | yes | yes | Seven handlers are nested in CampagneDetailPage: save/title, generate, enqueue, export, sync and delete. |
| `app/taches/page.tsx` | yes | yes | yes | Five TachesPage handlers remain local. isOverdue and isToday are file-level and directly call startOfToday. |

### mautic (`181701cd`)

| Path | True? | Safe? | Actionable? | Source assessment |
|---|---|---|---|---|
| `app/assets/js/libraries.js` | yes | yes | **no (L1)** | Bundled third-party libraries. Counts and owner boundaries match this exact pinned copy; direct predicate evidence is documented below. Application-level splitting advice does not belong on this distribution artifact. See L1. |
| `media/js/libraries.js` | yes | yes | **no (L1)** | Bundled third-party libraries. Counts and owner boundaries match this exact pinned copy; direct predicate evidence is documented below. Application-level splitting advice does not belong on this distribution artifact. See L1. |
| `app/bundles/LeadBundle/Entity/Lead.php` | yes | yes | partial (L2) | 49 getters and 34 setters are declared in Lead. Contact identity, location and lifecycle facets are reviewable; preserve Doctrine mappings, change tracking and the entity API when extracting helpers (L2). |
| `app/bundles/EmailBundle/Entity/Email.php` | yes | yes | partial (L2) | 35 getters and 30 setters belong to Email. Content, delivery configuration and reporting facets are concrete review entry points; preserve entity mappings and public accessors (L2). |
| `app/bundles/AssetBundle/Entity/Asset.php` | yes | yes | yes | Newly surfaced: 36 getters, 26 setters and isLocal/isRemote belong to Asset. Both predicates directly read storageLocation. Storage/path/upload behavior provides an extraction seam while preserving ORM mappings. |

## Direct predicate evidence

Every selected predicate member was checked in the pinned body, not through a
callee's dependencies. Identical labels in separate owners remain separate.

| Repository / path | Owner and members | Direct evidence at the pin |
|---|---|---|
| ripgrep `crates/ignore/src/walk.rs` | DirEntry: `is_stdin`, `is_dir` | Lines 56–57 and 103–104 directly read `self.dent`; impl line 36. |
| ripgrep `crates/ignore/src/walk.rs` | Work: `is_dir`, `is_symlink` | Lines 1578–1584 directly read `self.dent`; distinct impl line 1576 and distinct field identity. |
| ripgrep `crates/regex/src/literal.rs` | TSeq: `is_empty`, `is_exact`, `is_finite`, `is_inexact` | Lines 504–521 directly read `self.seq`; impl line 446. |
| ripgrep `crates/regex/src/literal.rs` | TSeq: `is_good`, `is_really_good` | Lines 558 and 575 call `self.len()`; the local body at 512–514 reads seq. The shared evidence is the direct call, not a transitive seq read. |
| ripgrep `crates/ignore/src/incremental.rs` | IncrementalMatch: `is_ignore`, `is_none`, `is_whitelist` | Lines 486–498 each directly read `self.mat`; impl line 455. |
| starship `src/modules/aws.rs` | file: `has_credential_process_or_sso`, `has_source_profile` | Direct `get_config` calls at lines 198 and 252 resolve to the same local declaration. |
| kairis-crm `app/pipeline/page.tsx` | file: `isDealLate`, `isDealStale`, `isDealWithoutTask` | Lines 66, 69 and 76 directly call the same file-local `isActiveDeal`. |
| kairis-crm `app/taches/page.tsx` | file: `isOverdue`, `isToday` | Lines 61 and 66 directly call file-local `startOfToday`. |
| App-Serveat Bootstrap bundle | wrapper at line 10: `isElement`, `isHTMLElement`, `isShadowRoot` | Direct `getWindow(node)` calls at lines 1737, 1742 and 1752; one local declaration identity. |
| Mautic `app/assets/js/libraries.js` | Moment wrapper line 6031: `isAfter`, `isBefore`, `isDaylightSavingTimeShifted`, `isSame` | Direct `local__createLocal` calls at 6392, 6394, 6367 and 6397; same local declaration. |
| Mautic `app/assets/js/libraries.js` | CodeMirror wrapper line 3001: `hasBadBidiRects`, `hasBadZoomedRects` | Direct `removeChildrenAndAdd` calls at lines 4037 and 4042, one local declaration. |
| Mautic `media/js/libraries.js` | Moment wrapper line 6033: same four members; CodeMirror wrapper line 3001: same two members | Independently analyzed this copy: identical member sets, distinct file-local declaration identities. Source offsets after the additional lines differ; owner labels correctly follow the copy. |
| Mautic `app/bundles/AssetBundle/Entity/Asset.php` | Asset: `isLocal`, `isRemote` | Class line 19 declares storageLocation at 39; lines 1381 and 1389 directly read `$this->storageLocation`. |

## Withdrawn paths

| Repository / path | True? | Safe? | Actionable? | Withdrawal evidence |
|---|---|---|---|---|
| barad-dur `src/metrics/coupling/mod.rs` | yes | yes | yes | `has_edge`, `has_import_extractable_files`, `has_detectable_files` all remain production file-owned functions, but all have empty eligible direct dependencies. The former `has_* (3)` claim is gone. |
| ripgrep `crates/matcher/src/lib.rs` | yes | yes | yes | The eight predicates span Match, LineTerminator, Captures, Matcher and the forwarding impl. Within Matcher, is_match calls is_match_at while is_match_at calls shortest_match_at: no identical direct dependency. No eligible group remains; the former `is_* (8)` claim is gone. |
| ripgrep `crates/core/flags/defs.rs` | yes | yes | yes | 108 is_switch declarations occupy separate flag impl owners and have no eligible direct dependencies; no owner contributes a two-member predicate group. |
| ripgrep `crates/globset/src/lib.rs` | yes | yes | yes | Strategy impls have separate owners; GlobSet's three predicates have distinct/empty direct dependencies. No shared-dependency pair remains. |
| Mautic `app/bundles/CoreBundle/Assets/js/libraries/51.Chart.js` | yes | yes | yes | Still has 44 qualifying nonpredicate declarations plus four Moment predicates sharing local__createLocal: 48 total. It now ranks below Asset's 64; withdrawal is the five-row cap, not a claim that the bundled file is small or unmeasured. |

## Five existing recommendations, corpus-wide

| Existing recommendation | True? | Safe? | Actionable? | Evidence and constraints |
|---|---|---|---|---|
| payp-app-front `getAllContacts.ts`: control flag | yes | yes | yes | Pin 2260b980: isPayp at line 12 chooses loadingType at 14 and the gateway mode at 19. Named entry points can retain the shared try/finally loading lifecycle. |
| helix `helix-stdx/src/env.rs`: shared mutable global | yes | yes | yes | Pin 079a789e: CWD is RwLock<Option<PathBuf>> at 12, read at 18 and written at 35/45. Explicit environment state must preserve the synchronization and process-directory semantics; no removal of those protections is required. |
| ripgrep `crates/core/messages.rs`: three common-coupling findings | yes | yes | yes | Pin 3fce3b5b: MESSAGES, IGNORE_MESSAGES and ERRORED are AtomicBool globals at 21/23/25 with load/store accessors. Injecting shared message state can preserve atomic access and the error-to-exit-status contract described at 9–15. |
| barad-dur Bus factor (score 25) | yes | yes | yes | Pinned report states one contributor covers 80% of attributable lines among two active authors; raw Count=1. Pairing/review coverage follows that concentration evidence without claiming every repository line is attributable. |
| kairis-crm Long methods (score 25) | yes | yes | yes | Pinned report lists 117/595 flagged functions; ContactsPage and ContactDetailPage occur in the same source inspected above and contain the listed handlers. Component/action extraction is concrete; preserve React hook order and asynchronous state updates. |

## Limitations and explicit dispositions

### L1 — generated/vendor files: Actionable failure (11 rows)

**Local issue note, owner: responsibility-advice/exclusion policy.** ApiClient
(one row), Bootstrap distributions (five eShopModernizing and three App-Serveat
rows), and Mautic's two libraries.js bundles are emitted or vendored artifacts.
The pinned headers/source identify NSwag, Bootstrap/Popper, jQuery/CodeMirror and
Moment. An application maintainer should work through generation, dependency
updates, bundling or exclusions. The present suggestions do not convey that.

Disposition: **corpus-tested, retained as an explicit known limitation**, rather
than reclassified as actionable or silently deferred. All eleven locations were
already responsibility-advice paths in the committed baselines; this change adds
owner/direct-dependency evidence and does not introduce vendor targeting. Fix #9
preserves the collection/exclusion population and nonpredicate prefix policy.
A follow-up should suppress or redirect structural refactoring advice for
identified generated/vendor artifacts, with regression examples for all four
corpus repositories. No external ticket or message was sent. No destructive edit
or incompatible API change is required by these suggestions, so this is an
Actionable failure, not a detected Safe failure.

### L2 — broad nonpredicate verbs and platform alternatives

Disposition: **corpus-tested and explicitly retired as a blocker for this
owner/direct-predicate change**. The cmd/analyze compute pair, Context getters,
entity accessor lists and tiny wrapper predicates are evidence-backed review
entry points, but do not prove a useful new architecture. Helix's two
get_terminal_provider declarations are mutually exclusive cfg variants, and
handle_signals also has platform variants. The declaration counts are true;
splitting the two alternative provider bodies solely because of the count has
little value. The editor row retains eleven other concrete Editor methods.

The binding change intentionally retains nonpredicate prefix behavior. No
repeated spelling was falsely counted as two owners or two runtime methods in
this audit. A future usefulness/ranking improvement can distinguish platform
alternatives and generated accessors; that is a separate local product-policy
note, not an unrecorded defect in direct dependency resolution.

## Decision

**No Safe failure was found.** All displayed structural counts/owners and all
predicate witnesses match the pinned sources. The two issue-#9 motivating rows
are withdrawn. Eleven pre-existing generated/vendor rows fail Actionable and
have the explicit L1 disposition above; weaker nonpredicate hints are recorded
under L2. Baseline acceptance can proceed after the separate determinism and
required review checks complete; this worksheet does not attest to those runs.

## Complete owner/member ledger

The following is the checked structural evidence for every displayed row.
Repeated names are shown with multiplicity. `file` denotes the file owner, not
an unknown owner. A dependency identity is file-local; equal labels in different
files or owners are not merged.

### barad-dur — `src/renderer/cli/mod.rs`

- **file / `render_*` (8)**: `render_actions_and_footer`, `render_categories`, `render_dep_section`, `render_repo_info`, `render_score_and_trend`, `render_single_category`, `render_top_unstable_files`, `render_trend_line`.

### barad-dur — `src/scorer/audit.rs`

- **file / `build_*` (5)**: `build_audit_report`, `build_crisis_files`, `build_dead_files`, `build_dir_concentration`, `build_velocity_buckets`.

### barad-dur — `src/snapshot/mod.rs`

- **RepoSnapshot (line 327) / `build_*` (4)**: `build_commits_by_author`, `build_commits_by_file`, `build_file_change_pairs`, `build_indexes`.

### barad-dur — `src/cmd/analyze.rs`

- **file / `compute_*` (2)**: `compute_selected_metrics`, `compute_trend_and_update_history`.

### barad-dur — `src/scorer.rs`

- **file / `build_*` (2)**: `build_history_entry`, `build_report`.

### ripgrep — `crates/ignore/src/walk.rs`

- **WalkBuilder (line 545) / `build_*` (3)**: `build_ignore`, `build_matchers`, `build_parallel`.
- **Work (line 1576) / `is_*` (2)**; direct field `dent` (`impl_item:54872:field:dent`): `is_dir`, `is_symlink`.
- **DirEntry (line 36) / `is_*` (2)**; direct field `dent` (`impl_item:800:field:dent`): `is_dir`, `is_stdin`.

### ripgrep — `crates/regex/src/literal.rs`

- **TSeq (line 446) / `is_*` (4)**; direct field `seq` (`impl_item:17714:field:seq`): `is_empty`, `is_exact`, `is_finite`, `is_inexact`.
- **TSeq (line 446) / `is_*` (2)**; direct callee `len` (`function:19048`): `is_good`, `is_really_good`.

### ripgrep — `crates/globset/src/glob.rs`

- **Parser<'a> (line 818) / `parse_*` (4)**: `parse_backslash`, `parse_class`, `parse_comma`, `parse_star`.

### ripgrep — `crates/ignore/src/incremental.rs`

- **IncrementalMatch (line 455) / `is_*` (3)**; direct field `mat` (`impl_item:18027:field:mat`): `is_ignore`, `is_none`, `is_whitelist`.

### ripgrep — `crates/index/src/literal.rs`

- **GramQueryBuilder (line 643) / `build_*` (2)**: `build_analysis`, `build_literal_or_hir`.

### helix — `helix-term/src/ui/statusline.rs`

- **file / `get_*` (2)**: `get_position`, `get_render_function`.
- **file / `render_*` (24)**: `render_code_action_hint`, `render_cwd`, `render_diagnostics`, `render_file_absolute_path`, `render_file_base_name`, `render_file_encoding`, `render_file_indent_style`, `render_file_line_ending`, `render_file_modification_indicator`, `render_file_name`, `render_file_type`, `render_lsp_spinner`, `render_mode`, `render_position`, `render_position_percentage`, `render_primary_selection_length`, `render_read_only_indicator`, `render_register`, `render_selections`, `render_separator`, `render_spacer`, `render_total_line_numbers`, `render_version_control`, `render_workspace_diagnostics`.

### helix — `helix-tui/src/buffer.rs`

- **Buffer (line 170) / `set_*` (11)**: `set_background`, `set_grapheme`, `set_span`, `set_spans`, `set_spans_truncated`, `set_string`, `set_string_anchored`, `set_string_truncated`, `set_stringn`, `set_style`, `set_tab`.
- **Cell (line 27) / `set_*` (6)**: `set_bg`, `set_char`, `set_fg`, `set_style`, `set_symbol`, `set_symbol_with_width`.

### helix — `helix-view/src/document.rs`

- **Document (line 721) / `get_*` (3)**: `get_current_revision`, `get_last_saved_revision`, `get_view_offset`.
- **Document (line 721) / `set_*` (13)**: `set_code_action_hints`, `set_diff_base`, `set_document_highlights`, `set_encoding`, `set_inlay_hints`, `set_jump_labels`, `set_language`, `set_language_by_language_id`, `set_last_saved_revision`, `set_path`, `set_selection`, `set_version_control_head`, `set_view_offset`.

### helix — `helix-view/src/editor.rs`

- **Editor (line 1412) / `get_*` (3)**: `get_last_cwd`, `get_status`, `get_synced_view_id`.
- **Editor (line 1412) / `set_*` (8)**: `set_cwd`, `set_doc_path`, `set_error`, `set_status`, `set_theme`, `set_theme_impl`, `set_theme_preview`, `set_warning`.
- **file / `get_*` (2)**: `get_terminal_provider` ×2.

### helix — `helix-term/src/application.rs`

- **Application (line 93) / `handle_*` (10)**: `handle_config_events`, `handle_document_write`, `handle_editor_event`, `handle_idle_timeout`, `handle_language_server_message`, `handle_show_document`, `handle_show_message`, `handle_signals` ×2, `handle_terminal_events`.

### starship — `src/modules/package.rs`

- **file / `get_*` (26)**: `get_cargo_version`, `get_composer_version`, `get_daml_project_version`, `get_dart_pub_version`, `get_galaxy_version`, `get_gradle_version`, `get_helm_package_version`, `get_jsr_package_version`, `get_julia_project_version`, `get_maven_version`, `get_meson_version`, `get_mix_version`, `get_nimble_version`, `get_node_package_version`, `get_pep621_dynamic_version`, `get_pep621_static_version`, `get_pep621_version`, `get_poetry_version`, `get_pyproject_version`, `get_rlang_version`, `get_sbt_version`, `get_setup_cfg_version`, `get_shard_version`, `get_version`, `get_vmod_version`, `get_vpkg_version`.
- **file / `parse_*` (3)**: `parse_file_version_for_hatchling`, `parse_hatchling_dynamic_version`, `parse_pep621_dynamic_version`.

### starship — `src/modules/git_status.rs`

- **RepoStatus (line 627) / `set_*` (2)**: `set_ahead_behind`, `set_ahead_behind_for_each_ref`.
- **GitStatusInfo<'a> (line 200) / `get_*` (18)**: `get_ahead_behind`, `get_conflicted`, `get_deleted`, `get_index_added`, `get_index_deleted`, `get_index_modified`, `get_index_typechanged`, `get_modified`, `get_renamed`, `get_repo_status`, `get_staged`, `get_stashed`, `get_typechanged`, `get_untracked`, `get_worktree_added`, `get_worktree_deleted`, `get_worktree_modified`, `get_worktree_typechanged`.
- **file / `get_*` (3)**: `get_repo_status`, `get_stashed_count`, `get_static_repo_status`.

### starship — `src/context/mod.rs`

- **ScanDir<'a> (line 747) / `set_*` (3)**: `set_extensions`, `set_files`, `set_folders`.
- **ScanAncestors<'a> (line 790) / `set_*` (2)**: `set_files`, `set_folders`.
- **Context<'a> (line 103) / `get_*` (8)**: `get_cmd_duration`, `get_config_path_os`, `get_env`, `get_env_os`, `get_git_repo`, `get_home`, `get_jj_repo`, `get_shell`.
- **file / `get_*` (3)**: `get_config_path_os`, `get_current_branch`, `get_remote_repository_info`.
- **file / `parse_*` (3)**: `parse_i64`, `parse_trim`, `parse_width`.

### starship — `src/modules/aws.rs`

- **file / `get_*` (10)**: `get_aws_profile_and_region`, `get_aws_region_from_config`, `get_config`, `get_config_file_path`, `get_credentials_duration`, `get_credentials_file_path`, `get_creds`, `get_profile_config`, `get_profile_creds`, `get_sso_cache_key_input`.
- **file / `has_*` (2)**; direct callee `get_config` (`function:1232`): `has_credential_process_or_sso`, `has_source_profile`.

### starship — `src/config.rs`

- **StarshipConfig (line 125) / `get_*` (5)**: `get_config`, `get_custom_module_config`, `get_custom_modules`, `get_env_var_modules`, `get_module_config`.
- **file / `parse_*` (2)**: `parse_color_string`, `parse_style_string`.

### dotnet-starter-kit — `src/apps/blazor/infrastructure/Api/ApiClient.cs`

- **ApiClient (line 986) / `get_*` (30)**: `GetBrandEndpointAsync` ×2, `GetMeEndpointAsync` ×2, `GetProductEndpointAsync` ×2, `GetRoleByIdEndpointAsync` ×2, `GetRolePermissionsEndpointAsync` ×2, `GetRolesEndpointAsync` ×2, `GetTenantByIdEndpointAsync` ×2, `GetTenantsEndpointAsync` ×2, `GetTodoEndpointAsync` ×2, `GetTodoListEndpointAsync` ×2, `GetUserAuditTrailEndpointAsync` ×2, `GetUserEndpointAsync` ×2, `GetUserPermissionsAsync` ×2, `GetUserRolesEndpointAsync` ×2, `GetUsersListEndpointAsync` ×2.
- **IApiClient (line 27) / `get_*` (30)**: `GetBrandEndpointAsync` ×2, `GetMeEndpointAsync` ×2, `GetProductEndpointAsync` ×2, `GetRoleByIdEndpointAsync` ×2, `GetRolePermissionsEndpointAsync` ×2, `GetRolesEndpointAsync` ×2, `GetTenantByIdEndpointAsync` ×2, `GetTenantsEndpointAsync` ×2, `GetTodoEndpointAsync` ×2, `GetTodoListEndpointAsync` ×2, `GetUserAuditTrailEndpointAsync` ×2, `GetUserEndpointAsync` ×2, `GetUserPermissionsAsync` ×2, `GetUserRolesEndpointAsync` ×2, `GetUsersListEndpointAsync` ×2.

### eShopModernizing — `eShopLegacyMVCSolution/eShopPorted/wwwroot/Scripts/bootstrap.bundle.js`

- **function_expression (line 10) / `compute_*` (2)**: `computeAutoPlacement`, `computeStyle`.
- **function_expression (line 10) / `get_*` (25)**: `getArea`, `getBordersSize`, `getBoundaries`, `getBoundingClientRect`, `getClientRect`, `getFixedPositionOffsetParent`, `getOffsetParent`, `getOffsetRectRelativeToArbitraryNode`, `getOppositePlacement`, `getOppositeVariation`, `getOuterSizes`, `getParentNode`, `getPopperOffsets`, `getReferenceOffsets`, `getRoot`, `getRoundedOffsets`, `getScroll`, `getScrollParent`, `getSize`, `getSpecialTransitionEndEvent`, `getStyleComputedProperty`, `getSupportedPropertyName`, `getViewportOffsetRectRelativeToArtbitraryNode`, `getWindow`, `getWindowSizes`.
- **function_expression (line 10) / `set_*` (3)**: `setAttributes`, `setStyles`, `setTransitionEndSupport`.

### eShopModernizing — `eShopLegacyMVCSolution/src/eShopLegacyMVC/Scripts/bootstrap.bundle.js`

- **function_expression (line 10) / `compute_*` (2)**: `computeAutoPlacement`, `computeStyle`.
- **function_expression (line 10) / `get_*` (25)**: `getArea`, `getBordersSize`, `getBoundaries`, `getBoundingClientRect`, `getClientRect`, `getFixedPositionOffsetParent`, `getOffsetParent`, `getOffsetRectRelativeToArbitraryNode`, `getOppositePlacement`, `getOppositeVariation`, `getOuterSizes`, `getParentNode`, `getPopperOffsets`, `getReferenceOffsets`, `getRoot`, `getRoundedOffsets`, `getScroll`, `getScrollParent`, `getSize`, `getSpecialTransitionEndEvent`, `getStyleComputedProperty`, `getSupportedPropertyName`, `getViewportOffsetRectRelativeToArtbitraryNode`, `getWindow`, `getWindowSizes`.
- **function_expression (line 10) / `set_*` (3)**: `setAttributes`, `setStyles`, `setTransitionEndSupport`.

### eShopModernizing — `eShopLegacyWebFormsSolution/src/eShopLegacyWebForms/Scripts/bootstrap.bundle.js`

- **function_expression (line 10) / `compute_*` (2)**: `computeAutoPlacement`, `computeStyle`.
- **function_expression (line 10) / `get_*` (25)**: `getArea`, `getBordersSize`, `getBoundaries`, `getBoundingClientRect`, `getClientRect`, `getFixedPositionOffsetParent`, `getOffsetParent`, `getOffsetRectRelativeToArbitraryNode`, `getOppositePlacement`, `getOppositeVariation`, `getOuterSizes`, `getParentNode`, `getPopperOffsets`, `getReferenceOffsets`, `getRoot`, `getRoundedOffsets`, `getScroll`, `getScrollParent`, `getSize`, `getSpecialTransitionEndEvent`, `getStyleComputedProperty`, `getSupportedPropertyName`, `getViewportOffsetRectRelativeToArtbitraryNode`, `getWindow`, `getWindowSizes`.
- **function_expression (line 10) / `set_*` (3)**: `setAttributes`, `setStyles`, `setTransitionEndSupport`.

### eShopModernizing — `eShopModernizedMVCSolution/src/eShopModernizedMVC/Scripts/bootstrap.bundle.js`

- **function_expression (line 10) / `compute_*` (2)**: `computeAutoPlacement`, `computeStyle`.
- **function_expression (line 10) / `get_*` (25)**: `getArea`, `getBordersSize`, `getBoundaries`, `getBoundingClientRect`, `getClientRect`, `getFixedPositionOffsetParent`, `getOffsetParent`, `getOffsetRectRelativeToArbitraryNode`, `getOppositePlacement`, `getOppositeVariation`, `getOuterSizes`, `getParentNode`, `getPopperOffsets`, `getReferenceOffsets`, `getRoot`, `getRoundedOffsets`, `getScroll`, `getScrollParent`, `getSize`, `getSpecialTransitionEndEvent`, `getStyleComputedProperty`, `getSupportedPropertyName`, `getViewportOffsetRectRelativeToArtbitraryNode`, `getWindow`, `getWindowSizes`.
- **function_expression (line 10) / `set_*` (3)**: `setAttributes`, `setStyles`, `setTransitionEndSupport`.

### eShopModernizing — `eShopModernizedWebFormsSolution/src/eShopModernizedWebForms/Scripts/bootstrap.bundle.js`

- **function_expression (line 10) / `compute_*` (2)**: `computeAutoPlacement`, `computeStyle`.
- **function_expression (line 10) / `get_*` (25)**: `getArea`, `getBordersSize`, `getBoundaries`, `getBoundingClientRect`, `getClientRect`, `getFixedPositionOffsetParent`, `getOffsetParent`, `getOffsetRectRelativeToArbitraryNode`, `getOppositePlacement`, `getOppositeVariation`, `getOuterSizes`, `getParentNode`, `getPopperOffsets`, `getReferenceOffsets`, `getRoot`, `getRoundedOffsets`, `getScroll`, `getScrollParent`, `getSize`, `getSpecialTransitionEndEvent`, `getStyleComputedProperty`, `getSupportedPropertyName`, `getViewportOffsetRectRelativeToArtbitraryNode`, `getWindow`, `getWindowSizes`.
- **function_expression (line 10) / `set_*` (3)**: `setAttributes`, `setStyles`, `setTransitionEndSupport`.

### App-Serveat — `templates/dotnet-clean-architecture/BlazorClient/BlazorClient/wwwroot/lib/bootstrap/dist/js/bootstrap.bundle.js`

- **BaseComponent (line 656) / `get_*` (2)**: `getInstance`, `getOrCreateInstance`.
- **function_expression (line 10) / `compute_*` (3)**: `computeAutoPlacement`, `computeOffsets`, `computeStyles`.
- **function_expression (line 10) / `get_*` (34)**: `getAltAxis`, `getBasePlacement`, `getBoundingClientRect`, `getClientRectFromMixedType`, `getClippingParents`, `getClippingRect`, `getCompositeRect`, `getComputedStyle$1`, `getContainingBlock`, `getDocumentElement`, `getDocumentRect`, `getElementEvents`, `getExpandedFallbackPlacements`, `getFreshSideObject`, `getHTMLElementScroll`, `getInnerBoundingClientRect`, `getLayoutRect`, `getMainAxisFromPlacement`, `getNodeName`, `getNodeScroll`, `getOffsetParent`, `getOppositePlacement`, `getOppositeVariationPlacement`, `getParentNode`, `getScrollParent`, `getSideOffsets`, `getTrueOffsetParent`, `getTypeEvent`, `getUAString`, `getVariation`, `getViewportRect`, `getWindow`, `getWindowScroll`, `getWindowScrollBarX`.
- **function_expression (line 10) / `is_*` (3)**; direct callee `getWindow` (`function:56867`): `isElement`, `isHTMLElement`, `isShadowRoot`.
- **object (line 560) / `get_*` (2)**: `getDataAttribute`, `getDataAttributes`.
- **object (line 735) / `get_*` (3)**: `getElementFromSelector`, `getMultipleElementsFromSelector`, `getSelectorFromElement`.

### App-Serveat — `templates/dotnet-clean-architecture/BlazorClient/BlazorClient/wwwroot/lib/bootstrap/dist/js/bootstrap.esm.js`

- **BaseComponent (line 652) / `get_*` (2)**: `getInstance`, `getOrCreateInstance`.
- **object (line 556) / `get_*` (2)**: `getDataAttribute`, `getDataAttributes`.
- **object (line 731) / `get_*` (3)**: `getElementFromSelector`, `getMultipleElementsFromSelector`, `getSelectorFromElement`.
- **file / `get_*` (2)**: `getElementEvents`, `getTypeEvent`.

### App-Serveat — `templates/dotnet-clean-architecture/BlazorClient/BlazorClient/wwwroot/lib/bootstrap/dist/js/bootstrap.js`

- **BaseComponent (line 675) / `get_*` (2)**: `getInstance`, `getOrCreateInstance`.
- **function_expression (line 10) / `get_*` (2)**: `getElementEvents`, `getTypeEvent`.
- **object (line 579) / `get_*` (2)**: `getDataAttribute`, `getDataAttributes`.
- **object (line 754) / `get_*` (3)**: `getElementFromSelector`, `getMultipleElementsFromSelector`, `getSelectorFromElement`.

### kairis-crm — `app/contacts/[id]/page.tsx`

- **ContactDetailPage (line 160) / `handle_*` (15)**: `handleAddNote`, `handleAddTask`, `handleAddToCampagne`, `handleAddToQueue`, `handleDeleteContact`, `handleDeleteNote`, `handleDeleteTask`, `handleGenerateAI`, `handleGenerateReply`, `handleLogEvent`, `handleOpenCompany`, `handleRescoreContact`, `handleSave`, `handleSaveLinkedinUrl`, `handleToggleTask`.

### kairis-crm — `app/contacts/page.tsx`

- **ContactsPage (line 120) / `handle_*` (14)**: `handleArchive`, `handleBulkDelete`, `handleBulkTag`, `handleCleanup`, `handleCreateContact`, `handleDelete`, `handleEnrich`, `handleInterest`, `handleOpenCompanyFromList`, `handlePurge`, `handleRemoveTag`, `handleSort`, `handleStopScoring`, `handleSyncOdoo`.

### kairis-crm — `app/pipeline/page.tsx`

- **PipelinePage (line 188) / `handle_*` (7)**: `handleCreateContact`, `handleDelete`, `handleDragStart`, `handleDrop`, `handleOpenClose`, `handleSave`, `handleSort`.
- **file / `is_*` (3)**; direct callee `isActiveDeal` (`function:2671`): `isDealLate`, `isDealStale`, `isDealWithoutTask`.

### kairis-crm — `app/campagnes/[id]/page.tsx`

- **CampagneDetailPage (line 68) / `handle_*` (7)**: `handleDelete`, `handleEnqueue`, `handleExport`, `handleGenerate`, `handleSave`, `handleSaveTitle`, `handleSync`.

### kairis-crm — `app/taches/page.tsx`

- **TachesPage (line 116) / `handle_*` (5)**: `handleCreate`, `handleDeleteEdit`, `handleDeleteRow`, `handleSaveEdit`, `handleSort`.
- **file / `is_*` (2)**; direct callee `startOfToday` (`function:2252`): `isOverdue`, `isToday`.

### mautic — `app/assets/js/libraries.js`

- **function_expression (line 37) / `get_*` (2)**: `getElements`, `getExpandoData`.
- **function_expression (line 5417) / `get_*` (3)**: `getLocal`, `getPrefetch`, `getRemote`.
- **function_expression (line 5434) / `get_*` (2)**: `getFlush`, `getNextTick`.
- **function_expression (line 5465) / `get_*` (2)**: `getDisplayFn`, `getTemplates`.
- **render (line 5468) / `get_*` (4)**: `getEmptyHtml`, `getFooterHtml`, `getHeaderHtml`, `getSuggestionsHtml`.
- **function_expression (line 5738) / `handle_*` (2)**: `handleClick`, `handleHover`.
- **function_expression (line 5760) / `get_*` (2)**: `getNotification`, `getWrapper`.
- **function_expression (line 5882) / `get_*` (6)**: `getAlpha`, `getHsl`, `getHsla`, `getHwb`, `getRgb`, `getRgba`.
- **function_expression (line 6031) / `compute_*` (2)**: `computeMonthsParse`, `computeWeekdaysParse`.
- **function_expression (line 6031) / `get_*` (26)**: `getCalendarFormat`, `getDateOffset`, `getDaysInMonth`, `getISOWeeksInYear`, `getIsLeapYear`, `getParseRegexForToken`, `getParsingFlags`, `getPrioritizedUnits`, `getSetDayOfWeek`, `getSetDayOfYear`, `getSetISODayOfWeek`, `getSetISOWeek`, `getSetISOWeekYear`, `getSetLocaleDayOfWeek`, `getSetMonth`, `getSetOffset`, `getSetQuarter`, `getSetWeek`, `getSetWeekYear`, `getSetWeekYearHelper`, `getSetZone`, `getWeeksInYear`, `getZoneAbbr`, `getZoneName`, `get_set__get`, `get_set__set`.
- **function_expression (line 6031) / `is_*` (4)**; direct callee `local__createLocal` (`function:1553214`): `isAfter`, `isBefore`, `isDaylightSavingTimeShifted`, `isSame`.
- **function_expression (line 6031) / `parse_*` (4)**: `parseIso`, `parseIsoWeekday`, `parseMs`, `parseWeekday`.
- **function_expression (line 6031) / `set_*` (6)**: `setHookCallback`, `setMonth`, `setOffsetToLocal`, `setOffsetToParsedOffset`, `setOffsetToUTC`, `setWeekAll`.
- **function_expression (line 1) / `set_*` (2)**: `setCss`, `setCssAll`.
- **function_expression (line 138) / `build_*` (2)**: `buildFragment`, `buildParams`.
- **function_expression (line 138) / `get_*` (5)**: `getAll`, `getClass`, `getData`, `getDefaultDisplay`, `getWidthOrHeight`.
- **function_expression (line 138) / `set_*` (2)**: `setGlobalEval`, `setPositiveNumber`.
- **function_expression (line 172) / `set_*` (2)**: `setFilters`, `setMatcher`.
- **function_expression (line 2016) / `get_*` (2)**: `getDimensions`, `getOffsets`.
- **function_expression (line 3001) / `build_*` (6)**: `buildCollapsedSpan`, `buildLineContent`, `buildLineElement`, `buildToken`, `buildTokenBadBidi`, `buildViewArray`.
- **function_expression (line 3001) / `compute_*` (2)**: `computeReplacedSel`, `computeSelAfterChange`.
- **function_expression (line 3001) / `get_*` (13)**: `getBetween`, `getBidiPartAt`, `getDimensions`, `getHandlers`, `getKeyMap`, `getLine`, `getLineContent`, `getLineStyles`, `getLines`, `getMarkedSpanFor`, `getOldSpans`, `getOrder`, `getStateBefore`.
- **function_expression (line 3001) / `handle_*` (3)**: `handleCharBinding`, `handleKeyBinding`, `handlePaste`.
- **function_expression (line 3001) / `has_*` (2)**; direct callee `removeChildrenAndAdd` (`function:818624`): `hasBadBidiRects`, `hasBadZoomedRects`.
- **function_expression (line 3001) / `set_*` (9)**: `setDocumentHeight`, `setGuttersForLineNumbers`, `setScrollLeft`, `setScrollTop`, `setSelection`, `setSelectionInner`, `setSelectionNoUndo`, `setSelectionReplaceHistory`, `setSimpleSelection`.
- **function_expression (line 4085) / `get_*` (2)**: `getHintElement`, `getText`.
- **function_expression (line 4204) / `get_*` (3)**: `getAttrRegexp`, `getAttrValue`, `getTagRegexp`.

### mautic — `media/js/libraries.js`

- **function_expression (line 37) / `get_*` (2)**: `getElements`, `getExpandoData`.
- **function_expression (line 5417) / `get_*` (3)**: `getLocal`, `getPrefetch`, `getRemote`.
- **function_expression (line 5434) / `get_*` (2)**: `getFlush`, `getNextTick`.
- **function_expression (line 5465) / `get_*` (2)**: `getDisplayFn`, `getTemplates`.
- **render (line 5468) / `get_*` (4)**: `getEmptyHtml`, `getFooterHtml`, `getHeaderHtml`, `getSuggestionsHtml`.
- **function_expression (line 5740) / `handle_*` (2)**: `handleClick`, `handleHover`.
- **function_expression (line 5762) / `get_*` (2)**: `getNotification`, `getWrapper`.
- **function_expression (line 5884) / `get_*` (6)**: `getAlpha`, `getHsl`, `getHsla`, `getHwb`, `getRgb`, `getRgba`.
- **function_expression (line 6033) / `compute_*` (2)**: `computeMonthsParse`, `computeWeekdaysParse`.
- **function_expression (line 6033) / `get_*` (26)**: `getCalendarFormat`, `getDateOffset`, `getDaysInMonth`, `getISOWeeksInYear`, `getIsLeapYear`, `getParseRegexForToken`, `getParsingFlags`, `getPrioritizedUnits`, `getSetDayOfWeek`, `getSetDayOfYear`, `getSetISODayOfWeek`, `getSetISOWeek`, `getSetISOWeekYear`, `getSetLocaleDayOfWeek`, `getSetMonth`, `getSetOffset`, `getSetQuarter`, `getSetWeek`, `getSetWeekYear`, `getSetWeekYearHelper`, `getSetZone`, `getWeeksInYear`, `getZoneAbbr`, `getZoneName`, `get_set__get`, `get_set__set`.
- **function_expression (line 6033) / `is_*` (4)**; direct callee `local__createLocal` (`function:1657683`): `isAfter`, `isBefore`, `isDaylightSavingTimeShifted`, `isSame`.
- **function_expression (line 6033) / `parse_*` (4)**: `parseIso`, `parseIsoWeekday`, `parseMs`, `parseWeekday`.
- **function_expression (line 6033) / `set_*` (6)**: `setHookCallback`, `setMonth`, `setOffsetToLocal`, `setOffsetToParsedOffset`, `setOffsetToUTC`, `setWeekAll`.
- **function_expression (line 1) / `set_*` (2)**: `setCss`, `setCssAll`.
- **function_expression (line 138) / `build_*` (2)**: `buildFragment`, `buildParams`.
- **function_expression (line 138) / `get_*` (5)**: `getAll`, `getClass`, `getData`, `getDefaultDisplay`, `getWidthOrHeight`.
- **function_expression (line 138) / `set_*` (2)**: `setGlobalEval`, `setPositiveNumber`.
- **function_expression (line 172) / `set_*` (2)**: `setFilters`, `setMatcher`.
- **function_expression (line 2016) / `get_*` (2)**: `getDimensions`, `getOffsets`.
- **function_expression (line 3001) / `build_*` (6)**: `buildCollapsedSpan`, `buildLineContent`, `buildLineElement`, `buildToken`, `buildTokenBadBidi`, `buildViewArray`.
- **function_expression (line 3001) / `compute_*` (2)**: `computeReplacedSel`, `computeSelAfterChange`.
- **function_expression (line 3001) / `get_*` (13)**: `getBetween`, `getBidiPartAt`, `getDimensions`, `getHandlers`, `getKeyMap`, `getLine`, `getLineContent`, `getLineStyles`, `getLines`, `getMarkedSpanFor`, `getOldSpans`, `getOrder`, `getStateBefore`.
- **function_expression (line 3001) / `handle_*` (3)**: `handleCharBinding`, `handleKeyBinding`, `handlePaste`.
- **function_expression (line 3001) / `has_*` (2)**; direct callee `removeChildrenAndAdd` (`function:922849`): `hasBadBidiRects`, `hasBadZoomedRects`.
- **function_expression (line 3001) / `set_*` (9)**: `setDocumentHeight`, `setGuttersForLineNumbers`, `setScrollLeft`, `setScrollTop`, `setSelection`, `setSelectionInner`, `setSelectionNoUndo`, `setSelectionReplaceHistory`, `setSimpleSelection`.
- **function_expression (line 4085) / `get_*` (2)**: `getHintElement`, `getText`.
- **function_expression (line 4204) / `get_*` (3)**: `getAttrRegexp`, `getAttrValue`, `getTagRegexp`.

### mautic — `app/bundles/LeadBundle/Entity/Lead.php`

- **Lead (line 17) / `get_*` (49)**: `getAddress1`, `getAddress2`, `getAttribution`, `getChannelRules`, `getCity`, `getColor`, `getCompany`, `getCompanyChangeLog`, `getCountry`, `getDateIdentified`, `getDefaultIdentifierFields`, `getDoNotContact`, `getEmail`, `getFirstSocialIdentity`, `getFirstname`, `getFrequencyRules`, `getId`, `getInternal`, `getIpAddresses`, `getLastActive`, `getLastname`, `getLeadPhoneNumber`, `getLocation`, `getManipulator`, `getMobile`, `getName`, `getNotes`, `getOwner`, `getPermissionUser`, `getPhone`, `getPointChanges`, `getPoints`, `getPointsChangeLog`, `getPosition`, `getPreferredLocale`, `getPreferredProfileImage`, `getPrimaryCompany`, `getPrimaryIdentifier`, `getPushIDs`, `getSecondaryIdentifier`, `getSocialCache`, `getStage`, `getStageChangeLog`, `getState`, `getTags`, `getTimezone`, `getTitle`, `getUtmTags`, `getZipcode`.
- **Lead (line 17) / `set_*` (34)**: `setActualPoints`, `setAddress1`, `setAddress2`, `setAvailableSocialFields`, `setChannelRules`, `setCity`, `setColor`, `setCompany`, `setCountry`, `setDateIdentified`, `setEmail`, `setFirstname`, `setFrequencyRules`, `setId`, `setInternal`, `setLastActive`, `setLastname`, `setManipulator`, `setMobile`, `setNewlyCreated`, `setOwner`, `setPhone`, `setPoints`, `setPosition`, `setPreferredProfileImage`, `setPrimaryCompany`, `setSocialCache`, `setStage`, `setState`, `setTags`, `setTimezone`, `setTitle`, `setUtmTags`, `setZipcode`.

### mautic — `app/bundles/EmailBundle/Entity/Email.php`

- **Email (line 33) / `get_*` (35)**: `getAssetAttachments`, `getBccAddress`, `getCategory`, `getClonedId`, `getContent`, `getCustomHtml`, `getDescription`, `getEmailType`, `getFromAddress`, `getFromName`, `getHeaders`, `getId`, `getLists`, `getName`, `getPendingCount`, `getPlainText`, `getPreferenceCenter`, `getPublicPreview`, `getPublishDown`, `getPublishUp`, `getQueuedCount`, `getReadCount`, `getReadPercentage`, `getReplyToAddress`, `getRevision`, `getSentCount`, `getSessionId`, `getStats`, `getSubject`, `getTemplate`, `getUnsubscribeForm`, `getUseOwnerAsMailer`, `getUtmTags`, `getVariantReadCount`, `getVariantSentCount`.
- **Email (line 33) / `set_*` (30)**: `setBccAddress`, `setCategory`, `setContent`, `setCustomHtml`, `setDescription`, `setEmailType`, `setFromAddress`, `setFromName`, `setHeaders`, `setLists`, `setName`, `setPendingCount`, `setPlainText`, `setPreferenceCenter`, `setPublicPreview`, `setPublishDown`, `setPublishUp`, `setQueuedCount`, `setReadCount`, `setReplyToAddress`, `setRevision`, `setSentCount`, `setSessionId`, `setSubject`, `setTemplate`, `setUnsubscribeForm`, `setUseOwnerAsMailer`, `setUtmTags`, `setVariantReadCount`, `setVariantSentCount`.

### mautic — `app/bundles/AssetBundle/Entity/Asset.php`

- **Asset (line 19) / `get_*` (36)**: `getAbsolutePath`, `getAbsoluteTempDir`, `getAbsoluteTempPath`, `getAlias`, `getCategory`, `getDescription`, `getDisallow`, `getDownloadCount`, `getDownloadUrl`, `getExtension`, `getFile`, `getFileContents`, `getFileExtensions`, `getFileInfo`, `getFileMimeType`, `getFilePath`, `getFileType`, `getIconClass`, `getId`, `getIniValue`, `getLanguage`, `getMaxSize`, `getMime`, `getOriginalFileName`, `getPath`, `getPublishDown`, `getPublishUp`, `getRemotePath`, `getRevision`, `getSize`, `getStorageLocation`, `getTempId`, `getTempName`, `getTitle`, `getUniqueDownloadCount`, `getUploadDir`.
- **Asset (line 19) / `is_*` (2)**; direct field `storageLocation` (`class_declaration:712:field:storageLocation`): `isLocal`, `isRemote`.
- **Asset (line 19) / `set_*` (26)**: `setAlias`, `setCategory`, `setDescription`, `setDisallow`, `setDownloadCount`, `setDownloadUrl`, `setExtension`, `setFile`, `setFileInfoFromFile`, `setFileNameFromRemote`, `setLanguage`, `setMaxSize`, `setMime`, `setOriginalFileName`, `setPath`, `setPublishDown`, `setPublishUp`, `setRemotePath`, `setRevision`, `setSize`, `setStorageLocation`, `setTempId`, `setTempName`, `setTitle`, `setUniqueDownloadCount`, `setUploadDir`.
