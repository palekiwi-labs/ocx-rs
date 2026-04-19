use crate::config::Config;
use crate::nix::DockerCliClient;
use crate::shadow_mounts::{build_shadow_mount_args, resolve_shadow_mounts};
use crate::user::ResolvedUser;
use crate::workspace::ResolvedWorkspace;
use anyhow::Result;
use std::collections::HashMap;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn handle_opencode(config: &Config, extra_args: Vec<String>) -> Result<()> {
    let user = crate::user::get_user()?;
    let workspace = crate::workspace::get_workspace(&user.username)?;

    let docker = DockerCliClient;
    crate::nix::ensure_running(&docker, config)?;
    crate::nix::build_dev(
        &docker,
        config,
        &user,
        &config.opencode_version,
        crate::nix::BuildOptions {
            force: false,
            no_cache: false,
        },
    )?;

    let port = match config.port {
        Some(p) => p,
        None => crate::commands::port::calculate_port()?,
    };

    let host_env: HashMap<String, String> = std::env::vars().collect();
    let image_tag = crate::nix::dev_image::get_image_tag(&config.opencode_version);
    let has_personal_flake = expand_tilde("~/.config/ocx/nix/flake.nix").exists();

    // Determine volume base name if not 'never'
    let volume_base = if config.data_volumes_mode == "never" {
        None
    } else {
        // Fallback to name or just an arbitrary project name for now
        // A complete `git` / `always` implementation can be added in a separate PR
        config.data_volumes_name.clone()
    };

    let args = build_run_args(
        &workspace,
        &user,
        config,
        volume_base.as_deref(),
        &host_env,
        &image_tag,
        has_personal_flake,
        &extra_args,
        port,
    );

    let mut cmd = Command::new("docker");
    cmd.args(args);

    let err = cmd.exec();
    Err(anyhow::Error::from(err).context("Failed to exec docker run"))
}

fn expand_tilde(path: &str) -> PathBuf {
    if path.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(&path[2..]);
        }
    }
    PathBuf::from(path)
}

