.PHONY: install update build

install:
	pnpm install
	cargo build

update:
	pnpm update
	cargo update

build:
	pnpm build
	cargo build
	docker compose build

