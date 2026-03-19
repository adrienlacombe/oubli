.PHONY: test test-integration test-devnet build-ios build-ios-sim build-android generate-swift generate-kotlin setup-ios build-swap-js clean

# Load secrets from .env if it exists
ifneq (,$(wildcard .env))
include .env
export
endif

# ── Testing ──────────────────────────────────────────────────

test:
	cargo test --workspace

test-integration:
	cargo test -p oubli-wallet --test integration

test-devnet:
	cargo test -p oubli-wallet --features devnet --test devnet_integration -- --nocapture --test-threads=1

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

setup-android: build-android generate-kotlin
	@echo ""
	@echo "✓ Android native library and Kotlin bindings updated."

# ── Swap JS Bundle ──────────────────────────────────────────

build-swap-js:
	cd oubli-swap-js && npm install && npm run build

# ── Clean ────────────────────────────────────────────────────

clean:
	cargo clean
