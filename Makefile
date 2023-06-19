.PHONY: build start dev stop test mocks lint clickhouse

SHELL := /bin/bash

build:
	@docker run --rm -v $(PWD):/app -w /app rust:1.60.0 cargo b --release

start-docker:
	@docker build . -t geyser-plugin
	@docker run --rm -p 2000:2000 -it geyser-plugin