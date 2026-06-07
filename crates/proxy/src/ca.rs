use std::path::PathBuf;
use anyhow::{Context, Result};
use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair, IsCa, BasicConstraints};

/// Returns the CA directory (~/.agent-meter/)
pub fn ca_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".agent-meter")
}

/// Returns (key_path, cert_path)
pub fn ca_paths() -> (PathBuf, PathBuf) {
    let dir = ca_dir();
    (dir.join("ca-key.pem"), dir.join("ca-cert.pem"))
}

/// Generate a new CA key + certificate
pub fn generate_ca(dir: &PathBuf) -> Result<(PathBuf, PathBuf)> {
    let key_path = dir.join("ca-key.pem");
    let cert_path = dir.join("ca-cert.pem");

    if cert_path.exists() && key_path.exists() {
        eprintln!("  CA already exists, skipping generation");
        return Ok((key_path, cert_path));
    }

    let mut params = CertificateParams::default();
    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, "agent-meter-proxy CA");
    dn.push(DnType::OrganizationName, "agent-meter");
    params.distinguished_name = dn;
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    // Valid for 10 years
    params.not_before = time::OffsetDateTime::now_utc();
    params.not_after = time::OffsetDateTime::now_utc() + time::Duration::days(3650);

    let key_pair = KeyPair::generate().context("generating CA key")?;
    let cert = params.self_signed(&key_pair).context("self-signing CA")?;

    std::fs::write(&key_path, key_pair.serialize_pem())?;
    std::fs::write(&cert_path, cert.pem())?;

    Ok((key_path, cert_path))
}

/// Install CA certificate into the system trust store
pub fn install_system_ca(cert_path: &PathBuf) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        let dest = PathBuf::from("/usr/local/share/ca-certificates/agent-meter-proxy.crt");
        eprintln!("  Installing CA to {} (requires sudo)", dest.display());
        let status = std::process::Command::new("sudo")
            .args(["cp", &cert_path.to_string_lossy(), &dest.to_string_lossy()])
            .status()
            .context("copying CA cert")?;
        if !status.success() {
            anyhow::bail!("Failed to copy CA cert (sudo required)");
        }
        let status = std::process::Command::new("sudo")
            .args(["update-ca-certificates"])
            .status()
            .context("update-ca-certificates")?;
        if !status.success() {
            // Try RHEL/Fedora method
            let _ = std::process::Command::new("sudo")
                .args(["update-ca-trust"])
                .status();
        }
    }

    #[cfg(target_os = "macos")]
    {
        eprintln!("  Installing CA to macOS Keychain (requires password)");
        let status = std::process::Command::new("security")
            .args([
                "add-trusted-cert", "-d", "-r", "trustRoot",
                "-k", "/Library/Keychains/System.keychain",
                &cert_path.to_string_lossy(),
            ])
            .status()
            .context("installing CA in macOS Keychain")?;
        if !status.success() {
            anyhow::bail!("Failed to install CA cert in Keychain");
        }
    }

    #[cfg(target_os = "windows")]
    {
        eprintln!("  Installing CA to Windows certificate store");
        let status = std::process::Command::new("certutil")
            .args(["-addstore", "ROOT", &cert_path.to_string_lossy()])
            .status()
            .context("certutil -addstore")?;
        if !status.success() {
            anyhow::bail!("Failed to install CA cert (run as Administrator)");
        }
    }

    Ok(())
}
