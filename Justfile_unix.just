dist_bin_dir := "dist/bin"
dist_applets_dir := "dist/applets"
applets_source_dir := "applets"
applets_dist_dir := "runtime-web/public/applets"
applets := "analog-clock drawing-game graph dot-drawing launcher settings find-dot mirror cross-memory memo tetris audio-test tactile-display-demo network-test weather tactile-game tactile-test braille-test charter-of-national-education device-manual rhythm-game"
cross_target_dir := "target-cross"

export CMAKE_POLICY_VERSION_MINIMUM := "3.5"

# Show available recipes
default:
    @just --list

# ==========================================
# 🔍 Check Environments
# ==========================================

# Check for cargo installation
_check-rust:
    @if ! command -v cargo > /dev/null; then \
        echo "❌ cargo is not installed."; \
        echo "Please install Rust and Cargo: https://rustup.rs/"; \
        exit 1; \
    fi

# Zig 설치 여부 확인
_check-zig:
    @command -v zig >/dev/null 2>&1 || { echo "❌ 'zig' is not installed. Please install Zig: https://ziglang.org/"; exit 1; }

# Add wasm32-unknown-unknown target
_add-wasm-target: _check-rust
    rustup target add wasm32-unknown-unknown

# Check for npx installation
_check-nodejs:
    @if ! command -v npx > /dev/null; then \
        echo "❌ npx is not installed."; \
        echo "Please install Node.js and npm: https://nodejs.org/"; \
        exit 1; \
    fi

# Install watchexec-cli if not installed
_install-trunk: _check-rust
    @if ! command -v trunk > /dev/null; then \
        echo "trunk not found. Installing trunk..."; \
        cargo install trunk --locked; \
    fi

# Install watchexec-cli if not installed
_install-watchexec: _check-rust
    @if ! command -v watchexec > /dev/null; then \
        echo "watchexec not found. Installing watchexec-cli..."; \
        cargo install watchexec-cli; \
    fi

# Install leptosfmt if not installed
_install-leptosfmt: _check-rust
    @if ! command -v leptosfmt > /dev/null; then \
        echo "leptosfmt not found. Installing leptosfmt..."; \
        cargo install leptosfmt; \
    fi

_install-wasm-tools: _check-rust
    @if ! command -v wasm-tools > /dev/null; then \
        echo "wasm-tools not found. Installing wasm-tools..."; \
        cargo install wasm-tools --locked; \
    fi

_install-espup: _check-rust
    @if ! command -v espup > /dev/null; then \
        echo "espup not found. Installing espup..."; \
        cargo install espup; \
    fi

_install-cross: _check-rust
    @if ! command -v cross > /dev/null; then \
        echo "cross not found. Installing cross..."; \
        cargo install cross --git https://github.com/cross-rs/cross; \
    fi

_install-cargo-zigbuild: _check-rust
    @if ! command -v cargo-zigbuild > /dev/null; then \
        echo "cargo-zigbuild not found. Starting installation..."; \
        cargo install cargo-zigbuild; \
    fi

_install-jaq: _check-rust
    @if ! command -v jaq > /dev/null; then \
        echo "jaq not found. Installing jaq..."; \
        cargo install jaq; \
    fi

# ==========================================
# 🔍 Check code quality
# ==========================================

# Format the code
[group('🔍 Check code quality')]
fmt: _check-rust _check-nodejs _install-leptosfmt
    cargo fmt --all
    leptosfmt web/src/**/*.rs
    npx -y prettier --write "**/*.md"

# Lint all packages
[group('🔍 Check code quality')]
clippy: _check-rust
    cargo clippy --workspace -- --deny warnings

# Alias for clippy
[group('🔍 Check code quality')]
lint: clippy

# Check all packages
[group('🔍 Check code quality')]
check: _check-rust fmt lint
    cargo check --workspace --exclude capture

# ==========================================
# 📄 Documentation
# ==========================================

# Generate and open rustdoc for all crates
[group('📄 Documentation')]
doc: _check-rust
    cargo doc --workspace --open

# ==========================================
# ⚙️ Applets & Runtime Development
# ==========================================

# Watch applets folder and rebuild on changes
[group('⚙️ Applets & Runtime Development')]
watch-applets: _install-watchexec
    watchexec --debounce 500ms --watch applets --exts rs -- just build-applets

# Build the Native runtime (Debug)
[group('⚙️ Applets & Runtime Development')]
build-runtime-native-debug: _check-rust
    cargo build --package runtime-native

# Run the runtime with console (Debug)
[group('⚙️ Applets & Runtime Development')]
run-applet *ARGS: build-runtime-native-debug
    RUST_LOG=info,runtime_native=debug cargo run --package runtime-native -- {{ARGS}}

# Watch runtime folder and rebuild on changes
[group('⚙️ Applets & Runtime Development')]
watch-runtime-native: _install-watchexec
    watchexec --watch runtime-native --exts rs -- just build-runtime-native-debug

# Watch and serve the Web runtime
[group('⚙️ Applets & Runtime Development')]
serve-runtime-web *ARGS: _install-trunk build-applets
    cd runtime-web && trunk serve {{ARGS}}

