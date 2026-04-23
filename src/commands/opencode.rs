use anyhow::Result;

use crate::config::Config;
use crate::dev;

pub fn handle_opencode(config: &Config, extra_args: Vec<String>) -> Result<()> {
    dev::run_opencode(config, extra_args)
}
