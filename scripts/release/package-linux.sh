#!/bin/bash
# Linux packaging script for agent-meter-proxy
# Creates DEB, RPM, and TGZ packages

set -e

VERSION="${1:-1.2.3}"
ARCH="${2:-x86_64}"

# Detect architecture
case "$ARCH" in
    x86_64|amd64)
        DEB_ARCH="amd64"
        RPM_ARCH="x86_64"
        ;;
    aarch64|arm64)
        DEB_ARCH="arm64"
        RPM_ARCH="aarch64"
        ;;
    *)
        echo "Unknown architecture: $ARCH"
        exit 1
        ;;
esac

# Create staging directory
STAGING="/tmp/agent-meter-proxy-pkg"
rm -rf "$STAGING"
mkdir -p "$STAGING"

# Copy binary
cp "agent-meter-proxy-linux-$ARCH" "$STAGING/agent-meter-proxy"
chmod +x "$STAGING/agent-meter-proxy"

# === Create DEB package ===
echo "Creating DEB package..."
DEB_DIR="$STAGING/deb"
mkdir -p "$DEB_DIR/DEBIAN"
mkdir -p "$DEB_DIR/usr/bin"
mkdir -p "$DEB_DIR/usr/share/doc/agent-meter-proxy"
mkdir -p "$DEB_DIR/usr/share/man/man1"

# Copy binary
cp "$STAGING/agent-meter-proxy" "$DEB_DIR/usr/bin/"

# Control file
cat > "$DEB_DIR/DEBIAN/control" <<EOF
Package: agent-meter-proxy
Version: $VERSION
Section: net
Priority: optional
Architecture: $DEB_ARCH
Depends: libc6 (>= 2.17)
Maintainer: DNOR <agent-meter@example.com>
Description: HTTPS proxy for AI IDE & CLI telemetry capture
 agent-meter-proxy captures LLM calls from VS Code, Cursor, Eclipse,
 Claude Code, Copilot CLI, Codex CLI and any HTTPS-based AI tool.
EOF

# Postinst script (install CA)
cat > "$DEB_DIR/DEBIAN/postinst" <<'EOF'
#!/bin/bash
set -e

# Generate and install CA certificate
CA_DIR="/usr/local/share/ca-certificates/agent-meter"
mkdir -p "$CA_DIR"

# Generate CA if not exists
if [ ! -f "$CA_DIR/agent-meter-ca.crt" ]; then
    echo "Generating CA certificate..."
    # CA generation would happen here
fi

# Install CA
update-ca-certificates 2>/dev/null || true

# Create systemd service if systemd is available
if command -v systemctl &> /dev/null; then
    cat > /etc/systemd/system/agent-meter-proxy.service <<'SERVICE'
[Unit]
Description=agent-meter-proxy HTTPS proxy
After=network.target

[Service]
Type=simple
ExecStart=/usr/bin/agent-meter-proxy start --daemon
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
SERVICE
    systemctl daemon-reload
    systemctl enable agent-meter-proxy 2>/dev/null || true
fi

echo "Installation complete. Run 'agent-meter-proxy setup' to configure."
EOF
chmod +x "$DEB_DIR/DEBIAN/postinst"

# Prerm script (stop service)
cat > "$DEB_DIR/DEBIAN/prerm" <<'EOF'
#!/bin/bash
set -e
systemctl stop agent-meter-proxy 2>/dev/null || true
systemctl disable agent-meter-proxy 2>/dev/null || true
EOF
chmod +x "$DEB_DIR/DEBIAN/prerm"

# Copyright
cat > "$DEB_DIR/usr/share/doc/agent-meter-proxy/copyright" <<EOF
Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/
Source: https://github.com/dnorio/agent-meter
Copyright: 2024 DNOR
License: MIT
EOF
gzip -n -9 "$DEB_DIR/usr/share/doc/agent-meter-proxy/copyright"

# Build DEB
dpkg-deb --build "$DEB_DIR" "agent-meter-proxy_${VERSION}_${DEB_ARCH}.deb"

# === Create RPM package ===
echo "Creating RPM package..."
RPM_DIR="$STAGING/rpm"
mkdir -p "$RPM_DIR/BUILD"
mkdir -p "$RPM_DIR/RPMS"
mkdir -p "$RPM_DIR/SPECS"
mkdir -p "$RPM_DIR/SOURCES/agent-meter-proxy"

# Copy binary
cp "$STAGING/agent-meter-proxy" "$RPM_DIR/SOURCES/agent-meter-proxy/"

# Spec file
cat > "$RPM_DIR/SPECS/agent-meter-proxy.spec" <<EOF
Name:           agent-meter-proxy
Version:        $VERSION
Release:        1%{?dist}
Summary:        HTTPS proxy for AI IDE & CLI telemetry capture
License:        MIT
URL:            https://github.com/dnorio/agent-meter
BuildArch:      $RPM_ARCH

%description
agent-meter-proxy captures LLM calls from VS Code, Cursor, Eclipse,
Claude Code, Copilot CLI, Codex CLI and any HTTPS-based AI tool.

%install
mkdir -p %{buildroot}/usr/bin
mkdir -p %{buildroot}/usr/local/share/ca-certificates/agent-meter
cp %{_sourcedir}/agent-meter-proxy/agent-meter-proxy %{buildroot}/usr/bin/
chmod +x %{buildroot}/usr/bin/agent-meter-proxy

%files
%attr(755, root, root) /usr/bin/agent-meter-proxy

%post
# Install CA certificate
update-ca-certificates 2>/dev/null || true

# Create systemd service
cat > /etc/systemd/system/agent-meter-proxy.service <<'SERVICE'
[Unit]
Description=agent-meter-proxy HTTPS proxy
After=network.target

[Service]
Type=simple
ExecStart=/usr/bin/agent-meter-proxy start --daemon
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
SERVICE
systemctl daemon-reload
systemctl enable agent-meter-proxy 2>/dev/null || true

%preun
systemctl stop agent-meter-proxy 2>/dev/null || true
systemctl disable agent-meter-proxy 2>/dev/null || true

%changelog
* Sat Jun 28 2026 DNOR <agent-meter@example.com> - $VERSION-1
- Initial release
EOF

# Build RPM
rpmbuild --define "_topdir $RPM_DIR" -bb "$RPM_DIR/SPECS/agent-meter-proxy.spec"
mv "$RPM_DIR/RPMS/"*/*.rpm "agent-meter-proxy-${VERSION}-1.${DEB_ARCH}.rpm" 2>/dev/null || true

# === Create TGZ (portable) ===
echo "Creating TGZ package..."
tar -czvf "agent-meter-proxy-${VERSION}-${ARCH}.tar.gz" -C "$STAGING" agent-meter-proxy

echo "Done! Packages created:"
ls -la *.deb *.rpm *.tar.gz 2>/dev/null || true