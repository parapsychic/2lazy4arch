#!/usr/bin/env bash
# Downloads the latest 2lazy4arch release and runs it. On the Arch ISO, as root:
#
#   curl -fsSL https://raw.githubusercontent.com/parapsychic/2lazy4arch/main/install.sh | bash
#
# Arguments go to 2lazy4arch, e.g. an unattended install from a config file:
#
#   curl -fsSL https://raw.githubusercontent.com/parapsychic/2lazy4arch/main/install.sh \
#     | bash -s -- --config-file https://example.com/my-arch.yaml --no-confirm
#
# LAZY_VERSION=v2.1.0 picks a release instead of the latest one.
set -euo pipefail

repo="parapsychic/2lazy4arch"
version="${LAZY_VERSION:-latest}"
if [[ $version == latest ]]; then
    url="https://github.com/$repo/releases/latest/download/2lazy4arch"
else
    url="https://github.com/$repo/releases/download/$version/2lazy4arch"
fi

if [[ $EUID -ne 0 ]]; then
    echo "Run this as root, on the Arch ISO." >&2
    exit 1
fi
if [[ ! -d /sys/firmware/efi ]]; then
    echo "This machine booted in BIOS mode. Boot the Arch ISO in UEFI mode; 2lazy4arch only installs on UEFI." >&2
    exit 1
fi

bin=/usr/local/bin/2lazy4arch
echo "Downloading $url"
curl -fL --retry 3 --progress-bar -o "$bin.part" "$url"
chmod +x "$bin.part"
mv "$bin.part" "$bin"

# Piped into bash, stdin is the rest of this script; the installer wants the keyboard.
if [[ ! -t 0 ]] && { true < /dev/tty; } 2>/dev/null; then
    exec "$bin" "$@" < /dev/tty
fi
exec "$bin" "$@"
