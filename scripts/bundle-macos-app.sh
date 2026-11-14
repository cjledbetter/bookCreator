#!/usr/bin/env bash
set -euo pipefail

APP_NAME="Book Creator"
BUNDLE_DIR="dist/${APP_NAME}.app"
BIN_PATH="${1:-target/release/book-creator}"

if [ ! -f "$BIN_PATH" ]; then
    echo "Error: Binary not found at $BIN_PATH"
    exit 1
fi

rm -rf "$BUNDLE_DIR"
mkdir -p "$BUNDLE_DIR/Contents/MacOS"
mkdir -p "$BUNDLE_DIR/Contents/Resources"

cp "$BIN_PATH" "$BUNDLE_DIR/Contents/MacOS/book-creator"
chmod +x "$BUNDLE_DIR/Contents/MacOS/book-creator"

cat > "$BUNDLE_DIR/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>Book Creator</string>
    <key>CFBundleDisplayName</key>
    <string>Book Creator</string>
    <key>CFBundleIdentifier</key>
    <string>io.bookcreator.studio</string>
    <key>CFBundleVersion</key>
    <string>0.1.0</string>
    <key>CFBundleShortVersionString</key>
    <string>0.1.0</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleExecutable</key>
    <string>book-creator</string>
    <key>LSMinimumSystemVersion</key>
    <string>12.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>CFBundleDocumentTypes</key>
    <array>
        <dict>
            <key>CFBundleTypeName</key>
            <string>Portable Book Project</string>
            <key>CFBundleTypeExtensions</key>
            <array>
                <string>book</string>
            </array>
            <key>CFBundleTypeRole</key>
            <string>Editor</string>
        </dict>
    </array>
</dict>
</plist>
PLIST

# Ad-hoc sign on macOS (required by Apple Silicon M1/M2/M3/M4 to execute local binaries)
if command -v codesign >/dev/null 2>&1; then
    codesign --force --deep --sign - "$BUNDLE_DIR"
fi

echo "Created macOS bundle at: $BUNDLE_DIR"
