use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use markdown_tui::RenderOptions;

use cc_eval::criteria::{score_criteria, status_from_score, total_score};
use cc_eval::llm::LlmProvider;
use cc_eval::results::{self, Baseline, EvalRun, ScenarioResult};
use cc_eval::scenario::{load_scenarios, ScenarioConfig};

/// Isolation mode for running evals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IsolationMode {
    /// No isolation (direct execution)
    None,
    /// Linux container via Apple's container CLI (default on macOS)
    Container,
}

#[derive(Parser)]
#[command(name = "cc-eval", about = "Claude Code eval runner")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run all eval scenarios in parallel (default)
    Run {
        /// Skip LLM judge (cheaper, faster)
        #[arg(long)]
        no_judge: bool,

        /// Filter to specific scenario(s), comma-separated
        #[arg(long, short)]
        filter: Option<String>,

        /// Disable container isolation (enabled by default on macOS)
        #[arg(long)]
        no_container: bool,

        /// Run the entire cc-eval process inside a Linux container.
        /// This avoids SSH key prompts by using container credentials.
        /// Builds Linux binaries automatically if needed.
        #[arg(long)]
        in_container: bool,

        /// LLM provider for answerer/judge (claude or gemini)
        #[arg(long, value_enum, default_value = "claude")]
        llm: LlmProvider,
    },
    /// Compare last two eval runs
    Compare,
    /// Show eval history
    History {
        /// Number of recent runs to show
        #[arg(short, default_value = "10")]
        n: usize,
    },
    /// List available scenarios
    List,
    /// Validate scenario markdown files
    Validate,
    /// Build dg and cc-eval binaries for Linux (required for --in-container)
    BuildLinux,
    /// Show insights from recent eval runs (errors, missing docs, judge feedback)
    Insights {
        /// Specific run ID to inspect (default: latest)
        #[arg(long, short)]
        run: Option<String>,

        /// Number of recent runs to summarize (ignored if --run specified)
        #[arg(short, default_value = "1")]
        n: usize,
    },
}

fn evals_dir() -> PathBuf {
    // Detect if we're in the cc-eval directory (has Cargo.toml with cc-eval)
    // or in the project root (has cc-eval/ subdirectory with Cargo.toml)
    if PathBuf::from("Cargo.toml").exists() {
        // We're in some cargo project - check if it's cc-eval
        if let Ok(content) = std::fs::read_to_string("Cargo.toml") {
            if content.contains("name = \"cc-eval\"") {
                // We're in cc-eval directory, use local evals/
                return PathBuf::from("evals");
            }
        }
    }
    // Assume we're at project root
    PathBuf::from("cc-eval/evals")
}

/// Find the pre-built Linux dg binary for container mode.
fn find_linux_dg_binary() -> Option<PathBuf> {
    // Walk up from current dir to find project root (has workspace Cargo.toml)
    let mut dir = std::env::current_dir().ok()?;
    loop {
        let cargo_toml = dir.join("Cargo.toml");
        if cargo_toml.exists() {
            if let Ok(content) = std::fs::read_to_string(&cargo_toml) {
                if content.contains("[workspace]") {
                    let binary = dir.join("target/aarch64-unknown-linux-musl/release/dg");
                    if binary.exists() {
                        return Some(binary);
                    }
                    return None;
                }
            }
        }
        if !dir.pop() {
            return None;
        }
    }
}

fn term_width() -> usize {
    crossterm::terminal::size()
        .map(|(cols, _)| cols as usize)
        .unwrap_or(120)
}

