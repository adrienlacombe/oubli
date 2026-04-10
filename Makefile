.DEFAULT_GOAL := help

.PHONY: help env-status test test-offline test-smoke test-integration test-sepolia test-mainnet test-devnet test-android-unit test-ios-ui check-rust check-swap audit-rust setup-rust-coverage coverage-rust coverage-rust-ci coverage-android-unit coverage-android-unit-ci build-ios build-ios-sim build-android generate-swift generate-kotlin regen-swift regen-kotlin regen-bindings setup-ios setup-android build-swap-js regen-all verify-swap-bundle verify-swift-bindings verify-kotlin-bindings clean

OUBLI_ENV_FILE ?= .sepolia.env
COVERAGE_RUST_DIR := target/coverage/rust
COVERAGE_RUST_HTML_DIR := $(COVERAGE_RUST_DIR)/html
COVERAGE_RUST_LCOV := $(COVERAGE_RUST_DIR)/lcov.info
COVERAGE_RUST_SUMMARY := $(COVERAGE_RUST_DIR)/summary.txt
COVERAGE_ANDROID_UNIT_DIR := target/coverage/android-unit
COVERAGE_ANDROID_UNIT_HTML_DIR := $(COVERAGE_ANDROID_UNIT_DIR)/html
COVERAGE_ANDROID_UNIT_XML := $(COVERAGE_ANDROID_UNIT_DIR)/report.xml
COVERAGE_ANDROID_UNIT_SUMMARY := $(COVERAGE_ANDROID_UNIT_DIR)/summary.txt

# Prefer a safe default for local work. An already-exported shell environment
# wins over the Makefile-selected file.
ifneq ($(origin OUBLI_RPC_URL), undefined)
  ACTIVE_ENV_SOURCE := environment
else ifneq (,$(wildcard $(OUBLI_ENV_FILE)))
  include $(OUBLI_ENV_FILE)
  export
  ACTIVE_ENV_SOURCE := $(OUBLI_ENV_FILE)
else
  ACTIVE_ENV_SOURCE := none
endif

define assert_mainnet_opt_in
	@if [ "$${OUBLI_CHAIN_ID:-}" = "SN_MAIN" ] && [ "$${OUBLI_ALLOW_MAINNET:-}" != "1" ]; then \
		echo "Refusing to run a mainnet workflow without OUBLI_ALLOW_MAINNET=1."; \
		echo "Use: make OUBLI_ENV_FILE=.mainnet.env OUBLI_ALLOW_MAINNET=1 test-mainnet"; \
		exit 1; \
	fi
endef

help:
	@echo "Common targets:"
	@echo "  make env-status              Show the active env source and selected network"
	@echo "  make test-offline            Run fast offline Rust unit tests"
	@echo "  make test-smoke              Run the wallet mock smoke tests"
	@echo "  make test-android-unit       Run Android JVM unit tests"
	@echo "  make test-ios-ui             Run iOS UI tests on an available iPhone simulator"
	@echo "  make check-rust              Run rustfmt check + offline Rust tests"
	@echo "  make audit-rust              Run cargo audit on the Rust dependency tree"
	@echo "  make coverage-rust           Generate Rust coverage summary + HTML + lcov"
	@echo "  make coverage-android-unit   Generate Android JVM coverage summary + HTML + XML"
	@echo "  make check-swap              Type-check oubli-swap-js"
	@echo "  make build-swap-js           Rebuild crates/oubli-swap/js/bundle.js"
	@echo "  make regen-swift             Rebuild iOS sim lib and regenerate Swift bindings"
	@echo "  make regen-kotlin            Rebuild Android lib and regenerate Kotlin bindings"
	@echo "  make regen-bindings          Regenerate both Swift and Kotlin bindings"
	@echo "  make regen-all               Rebuild swap bundle and both mobile bindings"
	@echo "  make test-sepolia            Run ignored Sepolia integration tests"
	@echo "  make OUBLI_ENV_FILE=.mainnet.env OUBLI_ALLOW_MAINNET=1 test-mainnet"
	@echo ""
	@echo "Safe default: Make auto-loads .sepolia.env when present."

env-status:
	@echo "Env source: $(ACTIVE_ENV_SOURCE)"
	@echo "Chain: $${OUBLI_CHAIN_ID:-unset}"
	@echo "RPC URL set: $$(if [ -n "$${OUBLI_RPC_URL:-}" ]; then echo yes; else echo no; fi)"
	@echo "Mainnet opt-in: $${OUBLI_ALLOW_MAINNET:-0}"

