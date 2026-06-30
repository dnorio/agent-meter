# agent-meter-proxy Windows installer (WiX)

Two installer definitions live here:

| File | Toolchain | Produces | Notes |
| ---- | --------- | -------- | ----- |
| `ProductUI.wxs` | WiX v5 + `WixToolset.UI.wixext` + `WixToolset.Util.wixext` (**Windows only**) | Full **wizard** MSI | License/EULA page, folder selection, branded banner/dialog, custom feature tree with optional CA install, env vars, Windows service and desktop shortcut. Built in CI on `windows-latest`. |
| `Product.wixl.wxs` | `wixl` / `msitools` (**Linux**) | Minimal silent MSI | Fallback only. No wizard UI (wixl cannot render WixUI dialogs). |

The published release MSIs (`agent-meter-proxy-1.2.4-x64.msi`,
`agent-meter-proxy-1.2.4-arm64.msi`) come from **`ProductUI.wxs`** via the
`package-windows` job in `.github/workflows/release-agent-meter-proxy.yml`.

## Wizard flow (ProductUI.wxs)

1. **Welcome** — branded left image (`assets/dialog.bmp`).
2. **License Agreement** — `License.rtf`, requires explicit accept.
3. **Custom Setup** — feature tree; user can toggle and change install folder:
   - Install local CA certificate (runs `agent-meter-proxy setup`)
   - Configure `HTTPS_PROXY` / `HTTP_PROXY` (user scope)
   - Install + start the Windows service
   - Desktop shortcut
4. **Ready / Progress / Finish.**

Branding assets (`assets/icon.ico`, `assets/banner.bmp`, `assets/dialog.bmp`) are
generated from the product logo (`crates/collector/ui/_static/favicon.svg`)
using `scripts/release/wix/gen_assets.py`.

## Build locally on Windows

```pwsh
dotnet tool install --global wix --version 5.0.0
wix extension add -g WixToolset.UI.wixext/5.0.0
wix extension add -g WixToolset.Util.wixext/5.0.0

# place the proxy exe next to the wxs (downloaded from the release or built)
wix build ProductUI.wxs `
  -ext WixToolset.UI.wixext -ext WixToolset.Util.wixext `
  -arch x64 -d "ProxyExe=agent-meter-proxy-windows-x86_64.exe" `
  -o agent-meter-proxy-1.2.3-x64.msi
```

## Code signing

The CI `package-windows` job signs every MSI with Authenticode. For release tags
(`agent-meter-proxy-v*`), signing is **mandatory** and the workflow fails if these
repository secrets are missing:

- `WINDOWS_CODESIGN_PFX_BASE64` — base64 of the Authenticode `.pfx`
- `WINDOWS_CODESIGN_PASSWORD` — the `.pfx` password

Unsigned MSI output is allowed only for non-release test builds. Use an EV/OV
code-signing certificate to reduce Windows SmartScreen friction.
