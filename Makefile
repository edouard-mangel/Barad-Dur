# Barad-dûr — convenience targets
#
# Usage:
#   make analyze              analyze current directory → dashboard/report.json
#   make analyze TARGET=~/my-repo
#   make dashboard            start dashboard dev server
#   make report               analyze + start dashboard (open browser manually)
#   make report TARGET=~/my-repo
#   make build                release build of CLI
#   make install              install barad-dur to ~/.cargo/bin
#   make gate-coupling        fail if HEAD adds new coupling findings vs origin/main

TARGET      ?= .
OUTPUT      ?= dashboard/report.json
OUTPUT_HTML ?= report.html
BROWSER     ?= xdg-open

.PHONY: analyze dashboard report html-report report-smoke build install setup version-bump gate-coupling

analyze:
	cargo run --release -- analyze $(TARGET) --json > $(OUTPUT)
	@echo "Report written to $(OUTPUT)"

## Generate self-contained HTML report (TARGET=. OUTPUT_HTML=report.html)
html-report:
	cargo run --release -- analyze $(TARGET) --html -o $(OUTPUT_HTML)
	@echo "Report written to $(OUTPUT_HTML)"

## Smoke-test the HTML report tabs in jsdom (needs dashboard deps installed)
report-smoke:
	cargo run --release -- analyze $(TARGET) --html -o /tmp/barad-dur-smoke.html
	node scripts/report-smoke.mjs /tmp/barad-dur-smoke.html

dashboard:
	cd dashboard && pnpm run dev

report: analyze
	@echo "Opening dashboard…"
	cd dashboard && pnpm run dev &
	@sleep 2 && $(BROWSER) http://localhost:5173 2>/dev/null || true

build:
	cargo build --release

install:
	cargo install --path .

## Coupling ratchet vs origin/main — fails if HEAD adds new coupling findings
gate-coupling:
	cargo run --quiet -- gate . --min-score 0 --no-new-coupling --baseline-ref origin/main

setup:
	git config core.hooksPath hooks
	@echo "Git hooks configured (commit-msg + pre-push)."

version-bump:
	./scripts/version-bump.sh
