use anyhow::Result;

use crate::config::Config;
use crate::dev;
use crate::dev::container_name::resolve_container_name;
use crate::dev::env_passthrough::build_passthrough_env_args;
use crate::dev::workspace::{get_workspace, ResolvedWorkspace};
use crate::docker::args::build_run_args;
use crate::docker::client::DockerClient;
use crate::nix;
use crate::opencode;
use crate::user::{get_user, ResolvedUser};

/// Orchestrate and run an OpenCode session.
pub fn run_opencode(config: &Config, extra_args: Vec<String>) -> Result<()> {
    let docker = DockerClient;
    let user = get_user()?;
    let workspace = get_workspace(&user.username)?;

    // Ensure the Nix daemon is running.
    nix::ensure_running(&docker, config)?;

    // Resolve version and ensure the dev image exists.
    let version = opencode::resolve_version(config)?;
    let image_tag = dev::image::get_image_tag(&version);
    dev::ensure_dev_image(&docker, config, &user, &version)?;

    // Resolve port and container name.
    let port = dev::port::resolve_port(config)?;
    let opencode_config_dir = opencode::ensure_config_dir()?;
    let cwd_basename = workspace
        .root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("ocx");
    let container_name = resolve_container_name(config, cwd_basename, port);

    // Build docker run flags.
    let opts = build_run_opts(config, &user, &workspace, &opencode_config_dir, port);

    // Build the full command.
    let mut cmd = config.opencode_command.clone();
    cmd.extend(extra_args);

    // Exec into the container.
    let docker_args = build_run_args(&container_name, &image_tag, opts, Some(cmd));
    Err(docker.exec_command(docker_args))
}

use std::path::Path;

/// Build the full set of Docker run flags for an OpenCode session.
pub fn build_run_opts(
    config: &Config,
    user: &ResolvedUser,
    workspace: &ResolvedWorkspace,
    opencode_config_dir: &Path,
    port: u16,
) -> Vec<String> {
    let mut opts: Vec<String> = vec![
        "--rm".to_string(),
        "-it".to_string(),
        // Security hardening
        "--security-opt".to_string(),
        "no-new-privileges".to_string(),
        "--cap-drop".to_string(),
        "ALL".to_string(),
        // Resource constraints
        "--memory".to_string(),
        config.memory.clone(),
        "--cpus".to_string(),
        config.cpus.to_string(),
        "--pids-limit".to_string(),
        config.pids_limit.to_string(),
        // Network
        "--network".to_string(),
        config.network.clone(),
    ];

    // Port publishing.
    if config.publish_port {
        opts.push("-p".to_string());
        opts.push(format!("{}:80", port));
    }

    // Environment: user identity and terminal capabilities.
    opts.extend([
        "-e".to_string(),
        format!("USER={}", user.username),
        "-e".to_string(),
        "TERM=xterm-256color".to_string(),
        "-e".to_string(),
        "COLORTERM=truecolor".to_string(),
        "-e".to_string(),
        "FORCE_COLOR=1".to_string(),
    ]);

    // LLM API keys and OpenCode-specific env vars present on the host.
    opts.extend(build_passthrough_env_args());

    // Workspace bind mount.
    opts.extend([
        "-v".to_string(),
        format!(
            "{}:{}:rw",
            workspace.root.display(),
            workspace.container_path.display()
        ),
        "--workdir".to_string(),
        workspace.container_path.to_string_lossy().into_owned(),
    ]);

    // OpenCode config directory bind mount.
    opts.extend([
        "-v".to_string(),
        format!(
            "{}:/home/{}/.config/opencode:rw",
            opencode_config_dir.display(),
            user.username
        ),
    ]);

    opts
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_build_run_opts_basic() {
        let config = Config::default();
        let user = ResolvedUser {
            username: "alice".to_string(),
            uid: 1000,
            gid: 1000,
        };
        let workspace = ResolvedWorkspace {
            root: PathBuf::from("/home/alice/project"),
            container_path: PathBuf::from("/home/alice/project"),
        };
        let opencode_config_dir = PathBuf::from("/home/alice/.config/opencode");
        let port = 32768;

        let opts = build_run_opts(&config, &user, &workspace, &opencode_config_dir, port);

        // Check for key flags
        assert!(opts.contains(&"--rm".to_string()));
        assert!(opts.contains(&"-it".to_string()));
        assert!(opts.contains(&"no-new-privileges".to_string()));
        assert!(opts.contains(&"USER=alice".to_string()));
        assert!(opts.contains(&"/home/alice/project:/home/alice/project:rw".to_string()));
        assert!(opts
            .contains(&"/home/alice/.config/opencode:/home/alice/.config/opencode:rw".to_string()));

        // Port check
        if config.publish_port {
            assert!(opts.contains(&"32768:80".to_string()));
        }
    }
}
