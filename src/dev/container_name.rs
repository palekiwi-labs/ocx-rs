use crate::config::Config;

/// Resolve the Docker container name for the opencode session.
///
/// - If `cfg.container_name` is set: `"{name}-{port}"`
/// - Otherwise: `"ocx-{basename}-{port}"` where `basename` is the name of the
///   current working directory.
///
/// `cwd_basename` is injected by the caller (from `std::env::current_dir()`)
/// so that this function remains pure and fully unit-testable.
pub fn resolve_container_name(cfg: &Config, cwd_basename: &str, port: u16) -> String {
    match &cfg.container_name {
        Some(name) => format!("{}-{}", name, port),
        None => format!("ocx-{}-{}", cwd_basename, port),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_custom_container_name_with_port() {
        let cfg = Config {
            container_name: Some("my-project".to_string()),
            ..Default::default()
        };
        assert_eq!(
            resolve_container_name(&cfg, "irrelevant", 8080),
            "my-project-8080",
        );
    }

    #[test]
    fn test_default_uses_basename_and_port() {
        let cfg = Config::default();
        assert_eq!(
            resolve_container_name(&cfg, "my-app", 8080),
            "ocx-my-app-8080",
        );
    }
}
