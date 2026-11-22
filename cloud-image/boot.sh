#!/bin/sh
# The start of a HangarAI Cloud Builds machine made from the person's own
# image (docs/native/cloud-builds.md, "Your own image"): the broker starts
# the image with `sh -c` and this script in place of its entrypoint (Vast:
# runtype "args", onstart "sh", args ["-c", <this file>, "hangar-boot"]).
# It downloads the box's kit (public/cloud-image/kit.Dockerfile) from
# HGC_KIT_URL, checks its list against the SHA-256 the broker named
# (HGC_KIT_SHA256) and every file against the list, before anything of it
# runs, installs it in /opt/hangar-box, and becomes tini, then `hangar-box
# boot`, which makes the image a machine like ours. The hashes, not TLS,
# prove the files: an image without CA certificates still downloads them.
# Needs: sh; curl, wget, python3, perl or bash; sha256sum, shasum, openssl
# or python3.
set -eu
umask 022
say() { echo "hangar-box boot: $*" >&2; }
die() { say "$*"; exit 1; }

url=${HGC_KIT_URL:-}
want=${HGC_KIT_SHA256:-}
kit=${HGC_KIT_DIR:-/opt/hangar-box}
case $url in https://?* | http://?*) ;; *) die "HGC_KIT_URL is not an address: '$url'" ;; esac
case $want in *[!0-9a-f]* | '') die "HGC_KIT_SHA256 is not a SHA-256" ;; esac
[ ${#want} -eq 64 ] || die "HGC_KIT_SHA256 is not a SHA-256"
url=${url%/}

fetch() {
	if command -v curl >/dev/null 2>&1; then
		curl -fsSL --retry 5 --retry-delay 2 --connect-timeout 20 -o "$2" "$1" && return 0
		curl -fsSLk --retry 5 --retry-delay 2 --connect-timeout 20 -o "$2" "$1" && return 0
	fi
	if command -v wget >/dev/null 2>&1; then
		wget -q -T 30 -O "$2" "$1" && return 0
		wget -q -T 30 --no-check-certificate -O "$2" "$1" && return 0
	fi
	for py in python3 python; do
		command -v $py >/dev/null 2>&1 || continue
		$py -I -c 'import shutil, ssl, sys, urllib.request as u
try: r = u.urlopen(sys.argv[1], timeout=60)
except Exception: r = u.urlopen(sys.argv[1], timeout=60, context=ssl._create_unverified_context())
with open(sys.argv[2], "wb") as f: shutil.copyfileobj(r, f)' "$1" "$2" && return 0
	done
	# Debian's, Ubuntu's and NVIDIA's own images have none of these, but
	# perl-base and bash: the same address in plain HTTP.
	rest=${1#*://}
	host=${rest%%/*}
	port=80
	case $host in *:*) port=${host##*:} host=${host%:*} ;; esac
	if command -v perl >/dev/null 2>&1; then
		perl -e 'use IO::Socket::INET;
my ($host, $port, $path, $out) = @ARGV;
my $s = IO::Socket::INET->new(PeerAddr => $host, PeerPort => $port, Timeout => 30) or exit 1;
binmode $s;
print $s "GET /$path HTTP/1.0\r\nHost: $host\r\nUser-Agent: hangar-box-boot\r\n\r\n";
my $status = <$s>;
exit 1 unless defined $status && $status =~ m{^HTTP/1\.\d 200};
while (my $line = <$s>) { last if $line =~ /^\r?\n$/ }
open(my $f, ">", $out) or exit 1;
binmode $f;
my $buf;
while (my $n = read($s, $buf, 1 << 16)) { print $f $buf or exit 1 }
close $f or exit 1;' "$host" "$port" "${rest#*/}" "$2" && return 0
	fi
	command -v bash >/dev/null 2>&1 || return 1
	bash -c 'exec 3<>"/dev/tcp/$1/$2" || exit 1
printf "GET /%s HTTP/1.0\r\nHost: %s\r\nUser-Agent: hangar-box-boot\r\n\r\n" "$3" "$1" >&3
IFS= read -r status <&3 || exit 1
case $status in "HTTP/1."?" 200"*) ;; *) exit 1 ;; esac
while IFS= read -r line <&3; do [ -z "${line%$(printf "\r")}" ] && break; done
cat <&3 >"$4"' hangar-boot "$host" "$port" "${rest#*/}" "$2"
}

sha256() {
	for tool in sha256sum "busybox sha256sum" "shasum -a 256" "openssl dgst -sha256 -r"; do
		out=$($tool "$1" 2>/dev/null) && [ -n "$out" ] && { echo "${out%% *}"; return 0; }
	done
	for py in python3 python; do
		$py -I -c 'import hashlib, sys
h = hashlib.sha256()
with open(sys.argv[1], "rb") as f:
    for b in iter(lambda: f.read(1 << 20), b""): h.update(b)
print(h.hexdigest())' "$1" 2>/dev/null && return 0
	done
	return 1
}

# A machine started again keeps the kit it checked.
if [ -f "$kit/SHA256SUMS" ] && [ "$(sha256 "$kit/SHA256SUMS" || true)" = "$want" ]; then
	say "the kit is there already"
else
	stage="$kit.part"
	rm -rf "$stage"
	mkdir -p "$stage"
	fetch "$url/SHA256SUMS" "$stage/SHA256SUMS" || die "the kit's list could not be downloaded from $url"
	got=$(sha256 "$stage/SHA256SUMS") || die "this image cannot compute a SHA-256 (sha256sum, shasum, openssl or python3)"
	[ "$got" = "$want" ] || die "the kit's list is not the one the broker named ($got)"
	while read -r sum path; do
		case $sum in *[!0-9a-f]* | '') die "the kit's list is not right" ;; esac
		[ ${#sum} -eq 64 ] || die "the kit's list is not right"
		case $path in bin/[a-z]* | sbin/[a-z]* | libexec/[a-z]*) ;; *) die "the kit's list names '$path'" ;; esac
		case $path in */*/* | *[!a-z0-9/_.-]* | *..*) die "the kit's list names '$path'" ;; esac
		mkdir -p "$stage/${path%/*}"
		fetch "$url/$path" "$stage/$path" || die "$path could not be downloaded"
		[ "$(sha256 "$stage/$path")" = "$sum" ] || die "$path is not the file the kit's list names"
		chmod 0755 "$stage/$path"
	done <"$stage/SHA256SUMS"
	for need in bin/hangar-box bin/tini bin/ssh-keygen bin/zstd sbin/sshd; do
		[ -x "$stage/$need" ] || die "the kit has no $need"
	done
	rm -rf "$kit"
	mv "$stage" "$kit"
	say "the kit is checked and installed in $kit"
fi
exec "$kit/bin/tini" -s -- "$kit/bin/hangar-box" boot
