//! Command-line interface for ralph-mobile-server.
//!
//! Provides argument parsing for server configuration.

use clap::Parser;

/// REST API + SSE server for mobile monitoring of Ralph orchestrator sessions.
#[derive(Parser, Debug)]
#[command(name = "ralph-mobile-server")]
#[command(about = "REST API + SSE server for mobile monitoring of Ralph orchestrator sessions")]
pub struct Args {
    /// Port to bind the server to.
    #[arg(short, long, default_value_t = 8080)]
    pub port: u16,

    /// Bind to all network interfaces (0.0.0.0) for LAN access.
    #[arg(long)]
    pub bind_all: bool,
}

impl Args {
    /// Returns the bind address based on the --bind-all flag.
    pub fn bind_address(&self) -> String {
        let host = if self.bind_all { "0.0.0.0" } else { "127.0.0.1" };
        format!("{}:{}", host, self.port)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_port() {
        let args = Args::try_parse_from(["ralph-mobile-server"]).unwrap();
        assert_eq!(args.port, 8080);
    }

    #[test]
    fn test_custom_port() {
        let args = Args::try_parse_from(["ralph-mobile-server", "--port", "9090"]).unwrap();
        assert_eq!(args.port, 9090);
    }

    #[test]
    fn test_short_port_flag() {
        let args = Args::try_parse_from(["ralph-mobile-server", "-p", "3000"]).unwrap();
        assert_eq!(args.port, 3000);
    }

    #[test]
    fn test_bind_all_default_false() {
        let args = Args::try_parse_from(["ralph-mobile-server"]).unwrap();
        assert!(!args.bind_all);
    }

    #[test]
    fn test_bind_all_flag() {
        let args = Args::try_parse_from(["ralph-mobile-server", "--bind-all"]).unwrap();
        assert!(args.bind_all);
    }

    #[test]
    fn test_bind_address_default() {
        let args = Args::try_parse_from(["ralph-mobile-server"]).unwrap();
        assert_eq!(args.bind_address(), "127.0.0.1:8080");
    }

    #[test]
    fn test_bind_address_bind_all() {
        let args = Args::try_parse_from(["ralph-mobile-server", "--bind-all"]).unwrap();
        assert_eq!(args.bind_address(), "0.0.0.0:8080");
    }

    #[test]
    fn test_bind_address_custom_port() {
        let args = Args::try_parse_from(["ralph-mobile-server", "--port", "9090"]).unwrap();
        assert_eq!(args.bind_address(), "127.0.0.1:9090");
    }

    #[test]
    fn test_bind_address_bind_all_custom_port() {
        let args =
            Args::try_parse_from(["ralph-mobile-server", "--bind-all", "--port", "9090"]).unwrap();
        assert_eq!(args.bind_address(), "0.0.0.0:9090");
    }

    #[test]
    fn test_all_flags_combined() {
        let args = Args::try_parse_from([
            "ralph-mobile-server",
            "--port",
            "3000",
            "--bind-all",
        ])
        .unwrap();
        assert_eq!(args.port, 3000);
        assert!(args.bind_all);
    }
}
