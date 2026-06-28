#!/bin/bash
# macOS packaging script for agent-meter-proxy
# Creates DMG and PKG packages

set -e

VERSION="${1:-1.2.3}"
ARCH="${2:-aarch64}"

# Detect architecture
case "$ARCH" in
    x86_64|intel)
        DMG_ARCH="x64"
        ;;
    aarch64|arm64|m1|m2|m3|m4)
        DMG_ARCH="arm64"
        ;;
    *)
        echo "Unknown architecture: $ARCH"
        exit 1
        ;;
esac

# Create staging directory
STAGING="/tmp/agent-meter-proxy-dmg"
rm -rf "$STAGING"
mkdir -p "$STAGING"

# Copy binary
cp "agent-meter-proxy-darwin-$ARCH" "$STAGING/agent-meter-proxy"
chmod +x "$STAGING/agent-meter-proxy"

# === Create DMG ===
echo "Creating DMG package..."

# Create app bundle structure
APP_DIR="$STAGING/agent-meter-proxy.app"
CONTENTS_DIR="$APP_DIR/Contents"
MACOS_DIR="$CONTENTS_DIR/MacOS"
RESOURCES_DIR="$CONTENTS_DIR/Resources"

mkdir -p "$MACOS_DIR"
mkdir -p "$RESOURCES_DIR"

# Copy binary
cp "$STAGING/agent-meter-proxy" "$MACOS_DIR/"

# Info.plist
cat > "$CONTENTS_DIR/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>agent-meter-proxy</string>
    <key>CFBundleIdentifier</key>
    <string>io.dnor.agent-meter-proxy</string>
    <key>CFBundleName</key>
    <string>agent-meter-proxy</string>
    <key>CFBundleDisplayName</key>
    <string>agent-meter-proxy</string>
    <key>CFBundleVersion</key>
    <string>$VERSION</string>
    <key>CFBundleShortVersionString</key>
    <string>$VERSION</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleSignature</key>
    <string>????</string>
    <key>LSMinimumSystemVersion</key>
    <string>10.15</string>
    <key>LSUIElement</key>
    <true/>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSHumanReadableCopyright</key>
    <string>Copyright © 2024 DNOR. All rights reserved.</string>
</dict>
</plist>
EOF

# Create symlink to /Applications
ln -s /Applications "$STAGING/Applications"

# Create DMG using hdiutil
DMG_NAME="agent-meter-proxy-${VERSION}-${DMG_ARCH}.dmg"
hdiutil create -volname "agent-meter-proxy" \
    -fs HFS+ \
    -type UDIF \
    -size 100m \
    -folder "$STAGING" \
    -imagekey sparseband-size=32768 \
    -ov -format UDZO \
    -o "$DMG_NAME"

echo "Done! DMG created: $DMG_NAME"

# === Create PKG (optional, for Homebrew) ===
echo "Creating PKG package..."

PKG_DIR="$STAGING/pkg"
mkdir -p "$PKG_DIR/root/usr/local/bin"
mkdir -p "$PKG_DIR/root/usr/local/share/agent-meter-proxy"
mkdir -p "$PKG_DIR/scripts"

# Copy binary
cp "$STAGING/agent-meter-proxy" "$PKG_DIR/root/usr/local/bin/"

# Preinstall script
cat > "$PKG_DIR/scripts/preinstall" <<'EOF'
#!/bin/bash
# Stop existing service if running
launchctl unload ~/Library/LaunchAgents/io.dnor.agent-meter-proxy.plist 2>/dev/null || true
EOF

# Postinstall script
cat > "$PKG_DIR/scripts/postinstall" <<'EOF'
#!/bin/bash

# Generate and install CA certificate
CA_DIR="/usr/local/share/ca-certificates/agent-meter"
mkdir -p "$CA_DIR"

# Generate CA if not exists (would be done by the proxy itself)
# The proxy will generate CA on first run

# Install CA to keychain
if [ -f "$CA_DIR/agent-meter-ca.crt" ]; then
    security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain "$CA_DIR/agent-meter-ca.crt" 2>/dev/null || true
fi

# Create launchd plist for background service
mkdir -p ~/Library/LaunchAgents
cat > ~/Library/LaunchAgents/io.dnor.agent-meter-proxy.plist <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>io.dnor.agent-meter-proxy</string>
    <key>ProgramArguments</key>
    <array>
        <string>/usr/local/bin/agent-meter-proxy</string>
        <string>start</string>
        <string>--daemon</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
</dict>
</plist>
PLIST

# Load service
launchctl load ~/Library/LaunchAgents/io.dnor.agent-meter-proxy.plist 2>/dev/null || true

echo "Installation complete!"
EOF

chmod +x "$PKG_DIR/scripts/"*

# Build PKG
# Note: productbuild requires macOS
# productbuild --root "$PKG_DIR/root" --scripts "$PKG_DIR/scripts" "agent-meter-proxy-${VERSION}.pkg"

echo "Done! DMG created: $DMG_NAME"
ls -la "$DMG_NAME"