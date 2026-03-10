.PHONY: ui-install ui-dev ui-build ui-lint ui-typecheck

ui-install:
	cd web-ui && npm install

ui-dev:
	cd web-ui && npm run dev

ui-build:
	cd web-ui && npm run build

ui-lint:
	cd web-ui && npm run lint

ui-typecheck:
	cd web-ui && npm run typecheck
