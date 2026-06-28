Feature: Manifest files excluded from hotspot/churn surfaces by default
  As an engineer reviewing barad-dûr hotspots
  I want manifest files left out of the ranked file analysis
  So that the hotspots reflect real code, not declarative config churn

  Background:
    Given a repository containing source files and core-ecosystem manifests
    And default excludes are enabled

  Scenario: Manifest is dropped from the snapshot
    When the repository is analyzed
    Then "package.json" is not present in the analyzed file set
    And "Cargo.toml" is not present in the analyzed file set
    And source files remain in the analyzed file set

  Scenario: Manifest does not appear in hotspots
    When the repository is analyzed
    And the hotspots are rendered
    Then no manifest file appears in the hotspot ranking

  Scenario: Nested manifests in a monorepo are also excluded
    Given a manifest at "apps/web/package.json"
    When the repository is analyzed
    Then "apps/web/package.json" is not present in the analyzed file set

  Scenario: Defaults disabled re-includes manifests
    Given default excludes are disabled
    When the repository is analyzed
    Then "package.json" is present in the analyzed file set

  Scenario: Dependency and CVE features are unaffected
    Given the deps category and dependency coupling are enabled
    When the repository is analyzed
    Then the deps category output is unchanged from before the exclusion
    And dependency-based coupling pairs are unchanged from before the exclusion
