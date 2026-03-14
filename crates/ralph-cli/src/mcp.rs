use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
pub struct McpArgs {
    #[command(subcommand)]
    pub command: McpCommands,
}

#[derive(Subcommand, Debug)]
pub enum McpCommands {
    /// Run the Ralph control plane as an MCP server over stdio
    Serve(ServeArgs),
}

#[derive(Args, Debug, Default)]
pub struct ServeArgs {
    /// Workspace root directory for config, tasks, loops, and planning state
    ///
    /// Precedence: CLI flag > RALPH_API_WORKSPACE_ROOT > current working directory.
    #[arg(long)]
    pub workspace_root: Option<PathBuf>,
}

pub async fn execute(args: McpArgs) -> Result<()> {
    match args.command {
        McpCommands::Serve(args) => {
            let mut config = ralph_api::ApiConfig::from_env()?;
            config.served_by = "ralph-mcp".to_string();
            config.auth_mode = ralph_api::AuthMode::TrustedLocal;
            config.token = None;
            if let Some(workspace_root) = resolve_workspace_root(args.workspace_root)? {
                config.workspace_root = workspace_root;
            }
            ralph_api::serve_stdio(config).await
        }
    }
}

fn resolve_workspace_root(workspace_root: Option<PathBuf>) -> Result<Option<PathBuf>> {
    let Some(workspace_root) = workspace_root else {
        return Ok(None);
    };

    let resolved = if workspace_root.is_absolute() {
        workspace_root
    } else {
        std::env::current_dir()?.join(workspace_root)
    };

    Ok(Some(resolved))
}

#[cfg(test)]
mod tests {
    use super::resolve_workspace_root;
    use crate::test_support::CwdGuard;
    use anyhow::Result;
    use std::path::PathBuf;

    #[test]
    fn resolve_workspace_root_returns_none_when_unset() -> Result<()> {
        assert_eq!(resolve_workspace_root(None)?, None);
        Ok(())
    }

    #[test]
    fn resolve_workspace_root_preserves_absolute_paths() -> Result<()> {
        let path = PathBuf::from("/tmp/ralph-workspace");
        assert_eq!(resolve_workspace_root(Some(path.clone()))?, Some(path));
        Ok(())
    }

    #[test]
    fn resolve_workspace_root_resolves_relative_paths_from_current_dir() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let _guard = CwdGuard::set(temp_dir.path());

        let resolved = resolve_workspace_root(Some(PathBuf::from("nested/workspace")))?;
        assert_eq!(resolved, Some(temp_dir.path().join("nested/workspace")));
        Ok(())
    }
}
