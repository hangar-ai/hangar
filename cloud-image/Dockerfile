# syntax=docker/dockerfile:1
#
# The image of a HangarAI Cloud Builds machine (docs/native/cloud-services.md,
# "1. Cloud Builds", "Architecture"; docs/native/cloud-builds.md, "The box"):
# Ubuntu 24.04 with the toolchains a customer's check needs (git,
# build-essential, mold, rustup, Node LTS with pnpm, Python with uv, Go and a
# JDK) and hangar-box, the agent the machine runs.
#
# Published as ghcr.io/hangar-ai/hangar-box:<version>, one image per release
# of HangarAI, which the broker of that release rents its machines with
# (hangar-cloud-api's BOX_IMAGE_REPOSITORY and BOX_IMAGE). At a release, when
# the person asks: scripts/release/cloud-image <tag> builds the static
# hangar-box of the tag on the build machine and hands it to the public
# repository's workflow (cloud-box.yml beside this file, published as
# .github/workflows/cloud-box.yml by scripts/publish-public, with this file as
# cloud-image/Dockerfile), which builds this image on GitHub's runner, checks
# it and pushes it. docs/native/PACKAGING.md, "The build machines' image".
#
# The public repository has no source code: the build context is this folder
# with the hangar-box binary beside this file, a static Linux build of the
# private repository's crates/hangar-box:
#
#   cargo build --profile dist --locked -p hangar-box --target x86_64-unknown-linux-musl
#   cp target/x86_64-unknown-linux-musl/dist/hangar-box public/cloud-image/
#   docker build --build-arg VERSION=3.0.0 -t ghcr.io/hangar-ai/hangar-box:3.0.0 public/cloud-image
#
# The machine is started with this image's own entrypoint (not the provider's
# SSH launch mode, which would bring its own sshd and keys), port 2222 mapped
# (BOX_SSH_PORT: hangar-box's sshd listens there, and a test of hangar-box
# reads the EXPOSE below), and three environment variables the broker sets
# for this machine alone: HGC_BROKER_URL (https://account.hangarai.dev),
# HGC_BOX_TOKEN, a one-time token, and HGC_BOX_THREADS, the threads rented,
# which every job is held to. hangar-box then draws the host key, sends it to
# the broker, and lets in only the keys of the jobs the broker gives it
# (crates/hangar-box).
#
# Its size is the start of every machine: the host pulls it before anything
# runs (2026-11-20, 21 s on a host that had it, 67 to 149 s on hosts that
# did not, more than 10 minutes on a slow one). Hence the stages: each
# toolchain is trimmed in its own official image of what no build reads, then
# copied in once, with its final owner, so that nothing is in the image twice
# (`chown -R` after a copy wrote Rust's 650 MB again). Measured on
# 2026-11-20: 1.08 GB to pull and 3.10 GB unpacked before, 845 MB and 2.39 GB
# now (`docker images`: 4.18 GB, 3.23 GB); the toolchains, the compilers and
# the libraries a build links against are all still here. The layers stay
# gzip's, which every host's Docker pulls: zstd would save 7 % more (789 MB),
# but a host whose Docker cannot read it would lose the machine.
#
# The base images' tags (rust:1, node:lts, golang:1, uv:latest) are read when
# a release's image is built, and never again for that version: an image is
# built once per release and its tag is never pushed twice.

