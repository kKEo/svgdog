BOOKS := user operator tester
BOOK ?= user

.PHONY: docs docs-check docs-serve docs-assets
docs-assets:
	./docs/sync-assets.sh

docs-check:
	./docs/check.sh

docs: docs-check
	@for b in $(BOOKS); do mdbook build docs/guide/$$b; done
	cp docs/guide/index.html docs/guide/tldr.html site/
	@if [ -d docs/guide/assets ]; then cp -r docs/guide/assets site/assets; fi

docs-serve:
	mdbook serve docs/guide/$(BOOK) --open
