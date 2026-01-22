//! # ralph-e2e
//!
//! End-to-end test harness for the Ralph Orchestrator.
//!
//! This binary validates Ralph's behavior against real AI backends (Claude, Kiro, OpenCode).
//! It exercises the full orchestration loop including:
//! - Backend connectivity and authentication
//! - Event parsing and routing
//! - Hat collection workflows
//! - Memory system functionality
//!
//! ## Usage
//!
//! ```bash
//! # Run all tests for all available backends
//! ralph-e2e all
//!
//! # Run tests for a specific backend
//! ralph-e2e claude
//!
//! # List available scenarios
//! ralph-e2e --list
//! ```

use clap::{Parser, ValueEnum};
use colored::Colorize;
use ralph_e2e::{
    AuthChecker,
    // Tier 7: Error Handling
    AuthFailureScenario,
    Backend as LibBackend,
    BackendUnavailableScenario,
    // Tier 3: Events
    BackpressureScenario,
    // Tier 2: Orchestration Loop
    CompletionScenario,
    // Tier 1: Connectivity
    ConnectivityScenario,
    EventsScenario,
    // Tier 5: Hat Collections
    HatBackendOverrideScenario,
    HatEventRoutingScenario,
    HatInstructionsScenario,
    HatMultiWorkflowScenario,
    HatSingleScenario,
    MaxIterationsScenario,
    // Tier 6: Memory System
    MemoryAddScenario,
    MemoryCorruptedFileScenario,
    MemoryInjectionScenario,
    MemoryLargeContentScenario,
    MemoryMissingFileScenario,
    MemoryPersistenceScenario,
    MemoryRapidWriteScenario,
    MemorySearchScenario,
    MultiIterScenario,
    ReportFormat as LibReportFormat,
    ReportWriter,
    RunConfig,
    SingleIterScenario,
    // Tier 4: Capabilities
    StreamingScenario,
    TerminalReporter,
    TestRunner,
    TestScenario,
    TimeoutScenario,
    ToolUseScenario,
    Verbosity,
    WorkspaceManager,
    create_incremental_progress_callback,
    resolve_ralph_binary,
};

/// Backend selection for E2E tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum Backend {
    /// Test all available backends
    #[default]
    All,
    /// Test Claude backend only
    Claude,
    /// Test Kiro backend only
    Kiro,
    /// Test OpenCode backend only
    Opencode,
}

impl std::fmt::Display for Backend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Backend::All => write!(f, "all"),
            Backend::Claude => write!(f, "claude"),
            Backend::Kiro => write!(f, "kiro"),
            Backend::Opencode => write!(f, "opencode"),
        }
    }
}

impl Backend {
    /// Converts CLI backend to library backend (if not All).
    fn to_lib_backend(self) -> Option<LibBackend> {
        match self {
            Backend::All => None,
            Backend::Claude => Some(LibBackend::Claude),
            Backend::Kiro => Some(LibBackend::Kiro),
            Backend::Opencode => Some(LibBackend::OpenCode),
        }
    }
}

/// E2E test harness for Ralph orchestrator.
///
/// Validates Ralph's behavior against real AI backends to ensure
/// the orchestration loop works correctly before releases.
#[derive(Parser, Debug)]
#[command(name = "ralph-e2e")]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Backend to test
    #[arg(value_enum, default_value_t = Backend::All)]
    pub backend: Backend,

    /// Show detailed output during tests
    #[arg(short, long)]
    pub verbose: bool,

    /// Only show pass/fail summary
    #[arg(short, long)]
    pub quiet: bool,

    /// List available test scenarios without running them
    #[arg(long)]
    pub list: bool,

    /// Run only tests matching this pattern
    #[arg(long)]
    pub filter: Option<String>,

    /// Generate report in specified format
    #[arg(long, value_enum, default_value_t = ReportFormat::Markdown)]
    pub report: ReportFormat,

    /// Keep test workspaces after tests complete (for debugging)
    #[arg(long)]
    pub keep_workspace: bool,

    /// Skip meta-Ralph analysis (faster, raw results only)
    #[arg(long)]
    pub skip_analysis: bool,
}

/// Report output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum ReportFormat {
    /// Markdown format (agent-readable)
    #[default]
    Markdown,
    /// JSON format (machine-readable)
    Json,
    /// Both markdown and JSON
    Both,
}

