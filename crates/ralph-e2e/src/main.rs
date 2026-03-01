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

use clap::{Parser, Subcommand, ValueEnum};
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
    HooksBddConfig,
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
    MockConfig,
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
    discover_hooks_bdd_scenarios,
    resolve_ralph_binary,
    run_hooks_bdd_suite,
    run_mock_cli,
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
    #[command(subcommand)]
    pub command: Option<Command>,

    #[command(flatten)]
    pub test_opts: TestOpts,
}

/// Subcommands for ralph-e2e.
#[derive(Subcommand, Debug)]
pub enum Command {
    /// Mock CLI adapter for replaying cassettes (used as custom backend).
    MockCli {
        /// Path to the cassette file to replay
        #[arg(long)]
        cassette: std::path::PathBuf,

        /// Replay speed multiplier (0.0 = instant, 1.0 = real-time, 10.0 = 10x faster)
        #[arg(long, default_value = "0.0")]
        speed: f32,

        /// Comma-separated list of allowed command prefixes
        #[arg(long)]
        allow: Option<String>,
    },
}

/// Options for running E2E tests.
#[derive(Parser, Debug)]
pub struct TestOpts {
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

    /// Run the hooks BDD acceptance suite from `features/hooks/*.feature`
    #[arg(long)]
    pub hooks_bdd: bool,

    /// Generate report in specified format
    #[arg(long, value_enum, default_value_t = ReportFormat::Markdown)]
    pub report: ReportFormat,

    /// Keep test workspaces after tests complete (for debugging)
    #[arg(long)]
    pub keep_workspace: bool,

    /// Skip meta-Ralph analysis (faster, raw results only)
    #[arg(long)]
    pub skip_analysis: bool,

    /// Use mock mode (replay cassettes instead of real backends)
    #[arg(long)]
    pub mock: bool,

    /// Replay speed for mock mode (0.0 = instant, 10.0 = 10x faster)
    #[arg(long, default_value = "0.0")]
    pub mock_speed: f32,
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

    // Handle subcommands
    if let Some(command) = cli.command {
        match command {
            Command::MockCli {
                cassette,
                speed,
                allow,
            } => {
                // Run the mock CLI
                if let Err(e) = run_mock_cli(&cassette, speed, allow.as_deref()) {
                    eprintln!("{} {}", "Error:".red().bold(), e);
                    std::process::exit(1);
                }
                return;
            }
        }
    }

    // Print header for test runs
    println!(
        "\n{} {}",
        "🧪 E2E Test Harness".bold(),
        format!("v{}", env!("CARGO_PKG_VERSION")).dimmed()
    );
    println!("{}", "━".repeat(40).dimmed());

    if cli.test_opts.mock {
        println!("{}", "Mode: Mock (cassette replay)".dimmed());
    }

    // Determine verbosity
    let verbosity = if cli.test_opts.quiet {
        Verbosity::Quiet
    } else if cli.test_opts.verbose {
        Verbosity::Verbose
    } else {
        Verbosity::Normal
    };

    if cli.test_opts.hooks_bdd {
        if cli.test_opts.list {
            list_hooks_bdd_scenarios(&cli.test_opts, verbosity);
        } else {
            run_hooks_bdd_placeholder_suite(&cli.test_opts, verbosity);
        }
        return;
    }

    // Run the tests
    let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");

    if cli.test_opts.list {
        rt.block_on(list_scenarios(&cli.test_opts, verbosity));
        return;
    }

    rt.block_on(run_tests(&cli.test_opts, verbosity));
}

fn list_hooks_bdd_scenarios(opts: &TestOpts, verbosity: Verbosity) {
    let scenarios = match discover_hooks_bdd_scenarios(opts.filter.as_deref()) {
        Ok(scenarios) => scenarios,
        Err(error) => {
            eprintln!("\n{} {}", "Error:".red().bold(), error);
            std::process::exit(1);
        }
    };

    println!("\n{}\n", "Available hooks BDD scenarios:".bold());

    let mut current_feature = String::new();
    for scenario in &scenarios {
        if scenario.feature_file != current_feature {
            current_feature = scenario.feature_file.clone();
            println!("  {}", current_feature.bold().underline());
        }

        println!(
            "    {}  {}",
            scenario.scenario_id.cyan(),
            scenario.scenario_name.dimmed()
        );
    }

    if scenarios.is_empty() {
        println!("  {}", "No hooks BDD scenarios discovered".yellow());
    }

    if !opts.mock && verbosity != Verbosity::Quiet {
        println!(
            "\n  {}",
            "Tip: run with --mock to satisfy CI-safe step definitions".yellow()
        );
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

fn run_hooks_bdd_placeholder_suite(opts: &TestOpts, verbosity: Verbosity) {
    let config = HooksBddConfig::new(opts.filter.clone(), opts.mock);
    let results = match run_hooks_bdd_suite(&config) {
        Ok(results) => results,
        Err(error) => {
            eprintln!("\n{} {}", "Error:".red().bold(), error);
            std::process::exit(1);
        }
    };

    if verbosity != Verbosity::Quiet {
        println!("\n{}", "Running hooks BDD acceptance suite".bold());
        if opts.mock {
            println!("{}", "Mode: CI-safe (--mock enabled)".dimmed());
        } else {
            println!(
                "{}",
                "Mode: non-CI-safe (--mock disabled)".yellow().dimmed()
            );
        }
        println!();
    }

    for result in &results.results {
        let status = if result.passed {
            "✅ PASS".green()
        } else {
            "❌ FAIL".red()
        };

        println!(
            "{} {} {} ({})",
            status,
            result.scenario_id.cyan(),
            result.scenario_name,
            result.feature_file.dimmed()
        );

        if verbosity != Verbosity::Quiet {
            println!("    {}", result.message.dimmed());
        }
    }

    println!(
        "\n{}",
        format!(
            "Summary: {} passed, {} failed, {} total",
            results.passed_count(),
            results.failed_count(),
            results.total_count()
        )
        .bold()
    );

    if !results.all_passed() {
        std::process::exit(1);
    }
}

async fn list_scenarios(opts: &TestOpts, verbosity: Verbosity) {
    // Check backend availability (skip in mock mode)
    if !opts.mock && verbosity != Verbosity::Quiet {
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
        if let Some(backend) = opts.backend.to_lib_backend()
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

async fn run_tests(opts: &TestOpts, verbosity: Verbosity) {
    // Check backend availability first (skip in mock mode)
    if !opts.mock && verbosity != Verbosity::Quiet {
        println!();
        let checker = AuthChecker::new();

        if let Some(backend) = opts.backend.to_lib_backend() {
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
    let mut config = RunConfig::new().keep_workspaces(opts.keep_workspace);

    if let Some(filter) = &opts.filter {
        config = config.with_filter(filter);
    }

    if let Some(backend) = opts.backend.to_lib_backend() {
        config = config.with_backend(backend);
    }

    // Configure mock mode if enabled
    if opts.mock {
        let mock_config = MockConfig::default().with_speed(opts.mock_speed);
        config = config.with_mock(mock_config);
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
    match report_writer.write(&results, None, opts.report.to_lib_format()) {
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
