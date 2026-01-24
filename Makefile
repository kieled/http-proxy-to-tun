# http-tun Makefile
# Convenience wrapper around tasks.py

.PHONY: help dev build test lint format clean install uninstall release

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

build-cli:
	@$(TASKS) build:cli

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

debug-collect:
	@$(TASKS) debug:collect
