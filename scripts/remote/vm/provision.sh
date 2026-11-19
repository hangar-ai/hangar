#!/usr/bin/env bash
# The build machine's setup, run as root on a fresh Vast.ai instance (an ubuntu:22.04 container) by
# scripts/remote/hangar_rbuild.py, which streams this file over SSH. Idempotent.
#
# What the CI's rust job installs on its ubuntu-22.04 runner (.github/workflows/ci.yml): the same release, so
# the same glibc as the AppImage; the system libraries the workspace links (WebKitGTK for the webview helper's
# engine, ALSA for cpal, libclang for whisper.cpp's bindings, OpenSSL), cmake and a C/C++ compiler for
# whisper.cpp, mold as the linker (`ld` points to it, like rui314/setup-mold), and rustup's stable toolchain
# with rustfmt, clippy and the wasm32 target the kit tests build for. Mesa's software Vulkan driver (lavapipe)
# lets the 3D capture tests draw, as on a GitHub runner, instead of skipping for want of an adapter, and the
# fonts a runner has let the views' tests shape text (cosmic-text panics with "no default font found" without).
set -euo pipefail
MOLD_VERSION=3.0.0
export DEBIAN_FRONTEND=noninteractive
hb=/root/hb
mkdir -p "$hb/state" "$hb/lanes" "$hb/bin"
say() { echo "[rbuild-box] $*"; }
t0=$(date +%s)

APT=(apt-get -o DPkg::Lock::Timeout=300 -o Acquire::Retries=3)
PKGS=(build-essential pkg-config cmake clang libclang-dev libwebkit2gtk-4.1-dev libasound2-dev libssl-dev
      git rsync curl ca-certificates python3 xz-utils zstd procps openssh-client file
      mesa-vulkan-drivers libvulkan1 fontconfig fonts-dejavu-core fonts-liberation)
ok=0
for attempt in 1 2 3; do
  if "${APT[@]}" update -qq >"$hb/state/apt.log" 2>&1 &&
     "${APT[@]}" install -y -qq --no-install-recommends "${PKGS[@]}" >>"$hb/state/apt.log" 2>&1; then
    ok=1; break
  fi
  say "apt-get failed (attempt $attempt), retrying"; sleep $((attempt * 15))
done
[ "$ok" = 1 ] || { tail -25 "$hb/state/apt.log"; exit 1; }
say "system packages in $(( $(date +%s) - t0 )) s"

if ! /usr/local/bin/mold --version 2>/dev/null | grep -q "mold $MOLD_VERSION "; then
  curl -fsSL --retry 3 "https://github.com/rui314/mold/releases/download/v$MOLD_VERSION/mold-$MOLD_VERSION-x86_64-linux.tar.gz" |
    tar -xz -C /usr/local --strip-components=1
fi
ld_real=$(realpath /usr/bin/ld)
[ "$ld_real" = /usr/local/bin/mold ] || ln -sf /usr/local/bin/mold "$ld_real"

export CARGO_HOME=/root/.cargo RUSTUP_HOME=/root/.rustup PATH=/root/.cargo/bin:$PATH
if [ ! -x /root/.cargo/bin/rustup ]; then
  curl --proto '=https' --tlsv1.2 -fsSL --retry 3 https://sh.rustup.rs |
    sh -s -- -y -q --no-modify-path --profile minimal --default-toolchain stable \
      --component rustfmt,clippy --target wasm32-unknown-unknown >"$hb/state/rustup.log" 2>&1 ||
    { tail -25 "$hb/state/rustup.log"; exit 1; }
else
  rustup update stable --no-self-update >"$hb/state/rustup.log" 2>&1 || true
  rustup component add rustfmt clippy >>"$hb/state/rustup.log" 2>&1
  rustup target add wasm32-unknown-unknown >>"$hb/state/rustup.log" 2>&1
fi

say "ready in $(( $(date +%s) - t0 )) s: $(nproc) threads, $(awk '/MemTotal/{printf "%d", $2/1048576}' /proc/meminfo) GB," \
    "$(df -BG --output=avail /root | tail -1 | tr -d ' ') free, glibc $(ldd --version | head -1 | awk '{print $NF}')," \
    "$(rustc -V), $(ld --version | head -1)"