fn render_opts() -> RenderOptions {
    RenderOptions {
        width: term_width(),
        ..Default::default()
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let command = cli.command.unwrap_or(Commands::Run {
        no_judge: false, filter: None, no_container: false,
        in_container: false, llm: LlmProvider::Claude
    });

    match command {
        Commands::Run { no_judge, filter, no_container, in_container, llm } => {
            // --in-container: run entire cc-eval inside a Linux container
            if in_container {
                let args: Vec<String> = std::env::args().collect();
                let exit_code = cc_eval::container::run_eval_in_container(&args[1..])?;
                std::process::exit(exit_code);
            }

            // Container is default on macOS, --no-container to disable
            let isolation = if no_container {
                IsolationMode::None
            } else if cfg!(target_os = "macos") {
                // Check if container CLI is available
                if !cc_eval::container::is_available() {
                    anyhow::bail!(
                        "Container CLI not found.\n\
                         Please install container with: brew install container\n\
                         Then start it with: container system start\n\n\
                         Or use --no-container to run without isolation."
                    );
                }
                IsolationMode::Container
            } else {
                // Non-macOS: no isolation by default
                IsolationMode::None
            };
            run_eval(no_judge, filter, isolation, llm).await
        }
        Commands::Compare => {
            results::compare_last_two(&evals_dir(), &render_opts())?;
            Ok(())
        }
        Commands::History { n } => {
            results::print_history(&evals_dir(), n, &render_opts())?;
            Ok(())
        }
        Commands::List => {
            list_scenarios()?;
            Ok(())
        }
        Commands::Validate => {
            validate_scenarios()?;
            Ok(())
        }
        Commands::BuildLinux => {
            build_linux_binaries()?;
            Ok(())
        }
        Commands::Insights { run, n } => {
            results::print_insights(&evals_dir(), run.as_deref(), n)?;
            Ok(())
        }
    }
}

/// Build dg and cc-eval binaries for Linux (required for --in-container).
fn build_linux_binaries() -> Result<()> {
    // Check if container CLI is available
    if !cc_eval::container::is_available() {
        anyhow::bail!(
            "Container CLI not available. Install and start:\n\
             1. Install: https://github.com/apple/container\n\
             2. Start: container system start"
        );
    }

    let (dg_path, eval_path) = cc_eval::container::build_linux_binaries()?;
    eprintln!("Linux binaries built:");
    eprintln!("  dg: {}", dg_path.display());
    eprintln!("  cc-eval: {}", eval_path.display());
    eprintln!();
    eprintln!("Now you can run evals inside a container:");
    eprintln!("  cc-eval run --in-container");
    Ok(())
}

/// List all available scenarios.
fn list_scenarios() -> Result<()> {
    let scenarios = load_scenarios()?;
    eprintln!("Available scenarios ({}):", scenarios.len());
    for s in &scenarios {
        eprintln!("  {} ({} turns)", s.name, s.max_turns);
    }
    Ok(())
}

/// Validate all scenario markdown files.
fn validate_scenarios() -> Result<()> {
    let scenarios = load_scenarios()?;
    eprintln!("Validated {} scenario(s):", scenarios.len());
    for s in &scenarios {
        let has_answerer = if s.answerer_context.is_some() { "+" } else { "-" };
        let has_q_judge = if s.judge_question_prompt.is_some() { "+" } else { "-" };
        let has_d_judge = if s.judge_doc_prompt.is_some() { "+" } else { "-" };
        eprintln!(
            "  {} [answerer:{} q-judge:{} d-judge:{}]",
            s.name, has_answerer, has_q_judge, has_d_judge
        );
    }
    Ok(())
}

/// Wrapper to hold container config (keeps it alive during eval).
enum IsolationConfig {
    None,
    Container(std::sync::Arc<cc_eval::container::ContainerConfig>),
}

impl IsolationConfig {
    fn cli_path(&self) -> Option<PathBuf> {
        match self {
            IsolationConfig::None => None,
            IsolationConfig::Container(c) => Some(c.cli_path().to_path_buf()),
        }
    }
}

async fn run_eval(no_judge: bool, filter: Option<String>, isolation: IsolationMode, llm_provider: LlmProvider) -> Result<()> {
    // Load scenarios from markdown files
    let all_scenarios = load_scenarios()?;

    // Parse filter into set of scenario names
    let filter_set: Option<std::collections::HashSet<&str>> = filter.as_ref().map(|f| {
        f.split(',').map(|s| s.trim()).collect()
    });

    let scenarios_to_run: Vec<&ScenarioConfig> = all_scenarios
        .iter()
        .filter(|s| filter_set.as_ref().map_or(true, |f| f.contains(s.name.as_str())))
        .collect();

    if scenarios_to_run.is_empty() {
        eprintln!("cc-eval: no scenarios match filter");
        return Ok(());
    }

    match isolation {
        IsolationMode::None => eprintln!("cc-eval: no isolation (use container for better isolation)"),
        IsolationMode::Container => {
            eprintln!("cc-eval: container isolation enabled (use --no-container to disable)");
            // Do container setup once before running scenarios
            cc_eval::container::ensure_container_ready()?;
        }
    }

    eprintln!("cc-eval: LLM provider for answerer/judge: {llm_provider}");
    eprintln!("cc-eval: running {} scenario(s) in parallel...", scenarios_to_run.len());

    // Create workspaces + spawn all scenarios in parallel
    let mut handles = Vec::new();
    for config in scenarios_to_run {
        let workspace = cc_eval::setup::create_workspace()?;
        let name = config.name.clone();
        let prompt = config.prompt.clone();
        eprintln!("  [{}] workspace: {}", name, workspace.path().display());

        // Copy fixtures if specified
        if let Some(ref fixtures_name) = config.fixtures {
            cc_eval::setup::copy_fixtures(workspace.path(), fixtures_name)?;
            eprintln!("  [{}] fixtures: {}", name, fixtures_name);
        }

        let workspace_path = workspace.path().to_path_buf();
        let config = config.clone();

        // Create isolation config based on mode
        let isolation_config = match isolation {
            IsolationMode::None => IsolationConfig::None,
            IsolationMode::Container => {
                // Find Linux dg binary for mounting into container
                let linux_dg_path = find_linux_dg_binary();
                if linux_dg_path.is_none() {
                    eprintln!("  [{}] warning: Linux dg binary not found", name);
                    eprintln!("         Run: cc-eval build-linux");
                }
                match cc_eval::container::ContainerConfig::new(&workspace_path, linux_dg_path.as_deref()) {
                    Ok(container) => IsolationConfig::Container(std::sync::Arc::new(container)),
                    Err(e) => {
                        eprintln!("  [{}] warning: container setup failed: {}", name, e);
                        IsolationConfig::None
                    }
                }
            }
        };

        let cli_path_opt = isolation_config.cli_path();
        if let Some(ref p) = cli_path_opt {
            eprintln!("  [{}] using CLI path: {}", name, p.display());
            if let Ok(content) = std::fs::read_to_string(p) {
                eprintln!("  [{}] wrapper content:\n{}", name, content);
            }
        }

        handles.push(tokio::spawn(async move {
            let stats = cc_eval::eval::run_scenario_with_cli(&config, &workspace_path, cli_path_opt.as_deref(), llm_provider).await;
            // Keep workspace and isolation config alive until scenario finishes
            let _ws = workspace;
            let _isolation = isolation_config;
            (config, name, prompt, stats)
        }));
    }

    // Collect results
    let mut scenario_results: Vec<(ScenarioConfig, String, String, cc_eval::eval::EvalStats)> = Vec::new();
    for handle in handles {
        let (config, name, prompt, stats_result) = handle.await?;
        match stats_result {
            Ok(stats) => {
                if stats.total_input_tokens == 0 && stats.total_output_tokens == 0 {
                    eprintln!("  [{}] WARNING: 0 tokens — Claude produced no output", name);
                }
                scenario_results.push((config, name, prompt, stats));
            }
            Err(e) => {
                eprintln!("  [{}] ERROR: {e:?}", name);
            }
        }
    }

    // Run judges in parallel if enabled
    let mut scored_scenarios: Vec<ScenarioResult> = Vec::new();
    if no_judge {
        eprintln!("cc-eval: judge skipped (--no-judge)");
        for (config, name, _prompt, stats) in &scenario_results {
            let criteria = score_criteria(&name, stats, None);
            let (score, max_score) = total_score(&criteria);
            let baseline = Baseline::default();
            let status = status_from_score(score, baseline.min_score, baseline.target_score);
            scored_scenarios.push(ScenarioResult {
                name: name.clone(),
                score,
                max_score,
                status: status.into(),
                criteria,
                stats: stats.clone(),
                judge: None,
            });
            // Keep config alive for potential future use
            let _ = config;
        }
    } else {
        eprintln!("cc-eval: running LLM judges ({llm_provider}) in parallel...");
        let mut judge_handles = Vec::new();
        for (config, name, _prompt, stats) in &scenario_results {
            let config = config.clone();
            let questions = stats.questions.clone();
            let doc_contents = stats.doc_contents.clone();
            let name = name.clone();
            judge_handles.push(tokio::spawn(async move {
                let judge =
                    cc_eval::judge::run_judge(&config, &questions, doc_contents.as_deref(), llm_provider)
                        .await
                        .ok();
                (name, judge)
            }));
        }

        // Collect judge results
        let mut judge_map: HashMap<String, cc_eval::judge::JudgeResults> = HashMap::new();
        for handle in judge_handles {
            let (name, judge) = handle.await?;
            if let Some(j) = judge {
                judge_map.insert(name, j);
            }
        }

        for (_config, name, _prompt, stats) in &scenario_results {
            let judge = judge_map.remove(name);
            let criteria = score_criteria(&name, stats, judge.as_ref());
            let (score, max_score) = total_score(&criteria);
            let baseline = Baseline::default();
            let status = status_from_score(score, baseline.min_score, baseline.target_score);
            scored_scenarios.push(ScenarioResult {
                name: name.clone(),
                score,
                max_score,
                status: status.into(),
                criteria,
                stats: stats.clone(),
                judge,
            });
        }
    }

    // Compute totals
    let total_score_val: u32 = scored_scenarios.iter().map(|s| s.score).sum();
    let total_max: u32 = scored_scenarios.iter().map(|s| s.max_score).sum();
    let baseline = Baseline::default();
    let avg_pct = if total_max > 0 {
        total_score_val * 100 / total_max
    } else {
        0
    };
    let overall_status = status_from_score(avg_pct, baseline.min_score, baseline.target_score);

    // Get git info
    let (git_commit, git_branch) = results::get_git_info(&std::env::current_dir()?);

    let run = EvalRun {
        run_id: results::generate_run_id(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        git_commit,
        git_branch,
        scenarios: scored_scenarios,
        total_score: total_score_val,
        total_max_score: total_max,
        status: overall_status.into(),
        baseline,
    };

    // Save results
    let evals_dir = evals_dir();
    if let Err(e) = results::save_result(&run, &evals_dir) {
        eprintln!("  warning: failed to save results: {e}");
    }

    // Print table
    print_results(&run);

    // Clean up orphaned scenario volumes from this and previous runs
    if cc_eval::container::is_available() {
        cc_eval::container::cleanup_orphaned_volumes();
    }

    if overall_status == "fail" {
        std::process::exit(1);
    }

    Ok(())
}

fn criterion_label(name: &str) -> &str {
    match name {
        "questions_before_action" => "QBA",
        "question_count" => "QCnt",
        "question_quality" => "QQual",
        "opp_created" => "OPP",
        "doc_quality" => "DQual",
        "docs_created" => "Docs",
        "cross_linking" => "XLink",
        other => other,
    }
}

fn print_results(run: &EvalRun) {
    eprintln!();
    eprintln!("=== cc-eval: {} ===", run.run_id);
    match (&run.git_branch, &run.git_commit) {
        (Some(b), Some(c)) => eprintln!("git: {b} @ {c}"),
        _ => {}
    }
    eprintln!();

    let opts = render_opts();

    // Collect all unique criterion names across all scenarios (preserving order)
    let mut criterion_names: Vec<String> = Vec::new();
    for s in &run.scenarios {
        for c in &s.criteria {
            if !criterion_names.contains(&c.name) {
                criterion_names.push(c.name.clone());
            }
        }
    }

    // Build headers
    let mut headers: Vec<&str> = vec!["Scenario", "Score"];
    let labels: Vec<&str> = criterion_names.iter().map(|n| criterion_label(n)).collect();
    headers.extend(&labels);
    headers.extend(&["TokIn", "TokOut", "Tools", "Errs", "Cost", "Dur", "API", "Anly", "Status"]);

    // Build rows
    let mut rows: Vec<Vec<String>> = Vec::new();
    for s in &run.scenarios {
        let mut row = vec![
            s.name.clone(),
            format!("{}/{}", s.score, s.max_score),
        ];

        for cname in &criterion_names {
            let cell = s
                .criteria
                .iter()
                .find(|c| &c.name == cname)
                .map(|c| format!("{}/{}", c.score, c.max_score))
                .unwrap_or("-".into());
            row.push(cell);
        }

        row.push(s.stats.total_input_tokens.to_string());
        row.push(s.stats.total_output_tokens.to_string());
        row.push(s.stats.tool_call_count.to_string());
        row.push(s.stats.tool_error_count.to_string());
        row.push(
            s.stats
                .total_cost_usd
                .map(|c| format!("${c:.4}"))
                .unwrap_or("-".into()),
        );
        row.push(
            s.stats
                .duration_ms
                .map(|d| format!("{:.1}s", d / 1000.0))
                .unwrap_or("-".into()),
        );
        // Timing breakdown
        row.push(format!("{:.1}s", s.stats.timing.api_time_ms / 1000.0));
        row.push(format!("{:.0}ms", s.stats.timing.analysis_time_ms));
        row.push(
            match s.status.as_str() {
                "pass" => "PASS",
                "below_target" => "BELOW",
                _ => "FAIL",
            }
            .into(),
        );
        rows.push(row);
    }

    // Totals row if multiple scenarios
    if run.scenarios.len() > 1 {
        let mut total_row = vec![
            "TOTAL".into(),
            format!("{}/{}", run.total_score, run.total_max_score),
        ];
        for _ in &criterion_names {
            total_row.push(String::new());
        }
        let total_in: u64 = run.scenarios.iter().map(|s| s.stats.total_input_tokens).sum();
        let total_out: u64 = run.scenarios.iter().map(|s| s.stats.total_output_tokens).sum();
        let total_tools: usize = run.scenarios.iter().map(|s| s.stats.tool_call_count).sum();
        let total_errs: usize = run.scenarios.iter().map(|s| s.stats.tool_error_count).sum();
        let total_cost: f64 = run
            .scenarios
            .iter()
            .filter_map(|s| s.stats.total_cost_usd)
            .sum();
        let total_dur: f64 = run
            .scenarios
            .iter()
            .filter_map(|s| s.stats.duration_ms)
            .sum();

        total_row.push(total_in.to_string());
        total_row.push(total_out.to_string());
        total_row.push(total_tools.to_string());
        total_row.push(total_errs.to_string());
        total_row.push(if total_cost > 0.0 {
            format!("${total_cost:.4}")
        } else {
            "-".into()
        });
        total_row.push(if total_dur > 0.0 {
            format!("{:.1}s", total_dur / 1000.0)
        } else {
            "-".into()
        });
        // Timing totals
        let total_api: f64 = run.scenarios.iter().map(|s| s.stats.timing.api_time_ms).sum();
        let total_analysis: f64 = run.scenarios.iter().map(|s| s.stats.timing.analysis_time_ms).sum();
        total_row.push(format!("{:.1}s", total_api / 1000.0));
        total_row.push(format!("{:.0}ms", total_analysis));
        total_row.push(
            match run.status.as_str() {
                "pass" => "PASS",
                "below_target" => "BELOW",
                _ => "FAIL",
            }
            .into(),
        );
        rows.push(total_row);
    }

    let table = markdown_tui::render_table(&headers, &rows, &opts);
    eprint!("{table}");

    eprintln!(
        "Overall: {}/{} (target: {})",
        run.total_score, run.total_max_score, run.baseline.target_score
    );
}
