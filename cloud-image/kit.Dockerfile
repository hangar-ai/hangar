# syntax=docker/dockerfile:1
#
# The box's kit: what a machine started from the person's own image
# (`.hangar/cloud.toml`'s `image`) downloads at start to become a HangarAI
# Cloud Builds machine, as ours is (docs/native/cloud-builds.md, "Your own
# image"). Every program in it is static: it runs in any Linux image, glibc
# or musl, whatever the image's libraries.
#
#   bin/hangar-box        the box itself (crates/hangar-box), the static
#                         musl build the image of Dockerfile carries
#   bin/tini              the container's first process: reaps orphans,
#                         passes signals to hangar-box
#   bin/zstd              the dependency cache's compression (crates/
#                         hangar-box/src/cache)
#   bin/ssh-keygen        the machine's host key, drawn at boot
#   sbin/sshd             OpenSSH's server, which hangar-box configures as in
#   libexec/sshd-session  our image (crates/hangar-box/src/sshd.rs): only
#   libexec/sshd-auth     `builder`, only the keys of the jobs the broker
#                         holds, each only to run `hangar-box session <job>`
#   SHA256SUMS            each file's SHA-256, as `sha256sum` writes them
#
# OpenSSH is built without OpenSSL: the box only ever uses Ed25519 keys and
# OpenSSH's own ciphers (chacha20-poly1305, AES-GCM and AES-CTR, curve25519
# and ML-KEM key exchange), which need no library, so nothing is linked but
# musl. It is built for /opt/hangar-box, where public/cloud-image/boot.sh
# puts the kit, and its privilege separation is the `sshd` user's, chrooted
# in /opt/hangar-box/empty, both of which `hangar-box boot` makes.
#
# Built at a release, beside the image, with the same hangar-box:
#
#   cp target/x86_64-unknown-linux-musl/dist/hangar-box public/cloud-image/
#   docker build -f public/cloud-image/kit.Dockerfile \
#     --output type=local,dest=kit public/cloud-image
#
# then served at the URL the broker gives the machine (HGC_KIT_URL), with the
# SHA-256 of its SHA256SUMS (HGC_KIT_SHA256): boot.sh checks every file
# against them before it runs anything.
#
# The sources' versions and hashes are pinned here; the Alpine packages are
# the build's tools only, none of them is in the kit but musl's static C
# library and tini-static.

ARG ALPINE=alpine:3.22

FROM ${ALPINE} AS build
RUN apk add --no-cache build-base linux-headers perl zlib-static zlib-dev tini-static curl

# OpenSSH, portable, 10.6p2 (2026-11-05).
ARG OPENSSH=openssh-10.6p2
ARG OPENSSH_SHA256=9ae44872090cf2f06b3cf9ff147e7f16f309671340763ed2f99248398ae29d5b
WORKDIR /src
RUN curl -fsSLo openssh.tar.gz "https://cdn.openbsd.org/pub/OpenBSD/OpenSSH/portable/${OPENSSH}.tar.gz" \
 && echo "${OPENSSH_SHA256}  openssh.tar.gz" | sha256sum -c - \
 && tar xzf openssh.tar.gz
WORKDIR /src/${OPENSSH}
# auth-passwd.c calls free() without stdlib.h, which musl's headers do not
# bring in by another way and GCC 14 refuses.
RUN sed -i 's/^#include <stdio.h>$/#include <stdio.h>\n#include <stdlib.h>/' auth-passwd.c \
 && ./configure \
      --prefix=/opt/hangar-box \
      --sysconfdir=/opt/hangar-box/etc \
      --with-privsep-path=/opt/hangar-box/empty \
      --with-privsep-user=sshd \
      --with-pid-dir=/run/hangar-box \
      --without-openssl \
      --without-pam \
      --without-selinux \
      --without-kerberos5 \
      --disable-security-key \
      --disable-lastlog --disable-utmp --disable-utmpx --disable-wtmp --disable-wtmpx \
      --with-ldflags=-static \
 && make -j"$(nproc)" sshd sshd-session sshd-auth ssh-keygen \
 && mkdir -p /kit/bin /kit/sbin /kit/libexec \
 && install -m 0755 -s sshd /kit/sbin/sshd \
 && install -m 0755 -s sshd-session sshd-auth /kit/libexec/ \
 && install -m 0755 -s ssh-keygen /kit/bin/ssh-keygen

# zstd 1.5.7, its command alone, static.
ARG ZSTD=zstd-1.5.7
ARG ZSTD_SHA256=eb33e51f49a15e023950cd7825ca74a4a2b43db8354825ac24fc1b7ee09e6fa3
WORKDIR /src
RUN curl -fsSLo zstd.tar.gz "https://github.com/facebook/zstd/releases/download/v${ZSTD#zstd-}/${ZSTD}.tar.gz" \
 && echo "${ZSTD_SHA256}  zstd.tar.gz" | sha256sum -c - \
 && tar xzf zstd.tar.gz \
 && make -C "${ZSTD}/programs" -j"$(nproc)" zstd-release LDFLAGS=-static HAVE_ZLIB=0 HAVE_LZMA=0 HAVE_LZ4=0 \
 && install -m 0755 -s "${ZSTD}/programs/zstd" /kit/bin/zstd \
 && install -m 0755 /sbin/tini-static /kit/bin/tini

# Every program static, or the kit would not run in an image of another C
# library.
COPY --chmod=0755 hangar-box /kit/bin/hangar-box
RUN apk add --no-cache file \
 && for f in /kit/bin/* /kit/sbin/* /kit/libexec/*; do \
      file "$f" | grep -Eq 'static(-pie)? linked|statically linked' \
        || { echo "$f is not static: $(file "$f")"; exit 1; }; \
    done \
 && /kit/bin/ssh-keygen -q -t ed25519 -N '' -C '' -f /tmp/check \
 && /kit/bin/zstd -q -c /tmp/check | /kit/bin/zstd -q -d -c | cmp - /tmp/check \
 && cd /kit && find bin sbin libexec -type f | sort | xargs sha256sum > SHA256SUMS

FROM scratch AS kit
COPY --from=build /kit/ /
