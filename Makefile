.PHONY: build start dev stop test mocks lint clickhouse

SHELL := /bin/bash

build:
	@docker run --rm -v $(PWD):/app -w /app rust:1.60.0 cargo b --release
