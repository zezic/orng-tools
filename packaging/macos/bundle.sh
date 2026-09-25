#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Sergey Ukolov
# SPDX-License-Identifier: GPL-3.0-only
#
# Builds `ORNG Registry.app` for both Mac architectures and packs it for
# `install.sh`. Run from anywhere; writes `target/bundle/`.
#
# The bundle is signed ad hoc and not notarized: there is no Developer ID to
# sign with. Apple silicon will not run unsigned code at all, and an ad hoc
# signature is what satisfies that without one. What it does not satisfy is
# Gatekeeper, which is why the installer fetches the archive itself rather than
# having it downloaded through a browser - see `install.sh`.

set -euo pipefail

cd "$(dirname "$0")/../.."

# `path+file:///.../orng-registry#0.1.0`, or `...#orng-registry@0.1.0` when the
# directory and the package are named differently.
id=$(cargo pkgid -p orng-registry)
version=${id##*[#@]}

name="ORNG Registry"
executable=orng-registry
# The first macOS Apple silicon shipped with, so the oldest either half of the
# universal binary can be built for. Stated here so both halves say the same.
export MACOSX_DEPLOYMENT_TARGET=11.0

slices=()
for target in aarch64-apple-darwin x86_64-apple-darwin; do
    cargo build --release --locked -p orng-registry --target "$target"
    slices+=("target/$target/release/$executable")
done

out=target/bundle
app="$out/$name.app"
rm -rf "$out"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"

lipo -create -output "$app/Contents/MacOS/$executable" "${slices[@]}"

# Every size Finder and the Dock ask for, from the one picture.
iconset="$out/icon.iconset"
mkdir "$iconset"
for side in 16 32 128 256 512; do
    sips -z "$side" "$side" apps/orng-registry/assets/icon.png \
        --out "$iconset/icon_${side}x${side}.png" > /dev/null
    double=$((side * 2))
    sips -z "$double" "$double" apps/orng-registry/assets/icon.png \
        --out "$iconset/icon_${side}x${side}@2x.png" > /dev/null
done
iconutil -c icns -o "$app/Contents/Resources/icon.icns" "$iconset"
rm -r "$iconset"

cat > "$app/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>en</string>
    <key>CFBundleDisplayName</key>
    <string>$name</string>
    <key>CFBundleExecutable</key>
    <string>$executable</string>
    <key>CFBundleIconFile</key>
    <string>icon</string>
    <key>CFBundleIdentifier</key>
    <string>io.github.zezic.orng-registry</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>$name</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>$version</string>
    <key>CFBundleVersion</key>
    <string>$version</string>
    <key>LSApplicationCategoryType</key>
    <string>public.app-category.music</string>
    <key>LSMinimumSystemVersion</key>
    <string>$MACOSX_DEPLOYMENT_TARGET</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
EOF
plutil -lint "$app/Contents/Info.plist" > /dev/null

# Seals the bundle as well as signing the executable, so the Info.plist and the
# icon are part of what the signature covers.
codesign --force --sign - "$app"
codesign --verify --strict "$app"

# Without these the archive carries this machine's extended attributes -
# `com.apple.provenance` on every entry, as pax headers - and `._` files beside
# them, and extracting it would put them on the user's copy.
COPYFILE_DISABLE=1 tar --no-xattrs --no-mac-metadata \
    -C "$out" -czf "$out/orng-registry-macos.tar.gz" "$name.app"
echo "$out/orng-registry-macos.tar.gz: $name $version"
