.DEFAULT_GOAL := help
SHELL := /bin/bash

DATA_DIR := wow-data
DATA     := $(DATA_DIR)/Data
WASM     := web/dist/wenilla_bg.wasm

.PHONY: help build start native fetch-data toolchain wasm android-toolchain android android-data clean

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## ' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-14s\033[0m %s\n", $$1, $$2}'

fetch-data: ## Download the game data (a WoW 1.12.1 client) from archive.org
	./scripts/fetch-data.sh

toolchain: ## Install the wasm toolchain (wasi-sdk, wasm-bindgen, binaryen)
	./scripts/web-setup.sh

wasm: ## Build the browser client into web/dist/
	./scripts/web-build.sh

build: fetch-data toolchain wasm ## Fetch data + install toolchain + build the wasm

start: ## Serve the game at http://127.0.0.1:8090 (run `make build` first)
	@test -f $(WASM) || { echo "  not built yet — run: make build"; exit 1; }
	cargo run --release -p wenilla-host -- --www web/dist --data $(DATA)

native: fetch-data ## Build + run the native desktop version
	WOW_DATA=$(DATA) cargo run --release -p warcraft-survivors

android-toolchain: ## Install the Android toolchain (rustup target, cargo-ndk; needs ANDROID_NDK_HOME)
	./scripts/android-setup.sh

android: ## Build the Android APK into android/app/build/outputs/apk/ (run `make android-toolchain` first)
	./scripts/android-build.sh

android-data: ## Fetch the game data here and adb push it to the connected phone (app installed first)
	./scripts/android-data.sh

clean: ## Remove the fetched game data and wasm build output
	rm -rf web/dist $(DATA_DIR)
