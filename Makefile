# OmniEdge Cross-Platform Build System

# Variables
CLI_CRATE = omniedge
DESKTOP_DIR = ui/desktop
OUTPUT_DIR = bin

# Targets
LINUX_AMD64 = x86_64-unknown-linux-gnu
LINUX_ARM64 = aarch64-unknown-linux-gnu
MACOS_AMD64 = x86_64-apple-darwin
MACOS_ARM64 = aarch64-apple-darwin
WINDOWS_AMD64 = x86_64-pc-windows-msvc
WINDOWS_ARM64 = aarch64-pc-windows-msvc

.PHONY: all cli desktop clean

all: cli desktop

# --- CLI Builds ---
cli: cli-linux cli-macos cli-windows

cli-linux: cli-linux-amd64 cli-linux-arm64
cli-macos: cli-macos-amd64 cli-macos-arm64
cli-windows: cli-windows-amd64 cli-windows-arm64

cli-linux-amd64:
	cargo build --release -p $(CLI_CRATE) --target $(LINUX_AMD64)
cli-linux-arm64:
	cargo build --release -p $(CLI_CRATE) --target $(LINUX_ARM64)

cli-macos-amd64:
	cargo build --release -p $(CLI_CRATE) --target $(MACOS_AMD64)
cli-macos-arm64:
	cargo build --release -p $(CLI_CRATE) --target $(MACOS_ARM64)

cli-windows-amd64:
	cargo build --release -p $(CLI_CRATE) --target $(WINDOWS_AMD64)
cli-windows-arm64:
	cargo build --release -p $(CLI_CRATE) --target $(WINDOWS_ARM64)

# --- Desktop Builds ---
desktop: desktop-linux desktop-macos desktop-windows

desktop-linux: desktop-linux-amd64 desktop-linux-arm64
desktop-macos: desktop-macos-amd64 desktop-macos-arm64
desktop-windows: desktop-windows-amd64 desktop-windows-arm64

desktop-linux-amd64:
	cd $(DESKTOP_DIR) && npm run tauri build -- --target $(LINUX_AMD64)
desktop-linux-arm64:
	cd $(DESKTOP_DIR) && npm run tauri build -- --target $(LINUX_ARM64)

desktop-macos-amd64:
	cd $(DESKTOP_DIR) && npm run tauri build -- --target $(MACOS_AMD64)
desktop-macos-arm64:
	cd $(DESKTOP_DIR) && npm run tauri build -- --target $(MACOS_ARM64)

desktop-windows-amd64:
	cd $(DESKTOP_DIR) && npm run tauri build -- --target $(WINDOWS_AMD64)
desktop-windows-arm64:
	cd $(DESKTOP_DIR) && npm run tauri build -- --target $(WINDOWS_ARM64)

clean:
	cargo clean
	rm -rf $(DESKTOP_DIR)/dist
	rm -rf $(DESKTOP_DIR)/src-tauri/target
