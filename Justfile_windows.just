# Windows/PowerShell support config
set windows-shell := ["powershell.exe", "-NoProfile", "-Command"]

dist_bin_dir := "dist/bin"
dist_applets_dir := "dist/applets"
applets_source_dir := "applets"
applets_dist_dir := "runtime-web/public/applets"
applets := "analog-clock drawing-game graph dot-drawing launcher settings find-dot mirror cross-memory memo tetris audio-test tactile-display-demo network-test weather tactile-game tactile-test braille-test charter-of-national-education device-manual rhythm-game"
cross_target_dir := "target-cross"

# Show available recipes
default:
    @just --list

# ==========================================
# 🔍 Check Environments (Windows)
# ==========================================

_check-rust:
    @powershell -NoProfile -Command "if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { Write-Host '❌ cargo is not installed.'; exit 1 }"

_check-zig:
    @powershell -NoProfile -Command "if (-not (Get-Command zig -ErrorAction SilentlyContinue)) { Write-Host '❌ zig is not installed.'; exit 1 }"

_add-wasm-target: _check-rust
    rustup target add wasm32-unknown-unknown

_check-nodejs:
    @powershell -NoProfile -Command "if (-not (Get-Command npx -ErrorAction SilentlyContinue)) { Write-Host '❌ npx is not installed.'; exit 1 }"

_install-trunk: _check-rust
    @powershell -NoProfile -Command "if (-not (Get-Command trunk -ErrorAction SilentlyContinue)) { cargo install trunk --locked }"

_install-watchexec: _check-rust
    @powershell -NoProfile -Command "if (-not (Get-Command watchexec -ErrorAction SilentlyContinue)) { cargo install watchexec-cli }"

_install-leptosfmt: _check-rust
    @powershell -NoProfile -Command "if (-not (Get-Command leptosfmt -ErrorAction SilentlyContinue)) { cargo install leptosfmt }"

_install-wasm-tools: _check-rust
    @powershell -NoProfile -Command "if (-not (Get-Command wasm-tools -ErrorAction SilentlyContinue)) { cargo install wasm-tools --locked }"

_install-espup: _check-rust
    @powershell -NoProfile -Command "if (-not (Get-Command espup -ErrorAction SilentlyContinue)) { cargo install espup }"

_install-cross: _check-rust
    @powershell -NoProfile -Command "if (-not (Get-Command cross -ErrorAction SilentlyContinue)) { cargo install cross --git https://github.com/cross-rs/cross }"

_install-cargo-zigbuild: _check-rust
    @powershell -NoProfile -Command "if (-not (Get-Command cargo-zigbuild -ErrorAction SilentlyContinue)) { cargo install cargo-zigbuild }"

_install-jaq: _check-rust
    @powershell -NoProfile -Command "if (-not (Get-Command jaq -ErrorAction SilentlyContinue)) { cargo install jaq }"

# ==========================================
# 🔍 Check code quality
# ==========================================

[group('🔍 Check code quality')]
fmt: _check-rust _check-nodejs _install-leptosfmt
    cargo fmt --all
    powershell -NoProfile -Command "Get-ChildItem -Path web/src -Recurse -Filter *.rs -ErrorAction SilentlyContinue | ForEach-Object { leptosfmt $_.FullName }"
    npx -y prettier --write "**/*.md"

[group('🔍 Check code quality')]
clippy: _check-rust
    cargo clippy --workspace -- --deny warnings

[group('🔍 Check code quality')]
lint: clippy

[group('🔍 Check code quality')]
check: _check-rust fmt lint
    cargo check --workspace --exclude capture

# ==========================================
# 📄 Documentation
# ==========================================

[group('📄 Documentation')]
doc: _check-rust
    cargo doc --workspace --open

# ==========================================
# ⚙️ Applets & Runtime Development
# ==========================================

[group('⚙️ Applets & Runtime Development')]
watch-applets: _install-watchexec
    watchexec --debounce 500ms --watch applets --exts rs -- just build-applets

[group('⚙️ Applets & Runtime Development')]
build-runtime-native-debug: _check-rust
    cargo build --package runtime-native

[group('⚙️ Applets & Runtime Development')]
run-applet *ARGS: build-runtime-native-debug
    powershell -NoProfile -Command "$env:RUST_LOG='info,runtime_native=debug'; cargo run --package runtime-native -- {{ARGS}}"

[group('⚙️ Applets & Runtime Development')]
watch-runtime-native: _install-watchexec
    watchexec --watch runtime-native --exts rs -- just build-runtime-native-debug

[group('⚙️ Applets & Runtime Development')]
serve-runtime-web *ARGS: _install-trunk build-applets
    powershell -NoProfile -Command "Set-Location runtime-web; trunk serve {{ARGS}}"

alias run-web := serve-runtime-web

# 애플릿 하나만 다시 빌드하고 웹 시뮬레이터를 띄웁니다.
# 나머지 애플릿은 dist/applets 에 이미 빌드된 산출물을 그대로 재사용합니다.
[group('⚙️ Applets & Runtime Development')]
serve-runtime-web-applet applet *ARGS: _install-trunk (build-applet applet) refresh-applets
    powershell -NoProfile -Command "Set-Location runtime-web; trunk serve {{ARGS}}"

alias run-web-applet := serve-runtime-web-applet