impl ReportFormat {
    /// Converts CLI report format to library report format.
    fn to_lib_format(self) -> LibReportFormat {
        match self {
            ReportFormat::Markdown => LibReportFormat::Markdown,
            ReportFormat::Json => LibReportFormat::Json,
            ReportFormat::Both => LibReportFormat::Both,
        }
    }
}

/// Returns all registered test scenarios.
fn get_all_scenarios() -> Vec<Box<dyn TestScenario>> {
    vec![
        // Tier 1: Connectivity (backend-agnostic)
        Box::new(ConnectivityScenario::new()),
        // Tier 2: Orchestration Loop (backend-agnostic)
        Box::new(SingleIterScenario::new()),
        Box::new(MultiIterScenario::new()),
        Box::new(CompletionScenario::new()),
        // Tier 3: Events (backend-agnostic)
        Box::new(EventsScenario::new()),
        Box::new(BackpressureScenario::new()),
        // Tier 4: Capabilities (backend-agnostic)
        Box::new(ToolUseScenario::new()),
        Box::new(StreamingScenario::new()),
        // Tier 5: Hat Collections (backend-agnostic)
        Box::new(HatSingleScenario::new()),
        Box::new(HatMultiWorkflowScenario::new()),
        Box::new(HatInstructionsScenario::new()),
        Box::new(HatEventRoutingScenario::new()),
        Box::new(HatBackendOverrideScenario::new()),
        // Tier 6: Memory System (backend-agnostic)
        Box::new(MemoryAddScenario::new()),
        Box::new(MemorySearchScenario::new()),
        Box::new(MemoryInjectionScenario::new()),
        Box::new(MemoryPersistenceScenario::new()),
        // Tier 6: Memory System (Chaos Tests)
        Box::new(MemoryCorruptedFileScenario::new()),
        Box::new(MemoryMissingFileScenario::new()),
        Box::new(MemoryRapidWriteScenario::new()),
        Box::new(MemoryLargeContentScenario::new()),
        // Tier 7: Error Handling (backend-agnostic)
        Box::new(TimeoutScenario::new()),
        Box::new(MaxIterationsScenario::new()),
        Box::new(AuthFailureScenario::new()),
        Box::new(BackendUnavailableScenario::new()),
    ]
}

fn main() {
    let cli = Cli::parse();

    // Print header
    println!(
        "\n{} {}",
        "🧪 E2E Test Harness".bold(),
        format!("v{}", env!("CARGO_PKG_VERSION")).dimmed()
    );
    println!("{}", "━".repeat(40).dimmed());

    // Determine verbosity
    let verbosity = if cli.quiet {
        Verbosity::Quiet
    } else if cli.verbose {
        Verbosity::Verbose
    } else {
        Verbosity::Normal
    };

    // Run the tests
    let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");

    if cli.list {
        rt.block_on(list_scenarios(&cli, verbosity));
        return;
    }

    rt.block_on(run_tests(&cli, verbosity));
}

async fn list_scenarios(cli: &Cli, verbosity: Verbosity) {
    // Check backend availability
    if verbosity != Verbosity::Quiet {
        println!("\n{}", "Checking backends...".dimmed());
        let checker = AuthChecker::new();
        let backends = checker.check_all().await;

        for info in backends {
            let status = match info.status_string().as_str() {
                s if s.contains("Authenticated") => format!("✅ {} - {}", info.backend, s).green(),
                s if s.contains("Not authenticated") => {
                    format!("⚠️  {} - {}", info.backend, s).yellow()
                }
                s => format!("❌ {} - {}", info.backend, s).red(),
            };
            println!("  {}", status);
        }
        println!();
    }

    // List scenarios
    let scenarios = get_all_scenarios();
    println!("{}\n", "Available scenarios:".bold());

    // Group by tier
    let mut current_tier = String::new();
    for scenario in &scenarios {
        // Filter by backend if specified
        if let Some(backend) = cli.backend.to_lib_backend()
            && !scenario.supported_backends().contains(&backend)
        {
            continue;
        }

        // Print tier header if changed
        if scenario.tier() != current_tier {
            current_tier = scenario.tier().to_string();
            println!("  {}", current_tier.bold().underline());
        }

        println!(
            "    {}  {}",
            scenario.id().cyan(),
            scenario.description().dimmed()
        );
    }

    if scenarios.is_empty() {
        println!("  {}", "No scenarios implemented yet".yellow());
    }

    println!(
        "\n  {}",
        format!(
            "Total: {} scenario{}",
            scenarios.len(),
            if scenarios.len() == 1 { "" } else { "s" }
        )
        .dimmed()
    );
}

