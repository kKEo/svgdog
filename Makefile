BOOKS := user operator tester
BOOK ?= user

.PHONY: help docs docs-check docs-serve docs-assets
help: ## List the available targets
	@awk 'BEGIN {FS = ":.*## "} /^[a-zA-Z_-]+:.*## / {printf "  %-12s %s\n", $$1, $$2}' $(MAKEFILE_LIST)

docs-assets: ## Copy diagram assets from the golden test outputs into the books
	./docs/sync-assets.sh

docs-check: ## Check that guide links resolve and CLI/MCP references are current
	./docs/check.sh

docs: docs-check ## Build the three guide books into site/
	@for b in $(BOOKS); do mdbook build docs/guide/$$b; done
	cp docs/guide/index.html docs/guide/tldr.html site/
	@if [ -d docs/guide/assets ]; then cp -r docs/guide/assets site/assets; fi

docs-serve: ## Serve one book locally with live reload (BOOK=user|operator|tester)
	mdbook serve docs/guide/$(BOOK) --open

# Bump the patch version (1.0.1 -> 1.0.2), commit it and tag the commit vX.Y.Z.
# Nothing is pushed: run `git push origin master vX.Y.Z` to start the release.
CARGO ?= cargo

.PHONY: bump
bump: ## Bump the patch version, commit and tag vX.Y.Z (does not push)
	@set -e; \
	if [ -n "$$(git status --porcelain)" ]; then \
		echo "The working tree is not clean; commit or stash first." >&2; exit 1; \
	fi; \
	old=$$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1); \
	if ! echo "$$old" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$$'; then \
		echo "Cannot bump version '$$old' (expected X.Y.Z)." >&2; exit 1; \
	fi; \
	new="$${old%.*}.$$(( $${old##*.} + 1 ))"; \
	if git rev-parse -q --verify "refs/tags/v$$new" >/dev/null; then \
		echo "Tag v$$new already exists." >&2; exit 1; \
	fi; \
	echo "Bumping $$old -> $$new"; \
	sed -i.bak "1,/^version = /s/^version = \".*\"/version = \"$$new\"/" Cargo.toml; \
	rm Cargo.toml.bak; \
	$(CARGO) update --workspace --quiet; \
	SVGDOG_UPDATE_GOLDEN=1 $(CARGO) test --locked --quiet --test cli r8_ >/dev/null; \
	$(CARGO) test --locked --quiet; \
	git commit -qam "Bump version to $$new"; \
	git tag "v$$new"; \
	echo "Committed and tagged v$$new. Publish with: git push origin $$(git branch --show-current) v$$new"