# ── Testing ──────────────────────────────────────────────────

test:
	@$(MAKE) test-offline

test-offline:
	cargo test --workspace --lib

test-smoke:
	cargo test -p oubli-wallet --test integration test_full_lifecycle_mock
	cargo test -p oubli-wallet --test integration test_fund_requires_t2

test-integration:
	@$(MAKE) test-sepolia

test-sepolia:
	@if [ "$${OUBLI_CHAIN_ID:-}" != "SN_SEPOLIA" ]; then \
		echo "Sepolia tests require SN_SEPOLIA. Use .sepolia.env or set OUBLI_CHAIN_ID=SN_SEPOLIA."; \
		exit 1; \
	fi
	cargo test -p oubli-wallet --test integration
	cargo test -p oubli-wallet --test integration -- --ignored --nocapture
	cargo test -p oubli-wallet --test sepolia_full_flow -- --ignored --nocapture

test-mainnet:
	$(call assert_mainnet_opt_in)
	@if [ "$${OUBLI_CHAIN_ID:-}" != "SN_MAIN" ]; then \
		echo "Mainnet tests require SN_MAIN. Use OUBLI_ENV_FILE=.mainnet.env."; \
		exit 1; \
	fi
	cargo test -p oubli-wallet --test mainnet_full_flow -- --ignored --nocapture

test-devnet:
	cargo test -p oubli-wallet --features devnet --test devnet_integration -- --nocapture --test-threads=1

test-android-unit:
	cd android && ./gradlew testDebugUnitTest

test-ios-ui: build-ios-sim
	cd ios && xcodegen generate
	cd ios && DEVICE_NAME="$$(xcrun simctl list devices available -j | python3 -c 'import json,sys; devices=json.load(sys.stdin)["devices"]; name=next((d["name"] for r in sorted(devices, reverse=True) for d in devices[r] if d.get("isAvailable") and d.get("name", "").startswith("iPhone")), ""); name or sys.exit("No available iPhone simulator found"); print(name)')" && \
		xcodebuild test -project Oubli.xcodeproj -scheme Oubli -destination "platform=iOS Simulator,name=$$DEVICE_NAME"

check-rust:
	cargo fmt --check
	cargo test --workspace --lib

audit-rust:
	cargo audit

setup-rust-coverage:
	cargo llvm-cov --version
	rustup component add llvm-tools-preview

coverage-rust: setup-rust-coverage
	rm -rf $(COVERAGE_RUST_DIR)
	mkdir -p $(COVERAGE_RUST_DIR)
	cargo llvm-cov clean --workspace
	cargo llvm-cov --workspace --lib --no-report
	cargo llvm-cov report | tee $(COVERAGE_RUST_SUMMARY)
	cargo llvm-cov report --html --output-dir $(COVERAGE_RUST_DIR)
	cargo llvm-cov report --lcov --output-path $(COVERAGE_RUST_LCOV)
	@echo ""
	@echo "Rust coverage outputs:"
	@echo "  Summary: $(COVERAGE_RUST_SUMMARY)"
	@echo "  HTML:    $(COVERAGE_RUST_HTML_DIR)/index.html"
	@echo "  LCOV:    $(COVERAGE_RUST_LCOV)"

coverage-rust-ci: coverage-rust