async fn run_tests(cli: &Cli, verbosity: Verbosity) {
    // Check backend availability first
    if verbosity != Verbosity::Quiet {
        println!();
        let checker = AuthChecker::new();

        if let Some(backend) = cli.backend.to_lib_backend() {
            let info = checker.check(backend).await;
            let status = info.status_string();
            let status_fmt = if status.contains("Authenticated") {
                format!("{}: {} ✅", info.backend, status).green()
            } else if status.contains("Not authenticated") {
                format!("{}: {} ⚠️", info.backend, status).yellow()
            } else {
                format!("{}: {} ❌", info.backend, status).red()
            };
            println!("{}", status_fmt);
        } else {
            println!("{}", "Checking all backends...".dimmed());
            for info in checker.check_all().await {
                let status = match info.status_string().as_str() {
                    s if s.contains("Authenticated") => {
                        format!("  ✅ {} - {}", info.backend, s).green()
                    }
                    s if s.contains("Not authenticated") => {
                        format!("  ⚠️  {} - {}", info.backend, s).yellow()
                    }
                    s => format!("  ❌ {} - {}", info.backend, s).red(),
                };
                println!("{}", status);
            }
        }
    }

    // Set up workspace manager with absolute path
    // The PTY executor calls std::env::current_dir() which requires the workspace to exist.
    // Using absolute paths ensures the workspace is resolvable regardless of working directory changes.
    let workspace_path = std::env::current_dir()
        .expect("Failed to get current directory")
        .join(".e2e-tests");
    let workspace_mgr = WorkspaceManager::new(workspace_path.clone());

    // Get scenarios
    let scenarios = get_all_scenarios();

    // Build run configuration
    let mut config = RunConfig::new().keep_workspaces(cli.keep_workspace);

    if let Some(filter) = &cli.filter {
        config = config.with_filter(filter);
    }

    if let Some(backend) = cli.backend.to_lib_backend() {
        config = config.with_backend(backend);
    }

    // Resolve the ralph binary to use (local build preferred over PATH)
    let ralph_binary = resolve_ralph_binary();
    if verbosity != Verbosity::Quiet {
        println!(
            "{}",
            format!("Using binary: {}", ralph_binary.display()).dimmed()
        );
    }

    // Create runner with incremental progress callback
    let runner = TestRunner::new(workspace_mgr, scenarios)
        .with_binary(ralph_binary)
        .on_progress(create_incremental_progress_callback(
            verbosity,
            workspace_path.clone(),
        ));

    // Notify about live report
    if verbosity != Verbosity::Quiet {
        println!(
            "{}",
            format!(
                "Live report: {}",
                workspace_path.join("report-live.md").display()
            )
            .dimmed()
        );
        println!();
    }

    // Run the tests
    let results = match runner.run(&config).await {
        Ok(results) => results,
        Err(e) => {
            eprintln!("\n{} {}", "Error:".red().bold(), e);
            std::process::exit(1);
        }
    };

    // Write reports to disk
    let report_writer = ReportWriter::new(workspace_path);
    match report_writer.write(&results, None, cli.report.to_lib_format()) {
        Ok(paths) => {
            if verbosity != Verbosity::Quiet {
                for path in &paths {
                    println!("{}", format!("Report written: {}", path.display()).dimmed());
                }
            }
        }
        Err(e) => {
            eprintln!("{} Failed to write report: {}", "Warning:".yellow(), e);
        }
    }

    // Print summary
    let reporter = TerminalReporter::with_verbosity(verbosity);

    if verbosity != Verbosity::Quiet {
        // Print failures in detail
        if !results.all_passed() {
            reporter.print_failures(&results);
        }
    }

    // Always print summary
    reporter.print_summary(&results);

    // Exit with appropriate code
    if !results.all_passed() {
        std::process::exit(1);
    }
}