[group('⚙️ Applets & Runtime Development')]
serve-audio-service: _check-rust
    powershell -NoProfile -Command "`$env:LIBCLANG_PATH='C:\Program Files\LLVM\bin'; `$env:BINDGEN_EXTRA_CLANG_ARGS='--target=x86_64-pc-windows-msvc'; `$env:PIPER_ESPEAKNG_DATA_DIRECTORY='C:\Program Files\eSpeak NG\espeak-ng-data'; `$env:TTS_ASSET_DIR='./assets'; `$env:RUST_LOG='info,audio_service=debug,ort=warn'; cargo watch --workdir audio-service -w src -w Cargo.toml -x 'run -p audio-service'"

[group('⚙️ PC Apps Development')]
run-capture *ARGS: _check-rust
    cargo run --package capture --release -- {{ARGS}}

# ==========================================
# ⚙️ Build (Release)
# ==========================================

[group('⚙️ Build (Release)')]
build-applet applet: _add-wasm-target _install-wasm-tools
    cargo build -p {{applet}} --target wasm32-unknown-unknown --release
    powershell -NoProfile -Command "New-Item -ItemType Directory -Force -Path '{{dist_applets_dir}}' | Out-Null"
    powershell -NoProfile -Command "New-Item -ItemType Directory -Force -Path (Join-Path $env:APPDATA 'moredream/runtime/data/applets') | Out-Null"
    powershell -NoProfile -Command "`$aOrig = cargo run -q -p applet-metadata-tool -- {{applets_source_dir}}/{{applet}} --name; `$aName = `$aOrig.Trim().Replace('-', '_'); cargo run -q -p applet-metadata-tool -- {{applets_source_dir}}/{{applet}} --embed target/wasm32-unknown-unknown/release/`$aName.wasm -o {{dist_applets_dir}}/`$aOrig.wasm; `$targetPath = Join-Path $env:APPDATA ('moredream/runtime/data/applets/' + `$aOrig + '.wasm'); Copy-Item -Path {{dist_applets_dir}}/`$aOrig.wasm -Destination `$targetPath -ErrorAction SilentlyContinue; Write-Host ('Copied ' + `$aOrig + '.wasm')"

[group('⚙️ Build (Release)')]
build-applets:
    powershell -NoProfile -Command "foreach (`$a in '{{applets}}'.Split(' ')) { just build-applet `$a }; just refresh-applets"

[group('⚙️ Build (Release)')]
refresh-applets: _install-jaq
    powershell -NoProfile -Command "if (Test-Path '{{applets_dist_dir}}_tmp') { Remove-Item -Recurse -Force '{{applets_dist_dir}}_tmp' }; New-Item -ItemType Directory -Force -Path '{{applets_dist_dir}}_tmp' | Out-Null; Get-ChildItem -Path '{{dist_applets_dir}}\*.wasm' -ErrorAction SilentlyContinue | Copy-Item -Destination '{{applets_dist_dir}}_tmp'; `$files = @(Get-ChildItem -Path '{{applets_dist_dir}}_tmp\*.wasm' -ErrorAction SilentlyContinue | Select-Object -ExpandProperty BaseName); `$json = '[' + ((`$files | ForEach-Object { `$_ | ConvertTo-Json -Compress }) -join ',') + ']'; Set-Content -Path '{{applets_dist_dir}}_tmp/applets.json' -Value `$json; if (Test-Path '{{applets_dist_dir}}') { Remove-Item -Recurse -Force '{{applets_dist_dir}}' }; Move-Item -Path '{{applets_dist_dir}}_tmp' -Destination '{{applets_dist_dir}}'; Write-Host 'Generated {{applets_dist_dir}}/applets.json'"

[group('⚙️ Build (Release)')]
build-runtime-native: _check-rust
    cargo build --package runtime-native --release

[group('⚙️ Build (Release)')]
build-runtime-web: _install-trunk
    powershell -NoProfile -Command "Set-Location runtime-web; trunk build"

[group('⚙️ Build (Release)')]
build-audio-service: _check-rust
    powershell -NoProfile -Command "`$env:LIBCLANG_PATH='C:\Program Files\LLVM\bin'; `$env:BINDGEN_EXTRA_CLANG_ARGS='--target=x86_64-pc-windows-msvc'; cargo build --package audio-service --release"

[group('⚙️ Build (Release)')]
build-capture: _check-rust
    cargo build --package capture --release

[group('⚙️ Build (Release)')]
build: build-applets build-runtime-native build-runtime-web build-audio-service

# ==========================================
# 🧹 Clean
# ==========================================

[group('🧹 Clean')]
clean-apps: 
    @echo "🧹 Cleaning build artifacts..."
    cargo clean
    powershell -NoProfile -Command "$appPath = Join-Path $env:APPDATA 'moredream/applets'; if (Test-Path $appPath) { Remove-Item -Recurse -Force $appPath }"
    @echo "✅ Cleaned apps and build directories."

[group('🧹 Clean')]
clean-data:
    @echo "🧹 Cleaning local runtime data..."
    powershell -NoProfile -Command "$dataPath = Join-Path $env:APPDATA 'moredream/runtime'; if (Test-Path $dataPath) { Remove-Item -Recurse -Force $dataPath }"
    @echo "✅ Cleaned runtime data."

[group('🧹 Clean')]
clean-settings:
    @echo "🧹 Cleaning settings..."
    powershell -NoProfile -Command "$configPath = Join-Path $env:APPDATA 'moredream/runtime/config'; if (Test-Path $configPath) { Remove-Item -Recurse -Force $configPath }"
    @echo "✅ Cleaned settings."

[group('🧹 Clean')]
clean-all: clean-apps clean-data clean-settings
    @echo "🚀 All clean tasks completed successfully!"
