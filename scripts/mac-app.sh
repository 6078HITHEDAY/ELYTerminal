#!/bin/sh
# mac-app.sh: target/release/ELYTerminal.app, signed ad hoc, and its disk image.
set -eu
cd "$(dirname "$0")/.."
cargo build --release --locked
version=$(cargo pkgid)
version=${version##*@}
out=target/release
app=$out/ELYTerminal.app
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
cargo run --release --locked --quiet --example icons -- \
  assets/icons/elyterminal-macos.svg "$work/ELYTerminal.iconset"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$out/elyterminal" "$app/Contents/MacOS/"
iconutil -c icns "$work/ELYTerminal.iconset" -o "$app/Contents/Resources/ELYTerminal.icns"
cat > "$app/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key><string>en</string>
  <key>CFBundleDisplayName</key><string>ELYTerminal</string>
  <key>CFBundleExecutable</key><string>elyterminal</string>
  <key>CFBundleIconFile</key><string>ELYTerminal</string>
  <key>CFBundleIdentifier</key><string>com.zacharyzhang.elyterminal</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>CFBundleName</key><string>ELYTerminal</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.developer-tools</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
EOF
plutil -lint -s "$app/Contents/Info.plist"
codesign --force --sign - "$app"
mkdir "$work/disk"
ditto "$app" "$work/disk/ELYTerminal.app"
ln -s /Applications "$work/disk/Applications"
dmg=$out/ELYTerminal-$version.dmg
hdiutil create -quiet -volname ELYTerminal -srcfolder "$work/disk" -ov -format UDZO "$dmg"
echo "mac-app: $app"
echo "mac-app: $dmg"
