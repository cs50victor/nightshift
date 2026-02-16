use anyhow::{Context, Result};
use self_update::cargo_crate_version;

const REPO_OWNER: &str = "cs50victor";
const REPO_NAME: &str = "nightshift";
const BIN_NAME: &str = "nightshift-daemon";

/// Check GitHub Releases for a newer stable version. If found, download and
/// replace the current binary on disk. Returns true if an update was applied.
///
/// NOTE(victor): self_update is synchronous -- must wrap in spawn_blocking.
/// Every async project using self_update does this (Dioxus, feroxbuster,
/// aliyundrive-webdav, moonup).
pub async fn check_and_apply() -> Result<bool> {
    tokio::task::spawn_blocking(do_update)
        .await
        .context("update task panicked")?
}

fn do_update() -> Result<bool> {
    let current = cargo_crate_version!();
    tracing::info!("checking for updates (current: v{})", current);

    let mut builder = self_update::backends::github::Update::configure();
    builder
        .repo_owner(REPO_OWNER)
        .repo_name(REPO_NAME)
        .bin_name(BIN_NAME)
        .current_version(current)
        .no_confirm(true)
        .show_download_progress(false);

    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        if !token.is_empty() {
            builder.auth_token(&token);
        }
    }

    let updater = builder.build().context("failed to build updater")?;

    let latest = match updater.get_latest_release() {
        Ok(release) => release,
        Err(e) => {
            tracing::warn!("failed to fetch latest release: {}", e);
            return Ok(false);
        }
    };

    // NOTE(victor): self_update has no built-in pre-release filter (issue #137).
    // Parse with semver and skip if pre-release field is non-empty.
    let latest_ver = semver::Version::parse(&latest.version)
        .with_context(|| format!("invalid version in release: {}", latest.version))?;

    if !latest_ver.pre.is_empty() {
        tracing::debug!("skipping pre-release: {}", latest.version);
        return Ok(false);
    }

    let current_ver = semver::Version::parse(current).context("invalid current version")?;

    if latest_ver <= current_ver {
        tracing::info!("up to date (latest: v{})", latest.version);
        return Ok(false);
    }

    tracing::info!("update available: v{} -> v{}", current, latest.version);

    let status = updater.update().context("failed to apply update")?;

    if status.updated() {
        tracing::info!("updated to v{}", status.version());
        Ok(true)
    } else {
        Ok(false)
    }
}

pub fn is_enabled() -> bool {
    std::env::var("NIGHTSHIFT_NO_UPDATE").is_err()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_version_is_valid_semver() {
        let v = cargo_crate_version!();
        assert!(
            semver::Version::parse(v).is_ok(),
            "CARGO_PKG_VERSION '{}' is not valid semver",
            v
        );
    }

    #[test]
    fn pre_release_detected() {
        let v = semver::Version::parse("1.0.0-alpha.1").unwrap();
        assert!(!v.pre.is_empty());

        let v = semver::Version::parse("1.0.0").unwrap();
        assert!(v.pre.is_empty());
    }

    #[test]
    fn version_comparison() {
        let current = semver::Version::parse("0.0.0").unwrap();
        let newer = semver::Version::parse("0.1.0").unwrap();
        let same = semver::Version::parse("0.0.0").unwrap();

        assert!(newer > current);
        assert!(same <= current);
    }
}
