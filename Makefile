# http-tun Makefile
# Convenience wrapper around tasks.py

.PHONY: help dev build build-upx test lint format clean install uninstall release release-upx setup-caps set-caps build-gui

PYTHON := python3
TASKS := $(PYTHON) tasks.py

help:
	@$(TASKS) --help

# Development
dev:
	@$(TASKS) dev

# Build
build:
	@$(TASKS) build

build-upx:
	@$(TASKS) build --upx

build-cli:
	@$(TASKS) build:cli

build-cli-upx:
	@$(TASKS) build:cli --upx

# Testing
test:
	@$(TASKS) test

test-privileged:
	@$(TASKS) test:privileged

test-e2e:
	@$(TASKS) test:e2e

# Code quality
lint:
	@$(TASKS) lint

format:
	@$(TASKS) format

# Installation
install:
	@$(TASKS) install

install-deps:
	@$(TASKS) install:deps

uninstall:
	@$(TASKS) uninstall

deps-ui:
	@$(TASKS) deps:ui

# Maintenance
clean:
	@$(TASKS) clean

release:
	@$(TASKS) release

release-upx:
	@$(TASKS) release --upx

debug-collect:
	@$(TASKS) debug:collect

# GUI with capabilities
build-gui:
	cargo build -p proxyvpn-iced --release
	@./scripts/set-caps.sh release

# One-time setup for capability permissions (requires sudo)
setup-caps:
	sudo ./scripts/setup-capabilities.sh

# Set capabilities on existing binaries
set-caps:
	@./scripts/set-caps.sh release
