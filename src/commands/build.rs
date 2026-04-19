use anyhow::Result;
use std::path::Path;

use crate::config::Config;
use crate::nix;
use crate::nix::DockerClient;
use crate::user::ResolvedUser;
use crate::version::github::VersionFetcher;
use crate::version::resolve_version;

pub fn handle_build<D: DockerClient, F: VersionFetcher>(
    docker: &D,
    cfg: &Config,
    user: &ResolvedUser,
    fetcher: &F,
    cache_path: &Path,
    base: bool,
    force: bool,
    no_cache: bool,
) -> Result<()> {
    let version = resolve_version(
        &cfg.opencode_version,
        cfg.version_cache_ttl_hours,
        cache_path,
        fetcher,
    )?;

    if base {
        nix::build(docker)?;
    }

    nix::build_dev(docker, cfg, user, &version, force, no_cache)?;

    Ok(())
}