alias run-web := serve-runtime-web

# Rebuild a single applet and serve the Web runtime.
# Other applets are reused from the existing dist/applets output.
[group('⚙️ Applets & Runtime Development')]
serve-runtime-web-applet applet *ARGS: _install-trunk (build-applet applet) refresh-applets
    cd runtime-web && trunk serve {{ARGS}}

alias run-web-applet := serve-runtime-web-applet

# Watch and serve the TTS service
[group('⚙️ Applets & Runtime Development')]
serve-audio-service: _check-rust
    TTS_ASSET_DIR=./assets RUST_LOG=info,audio_service=debug,ort=warn cargo watch --workdir audio-service -w src -w Cargo.toml -x "run -p audio-service"

# Run capture app
[group('⚙️ PC Apps Development')]
run-capture *ARGS: _check-rust
    cargo run --package capture --release -- {{ARGS}}

# ==========================================
# ⚙️ Firmware Development
# ==========================================

# Run firmware
[group('⚙️ Firmware Development')]
run-firmware: _install-espup
    . ~/export-esp.sh && cd firmware && cargo run --release

# ==========================================
# ⚙️ Build (Release)
# ==========================================

# Build the app wasm modules (Release)
[group('⚙️ Build (Release)')]
build-applet applet: _add-wasm-target _install-wasm-tools
    @cargo build -p {{applet}} --target wasm32-unknown-unknown --release && \
    mkdir -p {{dist_applets_dir}} && \
    applet_name_orig="$(cargo run -q -p applet-metadata-tool -- {{applets_source_dir}}/{{applet}} --name)" && \
    applet_name="$(echo ${applet_name_orig} | sed 's/-/_/g')" && \
    cargo run -q -p applet-metadata-tool -- {{applets_source_dir}}/{{applet}} \
        --embed "target/wasm32-unknown-unknown/release/${applet_name}.wasm" \
        -o "{{dist_applets_dir}}/${applet_name_orig}.wasm" && \
    case "$(uname -s)" in \
        Darwin*) os_dir="$HOME/Library/Application Support/com.moredream.runtime/applets" ;; \
        Linux*)  os_dir="$HOME/.local/share/runtime/applets" ;; \
        *)       os_dir="$APPDATA/moredream/runtime/data/applets" ;; \
    esac && \
    mkdir -p "$os_dir" && \
    cp "{{dist_applets_dir}}/${applet_name_orig}.wasm" "$os_dir/" || true && \
    echo "✅ Copied ${applet_name_orig}.wasm to $os_dir"


# Build all applets (Release)
[group('⚙️ Build (Release)')]
build-applets:
    @for applet in {{applets}}; do just build-applet $applet; done && just refresh-applets