coverage-android-unit:
	rm -rf $(COVERAGE_ANDROID_UNIT_DIR)
	mkdir -p $(COVERAGE_ANDROID_UNIT_DIR)
	cd android && ./gradlew jacocoDebugUnitTestReport
	cp android/app/build/reports/jacoco/jacocoDebugUnitTestReport/jacocoDebugUnitTestReport.xml $(COVERAGE_ANDROID_UNIT_XML)
	cp -R android/app/build/reports/jacoco/jacocoDebugUnitTestReport/html $(COVERAGE_ANDROID_UNIT_HTML_DIR)
	COVERAGE_XML="$(COVERAGE_ANDROID_UNIT_XML)" COVERAGE_SUMMARY="$(COVERAGE_ANDROID_UNIT_SUMMARY)" python3 -c 'import os, xml.etree.ElementTree as ET; from pathlib import Path; root = ET.parse(os.environ["COVERAGE_XML"]).getroot(); counters = {c.attrib["type"]: (int(c.attrib["missed"]), int(c.attrib["covered"])) for c in root.findall("counter")}; lines = ["Android JVM coverage summary (debug unit tests)"] + [f"{name.title():<11} covered={counters[name][1]:<6} missed={counters[name][0]:<6} total={counters[name][0] + counters[name][1]:<6} pct={(0.0 if counters[name][0] + counters[name][1] == 0 else (counters[name][1] / (counters[name][0] + counters[name][1]) * 100)):6.2f}%" for name in ("INSTRUCTION", "LINE", "BRANCH", "METHOD", "CLASS")]; summary = "\n".join(lines) + "\n"; Path(os.environ["COVERAGE_SUMMARY"]).write_text(summary); print(summary, end="")'
	@echo ""
	@echo "Android coverage outputs:"
	@echo "  Summary: $(COVERAGE_ANDROID_UNIT_SUMMARY)"
	@echo "  HTML:    $(COVERAGE_ANDROID_UNIT_HTML_DIR)/index.html"
	@echo "  XML:     $(COVERAGE_ANDROID_UNIT_XML)"

coverage-android-unit-ci: coverage-android-unit

check-swap:
	cd oubli-swap-js && npm ci && npm run check

# ── iOS ──────────────────────────────────────────────────────

build-ios:
	cargo build --release --target aarch64-apple-ios -p oubli-bridge

build-ios-sim:
	BINDGEN_EXTRA_CLANG_ARGS="--sysroot=$$(xcrun --sdk iphonesimulator --show-sdk-path) --target=arm64-apple-ios-simulator" \
		cargo build --target aarch64-apple-ios-sim -p oubli-bridge

generate-swift:
	mkdir -p ios/Generated/oubliFFI
	cargo run -p oubli-bridge --bin uniffi-bindgen -- generate \
		--library target/aarch64-apple-ios-sim/debug/liboubli_bridge.a \
		--language swift --out-dir ios/Generated
	mv ios/Generated/oubliFFI.h ios/Generated/oubliFFI/
	mv ios/Generated/oubliFFI.modulemap ios/Generated/oubliFFI/module.modulemap

regen-swift: build-ios-sim generate-swift

setup-ios: build-ios-sim generate-swift
	cd ios && xcodegen generate
	@echo ""
	@echo "✓ iOS project ready. Open ios/Oubli.xcodeproj in Xcode."

# ── Android ──────────────────────────────────────────────────

build-android:
	ANDROID_NDK_HOME=$${ANDROID_NDK_HOME:-$$HOME/Library/Android/sdk/ndk/28.2.13676358} \
	BINDGEN_EXTRA_CLANG_ARGS="--sysroot=$${ANDROID_NDK_HOME:-$$HOME/Library/Android/sdk/ndk/28.2.13676358}/toolchains/llvm/prebuilt/darwin-x86_64/sysroot --target=aarch64-linux-android26" \
		cargo ndk -t arm64-v8a build --release -p oubli-bridge
	mkdir -p android/app/src/main/jniLibs/arm64-v8a
	cp target/aarch64-linux-android/release/liboubli_bridge.so android/app/src/main/jniLibs/arm64-v8a/

generate-kotlin:
	cargo run -p oubli-bridge --bin uniffi-bindgen -- generate \
		--library target/aarch64-linux-android/release/liboubli_bridge.so \
		--language kotlin --out-dir android/app/src/main/java/

regen-kotlin: build-android generate-kotlin

regen-bindings: regen-swift regen-kotlin

setup-android: build-android generate-kotlin
	@echo ""
	@echo "✓ Android native library and Kotlin bindings updated."

# ── Swap JS Bundle ──────────────────────────────────────────

build-swap-js:
	cd oubli-swap-js && npm ci && npm run build

regen-all: build-swap-js regen-bindings

verify-swap-bundle: build-swap-js
	git diff --exit-code -- crates/oubli-swap/js/bundle.js

verify-swift-bindings: regen-swift
	git diff --exit-code -- ios/Generated/oubli.swift ios/Generated/oubliFFI/oubliFFI.h ios/Generated/oubliFFI/module.modulemap

verify-kotlin-bindings: regen-kotlin
	git diff --exit-code -- android/app/src/main/java/uniffi/oubli/oubli.kt

# ── Clean ────────────────────────────────────────────────────

clean:
	cargo clean