/// Builds the `docker run` arguments based on the resolved context.
#[allow(clippy::too_many_arguments)]
pub fn build_run_args(
    workspace: &ResolvedWorkspace,
    user: &ResolvedUser,
    config: &Config,
    volume_base: Option<&str>,
    host_env: &HashMap<String, String>,
    image_tag: &str,
    has_personal_flake: bool,
    extra_args: &[String],
    project_hash_port: u16,
) -> Vec<String> {
    let mut args = vec!["run".to_string(), "--rm".to_string(), "-it".to_string()];

    // Workdir
    args.push("--workdir".to_string());
    args.push(workspace.container_path.to_string_lossy().to_string());

    // Name
    let project_name = workspace
        .root
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();
    args.push("--name".to_string());
    args.push(format!("ocx-{}-{}", project_name, project_hash_port));

    // Resources
    args.push("--memory".to_string());
    args.push(config.memory.clone());

    args.push("--cpus".to_string());
    args.push(config.cpus.to_string());

    args.push("--pids-limit".to_string());
    args.push(config.pids_limit.to_string());

    // Security Hardening
    args.push("--cap-drop".to_string());
    args.push("ALL".to_string());

    args.push("--security-opt".to_string());
    args.push("no-new-privileges".to_string());

    if config.read_only {
        args.push("--read-only".to_string());
    }

    // Networking
    args.push("--network".to_string());
    args.push(config.network.clone());

    if config.publish_port {
        args.push("-p".to_string());
        let port = config.port.unwrap_or(project_hash_port);
        args.push(format!("{}:80", port));
    }

    if config.add_host_docker_internal {
        args.push("--add-host".to_string());
        args.push("host.docker.internal:host-gateway".to_string());
    }

    // Environment Variables
    args.push("-e".to_string());
    args.push(format!("USER={}", user.username));

    if let Some(tz) = &config.timezone {
        args.push("-e".to_string());
        args.push(format!("TZ={}", tz));
    } else if let Some(tz) = host_env.get("TZ") {
        args.push("-e".to_string());
        args.push(format!("TZ={}", tz));
    }

    for key in ["TERM", "COLORTERM", "FORCE_COLOR"] {
        if let Some(val) = host_env.get(key) {
            args.push("-e".to_string());
            args.push(format!("{}={}", key, val));
        }
    }

    args.push("-e".to_string());
    args.push("TMPDIR=/workspace/tmp".to_string());

    // Volumes
    let host_config_dir = expand_tilde(&config.opencode_config_dir);
    let container_config_dir = format!("/home/{}/.config/opencode", user.username);

    // Avoid mounting workspace twice if config is the workspace
    if host_config_dir != workspace.root {
        args.push("-v".to_string());
        args.push(format!(
            "{}:{}:rw",
            workspace.root.to_string_lossy(),
            workspace.container_path.to_string_lossy()
        ));
    }

    args.push("-v".to_string());
    args.push(format!(
        "{}:{}:rw",
        host_config_dir.to_string_lossy(),
        container_config_dir
    ));

    if has_personal_flake {
        let host_flake_dir = expand_tilde("~/.config/ocx/nix");
        let container_flake_dir = format!("/home/{}/.config/ocx/nix", user.username);
        args.push("-v".to_string());
        args.push(format!(
            "{}:{}:rw",
            host_flake_dir.to_string_lossy(),
            container_flake_dir
        ));
    }

    // Cache and Local Data
    if let Some(base) = volume_base {
        args.push("-v".to_string());
        args.push(format!("{}-cache:/home/{}/.cache:rw", base, user.username));

        args.push("-v".to_string());
        args.push(format!("{}-local:/home/{}/.local:rw", base, user.username));
    }

    // Nix Store
    args.push("-v".to_string());
    args.push(format!("{}:/nix:ro", config.nix_volume_name));

    // Tmpfs
    args.push("--tmpfs".to_string());
    args.push(format!(
        "/tmp:exec,nosuid,size={},uid={},gid={}",
        config.tmp_size, user.uid, user.gid
    ));

    args.push("--tmpfs".to_string());
    args.push(format!(
        "/workspace/tmp:exec,nosuid,size={},uid={},gid={}",
        config.workspace_tmp_size, user.uid, user.gid
    ));

    // Time
    if Path::new("/etc/localtime").exists() {
        args.push("-v".to_string());
        args.push("/etc/localtime:/etc/localtime:ro".to_string());
    }

    // Shadow Mounts
    let shadow_mounts = resolve_shadow_mounts(&config.forbidden_paths, workspace);
    let shadow_args = build_shadow_mount_args(&shadow_mounts);
    args.extend(shadow_args);

    // Image
    args.push(image_tag.to_string());

    // Command
    if has_personal_flake {
        args.push("nix".to_string());
        args.push("develop".to_string());
        args.push(format!("/home/{}/.config/ocx/nix", user.username));
        args.push("-c".to_string());
    }

    args.extend(config.opencode_command.clone());
    args.extend_from_slice(extra_args);

    args
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_workspace() -> ResolvedWorkspace {
        ResolvedWorkspace {
            root: PathBuf::from("/home/user/projects/my-app"),
            container_path: PathBuf::from("/workspace/my-app"),
        }
    }

    fn mock_user() -> ResolvedUser {
        ResolvedUser {
            username: "testuser".to_string(),
            uid: 1000,
            gid: 1000,
        }
    }

    fn mock_config() -> Config {
        let mut cfg = Config::default();
        cfg.memory = "2g".to_string();
        cfg.cpus = 1.5;
        cfg.pids_limit = 200;
        cfg.read_only = true;
        cfg.opencode_config_dir = "/home/user/.config/opencode".to_string();
        cfg.nix_volume_name = "test-nix-vol".to_string();
        cfg.tmp_size = "1g".to_string();
        cfg.workspace_tmp_size = "2g".to_string();
        cfg
    }

    #[test]
    fn test_build_base_args() {
        let workspace = mock_workspace();
        let user = mock_user();
        let config = Config::default();
        let host_env = HashMap::new();
        let port = 8080;

        let args = build_run_args(
            &workspace,
            &user,
            &config,
            None,
            &host_env,
            "my-image:latest",
            false,
            &[],
            port,
        );

        let expected_base = vec![
            "run",
            "--rm",
            "-it",
            "--workdir",
            "/workspace/my-app",
            "--name",
            "ocx-my-app-8080",
        ];

        // Ensure it starts with base args
        assert_eq!(&args[0..expected_base.len()], &expected_base[..]);
    }

    #[test]
    fn test_build_resource_and_security_args() {
        let workspace = mock_workspace();
        let user = mock_user();
        let config = mock_config();
        let host_env = HashMap::new();
        let port = 8080;

        let args = build_run_args(
            &workspace,
            &user,
            &config,
            None,
            &host_env,
            "my-image:latest",
            false,
            &[],
            port,
        );

        // Verify resource limits
        assert!(args.windows(2).any(|w| w == ["--memory", "2g"]));
        assert!(args.windows(2).any(|w| w == ["--cpus", "1.5"]));
        assert!(args.windows(2).any(|w| w == ["--pids-limit", "200"]));

        // Verify security hardening
        assert!(args.windows(2).any(|w| w == ["--cap-drop", "ALL"]));
        assert!(args
            .windows(2)
            .any(|w| w == ["--security-opt", "no-new-privileges"]));
        assert!(args.contains(&"--read-only".to_string()));
    }

    #[test]
    fn test_build_volume_args() {
        let workspace = mock_workspace();
        let user = mock_user();
        let config = mock_config();
        let host_env = HashMap::new();
        let port = 8080;

        let args = build_run_args(
            &workspace,
            &user,
            &config,
            Some("test-vol"),
            &host_env,
            "my-image:latest",
            false,
            &[],
            port,
        );

        // Workspace and Config mounts
        assert!(args
            .windows(2)
            .any(|w| w == ["-v", "/home/user/projects/my-app:/workspace/my-app:rw"]));
        assert!(args.windows(2).any(|w| w
            == [
                "-v",
                "/home/user/.config/opencode:/home/testuser/.config/opencode:rw"
            ]));

        // Cache and Local
        assert!(args
            .windows(2)
            .any(|w| w == ["-v", "test-vol-cache:/home/testuser/.cache:rw"]));
        assert!(args
            .windows(2)
            .any(|w| w == ["-v", "test-vol-local:/home/testuser/.local:rw"]));

        // Nix Store
        assert!(args.windows(2).any(|w| w == ["-v", "test-nix-vol:/nix:ro"]));

        // Tmpfs
        assert!(args
            .windows(2)
            .any(|w| w == ["--tmpfs", "/tmp:exec,nosuid,size=1g,uid=1000,gid=1000"]));
        assert!(args.windows(2).any(|w| w
            == [
                "--tmpfs",
                "/workspace/tmp:exec,nosuid,size=2g,uid=1000,gid=1000"
            ]));
    }

    #[test]
    fn test_build_network_and_env_args() {
        let workspace = mock_workspace();
        let user = mock_user();
        let mut config = mock_config();
        config.network = "my-network".to_string();
        config.publish_port = true;
        config.add_host_docker_internal = true;

        let mut host_env = HashMap::new();
        host_env.insert("TZ".to_string(), "Europe/Paris".to_string());
        host_env.insert("TERM".to_string(), "xterm-256color".to_string());

        let port = 8080;

        let args = build_run_args(
            &workspace,
            &user,
            &config,
            None,
            &host_env,
            "my-image:latest",
            false,
            &[],
            port,
        );

        // Networking
        assert!(args.windows(2).any(|w| w == ["--network", "my-network"]));
        assert!(args.windows(2).any(|w| w == ["-p", "8080:80"]));
        assert!(args
            .windows(2)
            .any(|w| w == ["--add-host", "host.docker.internal:host-gateway"]));

        // Environment
        assert!(args.windows(2).any(|w| w == ["-e", "USER=testuser"]));
        assert!(args.windows(2).any(|w| w == ["-e", "TZ=Europe/Paris"]));
        assert!(args.windows(2).any(|w| w == ["-e", "TERM=xterm-256color"]));
        assert!(args
            .windows(2)
            .any(|w| w == ["-e", "TMPDIR=/workspace/tmp"]));
    }

    #[test]
    fn test_image_and_command_args() {
        let workspace = mock_workspace();
        let user = mock_user();
        let config = mock_config();
        let host_env = HashMap::new();
        let port = 8080;

        // 1. Without flake
        let args = build_run_args(
            &workspace,
            &user,
            &config,
            None,
            &host_env,
            "my-image:latest",
            false,
            &["--extra".to_string()],
            port,
        );

        let expected_tail = vec!["my-image:latest", "opencode", "--extra"];
        assert_eq!(
            &args[args.len() - expected_tail.len()..],
            &expected_tail[..]
        );

        // 2. With flake
        let args = build_run_args(
            &workspace,
            &user,
            &config,
            None,
            &host_env,
            "my-image:latest",
            true,
            &["--extra".to_string()],
            port,
        );

        // check volume mount added
        let home_flake = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("~"))
            .join(".config/ocx/nix");
        let vol_str = format!(
            "{}:/home/testuser/.config/ocx/nix:rw",
            home_flake.to_string_lossy()
        );
        assert!(args.windows(2).any(|w| w == ["-v", &vol_str]));

        let expected_tail = vec![
            "my-image:latest",
            "nix",
            "develop",
            "/home/testuser/.config/ocx/nix",
            "-c",
            "opencode",
            "--extra",
        ];
        assert_eq!(
            &args[args.len() - expected_tail.len()..],
            &expected_tail[..]
        );
    }
}
