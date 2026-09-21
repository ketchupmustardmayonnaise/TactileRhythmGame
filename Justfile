# Default entry point for Just
set windows-shell := ["powershell.exe", "-NoProfile", "-Command"]

# OS별 모듈 지정
mod? windows 'Justfile_windows.just'
mod? unix 'Justfile_unix.just'

# Show available recipes
default:
    @just --list

# Forward all arguments to OS module automatically
[no-cd]
@fmt *ARGS:
    just {{ if os() == "windows" { "windows" } else { "unix" } }}::fmt {{ ARGS }}

[no-cd]
@clippy *ARGS:
    just {{ if os() == "windows" { "windows" } else { "unix" } }}::clippy {{ ARGS }}

[no-cd]
@lint *ARGS:
    just {{ if os() == "windows" { "windows" } else { "unix" } }}::lint {{ ARGS }}

[no-cd]
@check *ARGS:
    just {{ if os() == "windows" { "windows" } else { "unix" } }}::check {{ ARGS }}

[no-cd]
@doc *ARGS:
    just {{ if os() == "windows" { "windows" } else { "unix" } }}::doc {{ ARGS }}

[no-cd]
@build *ARGS:
    just {{ if os() == "windows" { "windows" } else { "unix" } }}::build {{ ARGS }}

[no-cd]
@build-applet *ARGS:
    just {{ if os() == "windows" { "windows" } else { "unix" } }}::build-applet {{ ARGS }}

[no-cd]
@build-applets *ARGS:
    just {{ if os() == "windows" { "windows" } else { "unix" } }}::build-applets {{ ARGS }}

[no-cd]
@refresh-applets *ARGS:
    just {{ if os() == "windows" { "windows" } else { "unix" } }}::refresh-applets {{ ARGS }}

[no-cd]
@build-runtime-native *ARGS:
    just {{ if os() == "windows" { "windows" } else { "unix" } }}::build-runtime-native {{ ARGS }}

[no-cd]
@build-runtime-web *ARGS:
    just {{ if os() == "windows" { "windows" } else { "unix" } }}::build-runtime-web {{ ARGS }}

[no-cd]
@build-audio-service *ARGS:
    just {{ if os() == "windows" { "windows" } else { "unix" } }}::build-audio-service {{ ARGS }}

[no-cd]
@build-capture *ARGS:
    just {{ if os() == "windows" { "windows" } else { "unix" } }}::build-capture {{ ARGS }}

[no-cd]
@run-applet *ARGS:
    just {{ if os() == "windows" { "windows" } else { "unix" } }}::run-applet {{ ARGS }}

[no-cd]
@run-capture *ARGS:
    just {{ if os() == "windows" { "windows" } else { "unix" } }}::run-capture {{ ARGS }}

[no-cd]
@serve-runtime-web *ARGS:
    just {{ if os() == "windows" { "windows" } else { "unix" } }}::serve-runtime-web {{ ARGS }}

alias run-web := serve-runtime-web

[no-cd]
@serve-runtime-web-applet *ARGS:
    just {{ if os() == "windows" { "windows" } else { "unix" } }}::serve-runtime-web-applet {{ ARGS }}

alias run-web-applet := serve-runtime-web-applet

[no-cd]
@serve-audio-service *ARGS:
    just {{ if os() == "windows" { "windows" } else { "unix" } }}::serve-audio-service {{ ARGS }}

[no-cd]
@watch-applets *ARGS:
    just {{ if os() == "windows" { "windows" } else { "unix" } }}::watch-applets {{ ARGS }}

[no-cd]
@watch-runtime-native *ARGS:
    just {{ if os() == "windows" { "windows" } else { "unix" } }}::watch-runtime-native {{ ARGS }}

[no-cd]
@clean-apps *ARGS:
    just {{ if os() == "windows" { "windows" } else { "unix" } }}::clean-apps {{ ARGS }}

[no-cd]
@clean-data *ARGS:
    just {{ if os() == "windows" { "windows" } else { "unix" } }}::clean-data {{ ARGS }}

[no-cd]
@clean-settings *ARGS:
    just {{ if os() == "windows" { "windows" } else { "unix" } }}::clean-settings {{ ARGS }}

[no-cd]
@clean-all *ARGS:
    just {{ if os() == "windows" { "windows" } else { "unix" } }}::clean-all {{ ARGS }}

# Unix-only recipes
[no-cd]
@build-firmware *ARGS:
    just unix::build-firmware {{ ARGS }}

[no-cd]
@run-firmware *ARGS:
    just unix::run-firmware {{ ARGS }}

[no-cd]
@build-runtime-native-aarch64 *ARGS:
    just unix::build-runtime-native-aarch64 {{ ARGS }}

[no-cd]
@build-audio-service-aarch64 *ARGS:
    just unix::build-audio-service-aarch64 {{ ARGS }}

[no-cd]
@dist-build *ARGS:
    just unix::dist-build {{ ARGS }}

[no-cd]
@deploy *ARGS:
    just unix::deploy {{ ARGS }}

[no-cd]
@deploy-services *ARGS:
    just unix::deploy-services {{ ARGS }}

[no-cd]
@deploy-applets *ARGS:
    just unix::deploy-applets {{ ARGS }}

[no-cd]
@deploy-assets *ARGS:
    just unix::deploy-assets {{ ARGS }}

[no-cd]
@debug-audio *ARGS:
    just unix::debug-audio {{ ARGS }}

[no-cd]
@debug-runtime *ARGS:
    just unix::debug-runtime {{ ARGS }}