[group('⚙️ Build (Release)')]
refresh-applets: _install-jaq
    @rm -rf "{{applets_dist_dir}}_tmp"
    @mkdir -p "{{applets_dist_dir}}_tmp"
    @cp -fv {{dist_applets_dir}}/*.wasm "{{applets_dist_dir}}_tmp/"
    @(ls -1 "{{applets_dist_dir}}_tmp"/*.wasm 2>/dev/null || true) | sed -e 's/.*\///' -e 's/\.wasm$//' | jaq -R -s 'split("\n") | map(select(length > 0))' > "{{applets_dist_dir}}_tmp/applets.json"
    @rm -rf "{{applets_dist_dir}}"
    @mv "{{applets_dist_dir}}_tmp" "{{applets_dist_dir}}"
    @echo "✅ Generated {{applets_dist_dir}}/applets.json"

# 네이티브 런타임 빌드 (릴리즈)
[group('⚙️ Build (Release)')]
build-runtime-native: _check-rust
    cargo build --package runtime-native --release

# 네이티브 런타임 빌드 (릴리즈, aarch64 크로스 컴파일)
[group('⚙️ Build (Release)')]
build-runtime-native-aarch64: _check-rust _check-zig
    ulimit -n 65536 2>/dev/null || true; \
    cargo zigbuild --package runtime-native --release --target aarch64-unknown-linux-gnu && \
    mkdir -p "{{dist_bin_dir}}" && \
    cp -fv target/aarch64-unknown-linux-gnu/release/runtime-native "{{dist_bin_dir}}/multiline-braille-display-runtime"

# Build the Web runtime (Release)
[group('⚙️ Build (Release)')]
build-runtime-web: _install-trunk
    cd runtime-web && trunk build

# Build the Audio service (Release)
[group('⚙️ Build (Release)')]
build-audio-service: _check-rust
    cargo build --package audio-service --release

# Build the Audio service (Release, aarch64)
[group('⚙️ Build (Release)')]
build-audio-service-aarch64: _check-rust _install-cross
    ulimit -n 65536 2>/dev/null || true; \
    XDG_CACHE_HOME=/target/cache \
    CARGO_TARGET_DIR={{cross_target_dir}} \
    cross build --package audio-service --release --target aarch64-unknown-linux-gnu

# Build capture app
[group('⚙️ Build (Release)')]
build-capture: _check-rust
    cargo build --package capture --release

# Build the firmware
[group('⚙️ Build (Release)')]
build-firmware: _check-rust
    . ~/export-esp.sh && cd firmware && cargo build --release

# Build everything (Release)
[group('⚙️ Build (Release)')]
build: build-applets build-runtime-native build-runtime-web build-audio-service

[group('⚙️ Build (Release)')]
dist-build: build-applets build-runtime-native-aarch64 build-audio-service-aarch64

[group('⚙️ Deploy (Release)')]
deploy: build-applets build-runtime-native-aarch64 build-audio-service-aarch64
    ulimit -n 65536 2>/dev/null || true
    . device-env.sh && \
    ssh $DEVICE_HOST "mkdir -p ~/bin ~/.local/share/runtime/applets ~/audio-service/assets" && \
    ssh $DEVICE_HOST "systemctl --user stop runtime-native.service; systemctl --user stop audio-service.service" && \
    scp target/aarch64-unknown-linux-gnu/release/runtime-native {{cross_target_dir}}/aarch64-unknown-linux-gnu/release/audio-service $DEVICE_HOST:bin/ && \
    scp -r {{dist_applets_dir}}/*.wasm $DEVICE_HOST:.local/share/runtime/applets && \
    ssh $DEVICE_HOST "systemctl --user start audio-service.service; systemctl --user start runtime-native.service"

[group('⚙️ Deploy (Release)')]
deploy-services:
    . device-env.sh && \
    scp cm5-overlays/home/muon/.config/systemd/user/{audio-service,runtime-native}.service $DEVICE_HOST:.config/systemd/user/ && \
    ssh $DEVICE_HOST "systemctl --user daemon-reload && systemctl --user enable runtime-native.service audio-service.service"

[group('⚙️ Deploy (Release)')]
deploy-applets: build-applets
    . device-env.sh && \
    ssh $DEVICE_HOST "mkdir -p ~/.local/share/runtime/applets" && \
    scp -r {{dist_applets_dir}}/*.wasm $DEVICE_HOST:.local/share/runtime/applets

[group('⚙️ Deploy (Release)')]
deploy-assets:
    . device-env.sh && \
    ssh $DEVICE_HOST "mkdir -p ~/audio-service/assets" && \
    scp -r audio-service/assets/* $DEVICE_HOST:audio-service/assets/

[group('⚙️ Deploy (Release)')]
debug-audio: build-audio-service-aarch64
    . device-env.sh && \
    ssh $DEVICE_HOST "systemctl --user stop audio-service.service" && \
    scp {{cross_target_dir}}/aarch64-unknown-linux-gnu/release/audio-service $DEVICE_HOST:bin/ && \
    ssh -t $DEVICE_HOST "RUST_LOG=info,audio-service=debug,ort=warn TTS_ASSET_DIR=~/audio-service/assets PIPER_ESPEAKNG_DATA_DIRECTORY=/usr/lib/aarch64-linux-gnu ~/bin/audio-service"

[group('⚙️ Deploy (Release)')]
debug-runtime *ARGS: build-runtime-native-aarch64
    . device-env.sh && \
    ssh $DEVICE_HOST "systemctl --user stop runtime-native.service" && \
    ssh $DEVICE_HOST "killall runtime-native; true" && \
    scp target/aarch64-unknown-linux-gnu/release/runtime-native $DEVICE_HOST:bin/ && \
    ssh -t $DEVICE_HOST "RUST_LOG=info,runtime-native=debug ~/bin/runtime-native --device {{ARGS}}"

# ==========================================
# 🧹 Clean
# ==========================================

# 빌드 결과물 및 앱 관련 디렉토리 삭제
[group('🧹 Clean')]
clean-apps: 
    @echo "🧹 Cleaning build artifacts..."
    cargo clean
    rm -rf "$HOME/Library/Application Support/com.moredream.applets"
    @echo "✅ Cleaned apps and build directories."

# 런타임 데이터 삭제 (build-applet의 OS별 경로 참조)
[group('🧹 Clean')]
clean-data:
    @echo "🧹 Cleaning local runtime data..."
    @case "$(uname -s)" in \
        Darwin*) rm -rf "$HOME/Library/Application Support/com.moredream.runtime" ;; \
        Linux*)  rm -rf "$HOME/.local/share/runtime" ;; \
        *)       rm -rf "$APPDATA/moredream/runtime" ;; \
    esac
    @echo "✅ Cleaned runtime data."

# 앱 설정(Settings) 데이터 삭제
[group('🧹 Clean')]
clean-settings:
    @echo "🧹 Cleaning settings..."
    @case "$(uname -s)" in \
        Darwin*) rm -rf "$HOME/Library/Preferences/com.moredream.runtime.plist" ;; \
        Linux*)  rm -rf "$HOME/.config/runtime" ;; \
        *)       rm -rf "$APPDATA/moredream/runtime/config" ;; \
    esac
    @echo "✅ Cleaned settings."

# 모든 Clean 작업 일괄 수행
[group('🧹 Clean')]
clean-all: clean-apps clean-data clean-settings
    @echo "🚀 All clean tasks completed successfully!"
