BOOKS := user operator tester showroom
BOOK ?= user

.PHONY: help docs docs-check docs-serve docs-landing docs-assets docs-gallery
help: ## List the available targets
	@awk 'BEGIN {FS = ":.*## "} /^[a-zA-Z_-]+:.*## / {printf "  %-12s %s\n", $$1, $$2}' $(MAKEFILE_LIST)

docs-assets: ## Copy diagram assets from the golden test outputs into the books
	./docs/sync-assets.sh

docs-gallery: ## Redraw the gallery pictures and copy their specs into the gallery pages
	./docs/regen-gallery.sh

docs-check: ## Check that guide links resolve and CLI/MCP references are current
	./docs/check.sh

docs: docs-check ## Build the three guide books into site/
	@for b in $(BOOKS); do mdbook build docs/guide/$$b; done
	cp docs/guide/index.html docs/guide/tldr.html site/
	@if [ -d docs/guide/assets ]; then cp -r docs/guide/assets site/assets; fi

PORT ?= 8000

docs-landing: docs ## Build the docs and serve the landing page locally (PORT=8000)
	@echo "Landing page: http://localhost:$(PORT)/  (Ctrl-C to stop)"
	@(sleep 1; open http://localhost:$(PORT)/ 2>/dev/null || xdg-open http://localhost:$(PORT)/ 2>/dev/null || true) &
	python3 -m http.server $(PORT) --directory site

docs-serve: ## Serve one book locally with live reload (BOOK=user|operator|tester|showroom)
	mdbook serve docs/guide/$(BOOK) --open

# Bump the version (BUMP=patch: 1.0.1 -> 1.0.2, minor: 1.1.0, major: 2.0.0), commit it
# and tag the commit vX.Y.Z. Nothing is pushed: run `git push origin master vX.Y.Z` to
# start the release.
CARGO ?= cargo
BUMP ?= patch

.PHONY: bump release-check
release-check: ## Check that the crate packages and publishes cleanly (dry run)
	$(CARGO) package --list --allow-dirty
	$(CARGO) publish --dry-run --allow-dirty

bump: ## Bump the version (BUMP=patch|minor|major), commit and tag vX.Y.Z (does not push)
	@set -e; \
	case "$(BUMP)" in patch|minor|major) ;; \
		*) echo "BUMP must be patch, minor or major (got '$(BUMP)')." >&2; exit 1;; esac; \
	if [ "$$(git branch --show-current)" != "master" ]; then \
		echo "Releases are cut from master; switch to it first." >&2; exit 1; \
	fi; \
	if [ -n "$$(git status --porcelain)" ]; then \
		echo "The working tree is not clean; commit or stash first." >&2; exit 1; \
	fi; \
	old=$$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1); \
	if ! echo "$$old" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$$'; then \
		echo "Cannot bump version '$$old' (expected X.Y.Z)." >&2; exit 1; \
	fi; \
	major=$${old%%.*}; rest=$${old#*.}; minor=$${rest%%.*}; patch=$${old##*.}; \
	case "$(BUMP)" in \
		major) new="$$(( major + 1 )).0.0";; \
		minor) new="$$major.$$(( minor + 1 )).0";; \
		patch) new="$$major.$$minor.$$(( patch + 1 ))";; \
	esac; \
	if git rev-parse -q --verify "refs/tags/v$$new" >/dev/null; then \
		echo "Tag v$$new already exists." >&2; exit 1; \
	fi; \
	echo "Bumping $$old -> $$new"; \
	sed -i.bak "1,/^version = /s/^version = \".*\"/version = \"$$new\"/" Cargo.toml; \
	rm Cargo.toml.bak; \
	jq --arg v "$$new" '.version = $$v | .packages[].version = $$v' server.json > server.json.tmp; \
	mv server.json.tmp server.json; \
	$(CARGO) update --workspace --quiet; \
	SVGDOG_UPDATE_GOLDEN=1 $(CARGO) test --locked --quiet --test cli r8_ >/dev/null; \
	$(CARGO) test --locked --quiet; \
	git commit -qam "Bump version to $$new"; \
	git tag "v$$new"; \
	echo "Committed and tagged v$$new. Publish with: git push origin $$(git branch --show-current) v$$new"
