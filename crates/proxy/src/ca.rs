use anyhow::{Context, Result};
use rcgen::{BasicConstraints, CertificateParams, DistinguishedName, DnType, IsCa, KeyPair};
use std::path::{Path, PathBuf};

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
pub fn generate_ca(dir: &Path) -> Result<(PathBuf, PathBuf)> {
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
pub fn install_system_ca(cert_path: &Path) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        let dest = PathBuf::from("/usr/local/share/ca-certificates/agent-meter-proxy.crt");
        eprintln!("  Installing CA to {} (requires sudo)", dest.display());
        let status = std::process::Command::new("sudo")
            .arg("cp")
            .arg(cert_path)
            .arg(&dest)
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
                "add-trusted-cert",
                "-d",
                "-r",
                "trustRoot",
                "-k",
                "/Library/Keychains/System.keychain",
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "agent-meter-proxy-ca-test-{}-{nanos}",
            std::process::id()
        ))
    }

    #[test]
    fn generate_ca_creates_key_and_cert_files() {
        let dir = unique_temp_dir();
        std::fs::create_dir_all(&dir).expect("temp dir should be created");

        let result = generate_ca(&dir).expect("ca generation should succeed");

        assert_eq!(result.0, dir.join("ca-key.pem"));
        assert_eq!(result.1, dir.join("ca-cert.pem"));
        assert!(result.0.exists());
        assert!(result.1.exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn generate_ca_is_idempotent_when_files_already_exist() {
        let dir = unique_temp_dir();
        std::fs::create_dir_all(&dir).expect("temp dir should be created");

        let first = generate_ca(&dir).expect("initial generation should succeed");
        let cert_before = std::fs::read_to_string(&first.1).expect("cert should be readable");

        let second = generate_ca(&dir).expect("second generation should succeed");
        let cert_after = std::fs::read_to_string(&second.1).expect("cert should be readable");

        assert_eq!(first, second);
        assert_eq!(cert_before, cert_after);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
