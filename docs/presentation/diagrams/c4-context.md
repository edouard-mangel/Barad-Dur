# C4 Context Diagram — Barad-dûr

Barad-dûr in relation to the people and external systems it interacts with.
A developer runs the tool locally or wires it into a CI/CD pipeline; the tool
reaches out to package registries and the OSV vulnerability database on demand,
and can optionally enrich remote-URL analyses via the GitHub API.

```mermaid
C4Context
    title System Context — Barad-dûr Repository Analyzer

    Person(developer, "Developer", "Runs barad-dur locally or in CI/CD to assess repository health")
    Person(team, "Engineering Team", "Reviews HTML reports and dashboard in a browser")

    System(baradur, "Barad-dûr", "CLI tool that analyzes git metadata and source code to produce scored health reports across 5 categories: Health, Coupling, Evolution, Git Hygiene, and Team")

    System_Ext(gitrepo, "Git Repository", "Local or remote git repository — commits, blame, file tree, tags")
    System_Ext(cicd, "CI/CD System", "GitLab CI / GitHub Actions pipeline that runs 'barad-dur gate' as a quality gate")
    System_Ext(crates, "crates.io", "Rust package registry — provides publish dates for Cargo dependencies")
    System_Ext(npm, "npm Registry", "JavaScript package registry — provides publish dates for npm dependencies")
    System_Ext(pypi, "PyPI", "Python package registry — provides publish dates for pip dependencies")
    System_Ext(nuget, "NuGet Gallery", "C# package registry — provides publish dates for NuGet dependencies")
    System_Ext(osv, "OSV Database", "Open Source Vulnerability database — provides CVE data for all ecosystems")
    System_Ext(github, "GitHub API", "Optional enrichment: stars, open issues, primary language for remote repos")

    Rel(developer, baradur, "Runs analyze / gate / coupling / backfill / watch / init / contributors", "CLI")
    Rel(team, baradur, "Views HTML report or React dashboard", "Browser")
    Rel(cicd, baradur, "Executes as a pipeline job", "CLI / exit code")

    Rel(baradur, gitrepo, "Reads commits, blame lines, file tree via git2 + git CLI", "libgit2 / subprocess")
    Rel(baradur, crates, "Fetches package publish dates (cached 7 days)", "HTTPS")
    Rel(baradur, npm, "Fetches package publish dates (cached 7 days)", "HTTPS")
    Rel(baradur, pypi, "Fetches package publish dates (cached 7 days)", "HTTPS")
    Rel(baradur, nuget, "Fetches package publish dates (cached 7 days)", "HTTPS")
    Rel(baradur, osv, "Fetches CVE advisories per dependency (cached 7 days)", "HTTPS")
    Rel(baradur, github, "Fetches repo metadata when --token supplied and URL is github.com", "HTTPS / REST API")

    UpdateLayoutConfig($c4ShapeInRow="3", $c4BoundaryInRow="1")
```