# Rust: rustup and the stable toolchain from the official image, with rustfmt
# and clippy, and nothing of rustup's downloads.
FROM rust:1-bookworm AS rust
RUN rustup component add rustfmt clippy \
 && rm -rf /usr/local/rustup/downloads/* /usr/local/rustup/tmp/*

# Node, the current LTS, with npm, corepack and pnpm; npm's cache and the
# release notes left out.
FROM node:lts-bookworm-slim AS node
RUN npm install --global pnpm@10 \
 && rm -rf /root/.npm /usr/local/share/doc /usr/local/share/man \
      /usr/local/CHANGELOG.md /usr/local/README.md

# Go, without the test suite and the test data of its own standard library
# (55 MB): building, testing and vetting a project read neither.
FROM golang:1-bookworm AS go
RUN rm -rf /usr/local/go/test \
 && find /usr/local/go/src -type d -name testdata -prune -exec rm -rf {} +

FROM ubuntu:24.04

# The release this image is of (3.0.0), which its tag carries too.
ARG VERSION=dev
LABEL org.opencontainers.image.title="hangar-box" \
      org.opencontainers.image.description="A HangarAI Cloud Builds machine: the toolchains and hangar-box" \
      org.opencontainers.image.source="https://github.com/hangar-ai/hangar" \
      org.opencontainers.image.url="https://hangarai.dev" \
      org.opencontainers.image.vendor="HangarAI" \
      org.opencontainers.image.version="${VERSION}" \
      org.opencontainers.image.ref.name="ghcr.io/hangar-ai/hangar-box:${VERSION}"

ARG DEBIAN_FRONTEND=noninteractive

# The system's part: compilers and linkers, git, a JDK, Python, the SSH
# server hangar-box configures itself, and tini, which reaps orphans and
# passes signals to hangar-box (its death ends the container, and every job
# with it). The base image already leaves out the manuals and the
# documentation; the package lists, apt's and dpkg's logs go in the same
# layer.
RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      build-essential pkg-config cmake clang mold \
      git ca-certificates curl unzip xz-utils zstd \
      libssl-dev zlib1g-dev \
      default-jdk-headless \
      python3 python3-venv python3-dev \
      openssh-server openssh-client \
      tini \
 && apt-get clean \
 && rm -rf /var/lib/apt/lists/* /var/cache/apt/* /var/cache/debconf/*-old \
      /var/log/apt /var/log/dpkg.log /var/log/alternatives.log \
 # The package's host keys would be the same on every machine: hangar-box
 # draws one per machine instead.
 && rm -f /etc/ssh/ssh_host_*

# The toolchains. Rust's home is `builder`'s (uid and group 2000, made below)
# to write, so that a project's rust-toolchain.toml can install its own
# version: given by the copy itself.
COPY --from=rust --chown=2000:2000 /usr/local/rustup /opt/rustup
COPY --from=rust /usr/local/cargo /opt/cargo
ENV RUSTUP_HOME=/opt/rustup CARGO_HOME=/opt/cargo
COPY --from=node /usr/local/ /usr/local/
COPY --from=go /usr/local/go /usr/local/go
# uv, which installs the Python a project asks for.
COPY --from=ghcr.io/astral-sh/uv:latest /uv /uvx /usr/local/bin/

# The jobs' user: no password (but not a locked account, which sshd would
# turn away without PAM), no group of the GPU's, a home the wipe empties.
RUN groupadd --gid 2000 builder \
 && useradd --create-home --uid 2000 --gid 2000 --shell /bin/sh builder \
 && usermod --password '*' builder

# The jobs' environment starts from this and nothing else: their caches
# (cargo's registry, Go's, pnpm's, uv's) in `builder`'s home, which the wipe
# empties; the toolchains' own folders, which hold nothing of a customer's.
#
# A job runs on the threads its machine was rented with, not its host's
# (hangar-box, cpus.rs), and hangar-box sets PYTHON_CPU_COUNT to their
# number: Python 3.13 and later read it for os.cpu_count(), and these lines
# make the image's Python 3.12, in a virtual environment too, do the same.
RUN mkdir -p /etc/hangar-box \
 && printf '%s\n' \
      '# The base environment of every job (hangar-box, lanes.rs).' \
      'PATH=/home/builder/.cargo/bin:/opt/cargo/bin:/usr/local/go/bin:/home/builder/go/bin:/home/builder/.local/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin' \
      'RUSTUP_HOME=/opt/rustup' \
      'CARGO_HOME=/home/builder/.cargo' \
      'GOPATH=/home/builder/go' \
      'PNPM_HOME=/home/builder/.local/share/pnpm' \
      'LANG=C.UTF-8' \
    > /etc/hangar-box/job.env \
 && printf '%s\n' \
      '' \
      '# hangar-box (public/cloud-image/Dockerfile): Python 3.13'"'"'s PYTHON_CPU_COUNT,' \
      '# so that os.cpu_count() says the threads a job was given, not the host'"'"'s.' \
      'import os as _os, sys as _sys' \
      '_n = "" if _sys.flags.ignore_environment else _os.environ.get("PYTHON_CPU_COUNT", "")' \
      'if _n.isascii() and _n.isdigit() and int(_n) > 0:' \
      '    _os.cpu_count = (lambda n: lambda: n)(int(_n))' \
      'del _os, _sys, _n' \
    >> "$(python3 -c 'import sysconfig; print(sysconfig.get_path("stdlib"))')/sitecustomize.py"

COPY --chmod=0755 hangar-box /usr/local/bin/hangar-box

EXPOSE 2222/tcp
ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/hangar-box"]
CMD ["serve"]
