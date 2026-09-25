#!/bin/sh
# SPDX-FileCopyrightText: 2026 Sergey Ukolov
# SPDX-License-Identifier: GPL-3.0-only
#
# Installs ORNG Registry into /Applications:
#
#     curl -fsSL https://github.com/zezic/orng-tools/releases/latest/download/install-macos.sh | sh
#
# or a particular release, by its tag:
#
#     curl -fsSL https://github.com/zezic/orng-tools/releases/latest/download/install-macos.sh | sh -s v0.1.0
#
# Why a script rather than a download link: the application is not notarized,
# so a copy a browser downloaded is quarantined, and Gatekeeper refuses to open
# it with a message that reads like the file is damaged. The quarantine is set
# by the program that downloads, and curl does not set it - so the copy this
# fetches opens like any other. Nothing here turns Gatekeeper off or edits a
# file's attributes.

set -eu

repo=zezic/orng-tools
asset=orng-registry-macos.tar.gz
app="ORNG Registry.app"
destination=/Applications

if [ "$(uname -s)" != Darwin ]; then
    echo "This installs the Mac application; see https://github.com/$repo/releases" >&2
    exit 1
fi

# One tag for both downloads, so the archive and its checksum cannot come from
# two releases if one is published in between.
if [ $# -gt 0 ]; then
    tag=$1
else
    # `latest` redirects to `.../releases/tag/<tag>`.
    latest=$(curl -fsSLI -o /dev/null -w '%{url_effective}' \
        "https://github.com/$repo/releases/latest")
    tag=${latest##*/}
fi
base="https://github.com/$repo/releases/download/$tag"

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

echo "Downloading ORNG Registry $tag"
curl -fL --progress-bar -o "$work/$asset" "$base/$asset"
curl -fsSL -o "$work/SHA256SUMS" "$base/SHA256SUMS"
(cd "$work" && grep " $asset\$" SHA256SUMS | shasum -a 256 -c - > /dev/null) || {
    echo "The download does not match the release's checksum." >&2
    exit 1
}

if pgrep -xq orng-registry; then
    echo "ORNG Registry is running. Quit it and run this again." >&2
    exit 1
fi

tar -xzf "$work/$asset" -C "$work"

# An administrator can write /Applications without asking; anybody else is
# asked for an administrator's password once, here, rather than failing.
sudo=
[ -w "$destination" ] || sudo=sudo
$sudo rm -rf "$destination/$app"
$sudo mv "$work/$app" "$destination/"

echo "Installed $destination/$app"
