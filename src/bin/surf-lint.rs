//! `surf-lint` — CLI for the SurfDoc lint/check/fix pipeline.
//!
//! Built only with the non-default `cli` feature:
//!
//! ```text
//! cargo run --features cli -- check path/to/doc.surf
//! cargo test --features cli            # includes tests/cli.rs
//! ```
//!
//! # Exit codes (stable contract for CI)
//!
//! | Code | Meaning |
//! |------|---------|
//! | 0    | clean — no errors (and no warnings, or warnings not denied) |
//! | 1    | errors found; for `fix`: errors remain after fixing, or the fix loop did not converge |
//! | 2    | warnings present AND `--deny-warnings` |
//! | 3    | internal/IO error (unreadable path, bad usage, write failure) |
//!
//! Multi-file runs aggregate to the MAX code across files. `fix --dry-run`
//! does NOT exit non-zero merely because changes would be applied — it exits
//! per check semantics on the issues that would REMAIN after the simulated
//! fix (documented contract).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use serde::Deserialize;
use surf_parse::lint::{
    JSON_SCHEMA_VERSION, MAX_FIX_ITERATIONS, apply_fixes_with, reports_to_json, rule_registry,
};
use surf_parse::{CheckReport, Diagnostic, FixSafety, LintConfig, Severity, check_with};

const EXIT_CLEAN: u8 = 0;
const EXIT_ERRORS: u8 = 1;
const EXIT_DENIED_WARNINGS: u8 = 2;
const EXIT_INTERNAL: u8 = 3;

/// Lines of context around a change in `--diff` output.
const DIFF_CONTEXT: usize = 3;

#[derive(Parser)]
#[command(
    name = "surf-lint",
    version,
    about = "Deterministic check/diagnose/fix for SurfDoc (.surf) files",
    long_about = "Deterministic check/diagnose/fix for SurfDoc (.surf) files.\n\n\
        Exit codes: 0 clean; 1 errors found (fix: errors remain or no convergence);\n\
        2 warnings present with --deny-warnings; 3 internal/IO error.\n\
        Multi-file runs exit with the MAX code across files.\n\n\
        Configuration (.surflint.toml, schema v1): check/fix look for a\n\
        .surflint.toml next to each target file, walking parent directories up\n\
        to the filesystem root (first found wins; stdin searches from the\n\
        current directory). --config <path> overrides discovery; --no-config\n\
        disables it. A malformed config is an internal error (exit 3).\n\n\
        \x20   [severity]                # per-rule severity override / disable\n\
        \x20   L010 = \"off\"              # \"error\" | \"warning\" | \"info\" | \"off\"\n\
        \x20   P005 = \"info\"\n\n\
        \x20   [frontmatter]             # extra allowed enum values (suppress P005)\n\
        \x20   extra_types = [\"checkpoint\", \"spec\"]\n\
        \x20   extra_statuses = [\"superseded\"]\n\
        \x20   extra_scopes = [\"org\"]"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Check files and report diagnostics (no modification).
    Check {
        /// Files, directories (recursed for *.surf), or '-' for stdin.
        #[arg(required = true)]
        paths: Vec<String>,
        /// Output format.
        #[arg(long, value_enum, default_value_t = Format::Human)]
        format: Format,
        /// Exit 2 when warnings are present (and no errors).
        #[arg(long)]
        deny_warnings: bool,
        /// Use this .surflint.toml instead of per-file discovery.
        #[arg(long, conflicts_with = "no_config")]
        config: Option<PathBuf>,
        /// Ignore .surflint.toml files entirely.
        #[arg(long)]
        no_config: bool,
    },
    /// Apply deterministic auto-fixes in place ('-' fixes stdin to stdout).
    Fix {
        /// Files, directories (recursed for *.surf), or '-' for stdin.
        #[arg(required = true)]
        paths: Vec<String>,
        /// Report would-be changes without writing any file.
        #[arg(long)]
        dry_run: bool,
        /// Also apply Suggested-tier fixes (default: Safe tier only).
        #[arg(long)]
        fix_suggested: bool,
        /// Show a unified-style diff of the changes.
        #[arg(long)]
        diff: bool,
        /// Use this .surflint.toml instead of per-file discovery.
        #[arg(long, conflicts_with = "no_config")]
        config: Option<PathBuf>,
        /// Ignore .surflint.toml files entirely.
        #[arg(long)]
        no_config: bool,
    },
    /// Print the lint rule registry (spec/rules.toml).
    Rules {
        /// Output format.
        #[arg(long, value_enum, default_value_t = RulesFormat::Human)]
        format: RulesFormat,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Format {
    /// Source excerpts with caret underlines.
    Human,
    /// Versioned JSON envelope (schema_version 1).
    Json,
    /// GitHub Actions workflow annotations.
    Github,
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum RulesFormat {
    /// Aligned table.
    Human,
    /// Versioned JSON envelope (schema_version 1).
    Json,
}

fn main() -> ExitCode {
    // Map clap usage errors onto the documented contract: help/version are
    // clean (0); malformed usage is an internal error (3), NOT clap's default
    // 2, which this CLI reserves for --deny-warnings.
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) if e.use_stderr() => {
            let _ = e.print();
            return ExitCode::from(EXIT_INTERNAL);
        }
        Err(e) => {
            let _ = e.print();
            return ExitCode::from(EXIT_CLEAN);
        }
    };
    match run(&cli.command) {
        Ok(code) => ExitCode::from(code),
        Err(msg) => {
            eprintln!("surf-lint: error: {msg}");
            ExitCode::from(EXIT_INTERNAL)
        }
    }
}

fn run(command: &Command) -> Result<u8, String> {
    match command {
        Command::Check {
            paths,
            format,
            deny_warnings,
            config,
            no_config,
        } => run_check(
            paths,
            *format,
            *deny_warnings,
            &ConfigSource::new(config.as_deref(), *no_config)?,
        ),
        Command::Fix {
            paths,
            dry_run,
            fix_suggested,
            diff,
            config,
            no_config,
        } => run_fix(
            paths,
            *dry_run,
            *fix_suggested,
            *diff,
            &ConfigSource::new(config.as_deref(), *no_config)?,
        ),
        Command::Rules { format } => run_rules(*format),
    }
}

// ------------------------------------------------------------------
// .surflint.toml configuration (CLI-side only — the lint core takes the
// resulting LintConfig struct and never touches the filesystem)
// ------------------------------------------------------------------

/// `.surflint.toml` schema v1.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct SurfLintToml {
    /// Rule id → "error" | "warning" | "info" | "off".
    #[serde(default)]
    severity: BTreeMap<String, String>,
    /// Extra allowed front matter enum values (suppress P005 for them).
    #[serde(default)]
    frontmatter: FrontMatterToml,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct FrontMatterToml {
    #[serde(default)]
    extra_types: Vec<String>,
    #[serde(default)]
    extra_statuses: Vec<String>,
    #[serde(default)]
    extra_scopes: Vec<String>,
}

impl SurfLintToml {
    fn into_lint_config(self, origin: &Path) -> Result<LintConfig, String> {
        let mut cfg = LintConfig::default();
        for (rule, value) in self.severity {
            match value.as_str() {
                "error" => cfg.severity_overrides.insert(rule, Severity::Error),
                "warning" => cfg.severity_overrides.insert(rule, Severity::Warning),
                "info" => cfg.severity_overrides.insert(rule, Severity::Info),
                "off" => {
                    cfg.disabled_rules.insert(rule);
                    None
                }
                other => {
                    return Err(format!(
                        "invalid config '{}': [severity] {rule} = {other:?} — expected \"error\", \"warning\", \"info\", or \"off\"",
                        origin.display()
                    ));
                }
            };
        }
        let mut extend = |field: &str, values: Vec<String>| {
            if !values.is_empty() {
                cfg.extra_frontmatter_values
                    .entry(field.to_string())
                    .or_insert_with(BTreeSet::new)
                    .extend(values);
            }
        };
        extend("type", self.frontmatter.extra_types);
        extend("status", self.frontmatter.extra_statuses);
        extend("scope", self.frontmatter.extra_scopes);
        Ok(cfg)
    }
}

/// Load and translate a `.surflint.toml`. Any read/parse/value error is an
/// internal error (exit 3) with the offending path in the message.
fn load_config_file(path: &Path) -> Result<LintConfig, String> {
    let raw = fs::read_to_string(path)
        .map_err(|e| format!("cannot read config '{}': {e}", path.display()))?;
    let parsed: SurfLintToml =
        toml::from_str(&raw).map_err(|e| format!("invalid config '{}': {e}", path.display()))?;
    parsed.into_lint_config(path)
}

/// First `.surflint.toml` found from `start` walking up to the root.
fn discover_config_path(start: &Path) -> Option<PathBuf> {
    let mut dir = Some(start);
    while let Some(d) = dir {
        let candidate = d.join(".surflint.toml");
        if candidate.is_file() {
            return Some(candidate);
        }
        dir = d.parent();
    }
    None
}

/// How check/fix resolve the [`LintConfig`] for each input.
enum ConfigSource {
    /// `--no-config`: defaults everywhere.
    Disabled,
    /// `--config <path>`: one explicit config for every input.
    Fixed(LintConfig),
    /// Default: per-file discovery, cached per starting directory.
    Discover(std::cell::RefCell<BTreeMap<PathBuf, LintConfig>>),
}

impl ConfigSource {
    fn new(explicit: Option<&Path>, no_config: bool) -> Result<Self, String> {
        if no_config {
            return Ok(ConfigSource::Disabled);
        }
        match explicit {
            Some(path) => Ok(ConfigSource::Fixed(load_config_file(path)?)),
            None => Ok(ConfigSource::Discover(std::cell::RefCell::new(
                BTreeMap::new(),
            ))),
        }
    }

    /// Config for one input. Files discover from their parent directory;
    /// stdin discovers from the current directory.
    fn for_input(&self, input: &Input) -> Result<LintConfig, String> {
        let cache = match self {
            ConfigSource::Disabled => return Ok(LintConfig::default()),
            ConfigSource::Fixed(cfg) => return Ok(cfg.clone()),
            ConfigSource::Discover(cache) => cache,
        };
        let start = match input {
            Input::Stdin => std::env::current_dir()
                .map_err(|e| format!("cannot determine current directory: {e}"))?,
            Input::File(p) => {
                let abs = if p.is_absolute() {
                    p.clone()
                } else {
                    std::env::current_dir()
                        .map_err(|e| format!("cannot determine current directory: {e}"))?
                        .join(p)
                };
                abs.parent().map(Path::to_path_buf).unwrap_or(abs)
            }
        };
        if let Some(cfg) = cache.borrow().get(&start) {
            return Ok(cfg.clone());
        }
        let cfg = match discover_config_path(&start) {
            Some(path) => load_config_file(&path)?,
            None => LintConfig::default(),
        };
        cache.borrow_mut().insert(start, cfg.clone());
        Ok(cfg)
    }
}

// ------------------------------------------------------------------
// Input collection
// ------------------------------------------------------------------

/// One unit of work: stdin or a file path.
enum Input {
    Stdin,
    File(PathBuf),
}

impl Input {
    fn label(&self) -> String {
        match self {
            Input::Stdin => "<stdin>".to_string(),
            Input::File(p) => p.display().to_string(),
        }
    }

    fn read(&self) -> Result<String, String> {
        match self {
            Input::Stdin => {
                let mut buf = String::new();
                std::io::stdin()
                    .read_to_string(&mut buf)
                    .map_err(|e| format!("cannot read stdin: {e}"))?;
                Ok(buf)
            }
            Input::File(p) => {
                fs::read_to_string(p).map_err(|e| format!("cannot read '{}': {e}", p.display()))
            }
        }
    }
}

/// Expand CLI path arguments: `-` is stdin, directories are recursed for
/// `*.surf` (sorted for deterministic output), files are taken as-is.
fn collect_inputs(paths: &[String]) -> Result<Vec<Input>, String> {
    let mut inputs = Vec::new();
    for raw in paths {
        if raw == "-" {
            inputs.push(Input::Stdin);
            continue;
        }
        let path = PathBuf::from(raw);
        let meta =
            fs::metadata(&path).map_err(|e| format!("cannot access '{}': {e}", path.display()))?;
        if meta.is_dir() {
            collect_surf_files(&path, &mut inputs)?;
        } else {
            inputs.push(Input::File(path));
        }
    }
    Ok(inputs)
}

/// Recursively collect `*.surf` files under `dir` in sorted order.
fn collect_surf_files(dir: &Path, inputs: &mut Vec<Input>) -> Result<(), String> {
    let entries =
        fs::read_dir(dir).map_err(|e| format!("cannot read directory '{}': {e}", dir.display()))?;
    let mut paths: Vec<PathBuf> = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("cannot read directory '{}': {e}", dir.display()))?;
        paths.push(entry.path());
    }
    paths.sort();
    for path in paths {
        if path.is_dir() {
            collect_surf_files(&path, inputs)?;
        } else if path.extension().is_some_and(|ext| ext == "surf") {
            inputs.push(Input::File(path));
        }
    }
    Ok(())
}

// ------------------------------------------------------------------
// check
// ------------------------------------------------------------------

fn run_check(
    paths: &[String],
    format: Format,
    deny_warnings: bool,
    config: &ConfigSource,
) -> Result<u8, String> {
    let inputs = collect_inputs(paths)?;
    let stdout = std::io::stdout();
    let color = format == Format::Human && stdout.is_terminal();
    let mut out = stdout.lock();

    let mut exit = EXIT_CLEAN;
    let mut files_checked = 0usize;
    let mut totals = CheckTotals::default();
    let mut json_reports: Vec<(String, CheckReport)> = Vec::new();

    for input in inputs {
        let label = input.label();
        let cfg = config.for_input(&input)?;
        let content = match input.read() {
            Ok(c) => c,
            Err(msg) => {
                eprintln!("surf-lint: error: {msg}");
                exit = exit.max(EXIT_INTERNAL);
                continue;
            }
        };
        // Diagnostics are anchored to the CRLF-normalised source `parse`
        // works on; render excerpts/offsets against the same text.
        let source = content.replace("\r\n", "\n");
        let report = check_with(&source, &cfg);
        files_checked += 1;
        totals.add(&report);
        exit = exit.max(check_exit_code(&report, deny_warnings));

        match format {
            Format::Human => {
                render_human_file(&mut out, &label, &source, &report, color).map_err(stdout_err)?
            }
            Format::Github => render_github_file(&mut out, &label, &report).map_err(stdout_err)?,
            Format::Json => json_reports.push((label, report)),
        }
    }

    match format {
        Format::Human => {
            writeln!(out, "{}", totals.summary_line(files_checked)).map_err(stdout_err)?;
        }
        Format::Json => {
            // Shared envelope construction (surf_parse::lint) — identical
            // structure to the wasm `check_json` export.
            let pairs: Vec<(Option<&str>, &CheckReport)> = json_reports
                .iter()
                .map(|(label, report)| (Some(label.as_str()), report))
                .collect();
            let envelope = reports_to_json(&pairs);
            let rendered = serde_json::to_string_pretty(&envelope)
                .map_err(|e| format!("cannot serialize JSON output: {e}"))?;
            writeln!(out, "{rendered}").map_err(stdout_err)?;
        }
        Format::Github => {}
    }
    Ok(exit)
}

/// Exit code contribution of one checked file.
fn check_exit_code(report: &CheckReport, deny_warnings: bool) -> u8 {
    if report.error_count > 0 {
        EXIT_ERRORS
    } else if deny_warnings && report.warning_count > 0 {
        EXIT_DENIED_WARNINGS
    } else {
        EXIT_CLEAN
    }
}

#[derive(Default)]
struct CheckTotals {
    errors: usize,
    warnings: usize,
    infos: usize,
    fixable: usize,
}

impl CheckTotals {
    fn add(&mut self, report: &CheckReport) {
        self.errors += report.error_count;
        self.warnings += report.warning_count;
        self.infos += report.info_count;
        self.fixable += report.fixable_count;
    }

    fn summary_line(&self, files: usize) -> String {
        let noun = if files == 1 { "file" } else { "files" };
        if self.errors == 0 && self.warnings == 0 && self.infos == 0 {
            return format!("checked {files} {noun}: clean");
        }
        let mut parts = Vec::new();
        if self.errors > 0 {
            parts.push(format!("{} error(s)", self.errors));
        }
        if self.warnings > 0 {
            parts.push(format!("{} warning(s)", self.warnings));
        }
        if self.infos > 0 {
            parts.push(format!("{} info(s)", self.infos));
        }
        format!(
            "checked {files} {noun}: {} ({} fixable)",
            parts.join(", "),
            self.fixable
        )
    }
}

fn stdout_err(e: std::io::Error) -> String {
    format!("cannot write output: {e}")
}

// ------------------------------------------------------------------
// Human renderer
// ------------------------------------------------------------------

/// ANSI escape palette; all-empty when color is off.
struct Palette {
    red: &'static str,
    yellow: &'static str,
    cyan: &'static str,
    bold: &'static str,
    dim: &'static str,
    reset: &'static str,
}

impl Palette {
    fn new(color: bool) -> Self {
        if color {
            Palette {
                red: "\x1b[31m",
                yellow: "\x1b[33m",
                cyan: "\x1b[36m",
                bold: "\x1b[1m",
                dim: "\x1b[2m",
                reset: "\x1b[0m",
            }
        } else {
            Palette {
                red: "",
                yellow: "",
                cyan: "",
                bold: "",
                dim: "",
                reset: "",
            }
        }
    }

    fn severity_color(&self, severity: Severity) -> &'static str {
        match severity {
            Severity::Error => self.red,
            Severity::Warning => self.yellow,
            Severity::Info => self.cyan,
        }
    }
}

fn severity_name(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "info",
    }
}

/// Byte offset of the start of each (1-based) line in `source`.
fn line_start_offsets(source: &str) -> Vec<usize> {
    let mut offsets = vec![0usize];
    for (i, b) in source.bytes().enumerate() {
        if b == b'\n' {
            offsets.push(i + 1);
        }
    }
    offsets
}

fn render_human_file(
    out: &mut impl Write,
    label: &str,
    source: &str,
    report: &CheckReport,
    color: bool,
) -> std::io::Result<()> {
    let p = Palette::new(color);
    let line_starts = line_start_offsets(source);
    let lines: Vec<&str> = source.split('\n').collect();

    for d in &report.diagnostics {
        render_human_diagnostic(out, label, d, &lines, &line_starts, &p)?;
    }
    Ok(())
}

fn render_human_diagnostic(
    out: &mut impl Write,
    label: &str,
    d: &Diagnostic,
    lines: &[&str],
    line_starts: &[usize],
    p: &Palette,
) -> std::io::Result<()> {
    let code = d.code.as_deref().unwrap_or("----");
    let sev = severity_name(d.severity);
    let sev_color = p.severity_color(d.severity);
    let line_no = d.span.map(|s| s.start_line).unwrap_or(0);

    if line_no > 0 {
        writeln!(
            out,
            "{bold}{label}:{line_no}:{reset} {sev_color}{sev}[{code}]{reset} {msg}",
            bold = p.bold,
            reset = p.reset,
            msg = d.message,
        )?;
    } else {
        writeln!(
            out,
            "{bold}{label}:{reset} {sev_color}{sev}[{code}]{reset} {msg}",
            bold = p.bold,
            reset = p.reset,
            msg = d.message,
        )?;
    }

    // Source excerpt with caret underline (first line of the span).
    if let Some(span) = d.span
        && span.start_line > 0
        && let Some(line) = lines.get(span.start_line - 1)
        && let Some(&line_start) = line_starts.get(span.start_line - 1)
    {
        let line_end = line_start + line.len();
        // Clamp the span to this line; degenerate spans underline 1 char.
        let start = span.start_offset.clamp(line_start, line_end);
        let end = span.end_offset.clamp(start, line_end);
        let col_chars = count_chars(line, 0, start - line_start);
        let width_chars = count_chars(line, start - line_start, end - line_start).max(1);

        let gutter = format!("{line_no:>5} | ");
        writeln!(
            out,
            "{dim}{gutter}{reset}{line}",
            dim = p.dim,
            reset = p.reset
        )?;
        writeln!(
            out,
            "{dim}{blank:>5} | {reset}{pad}{sev_color}{carets}{reset}",
            dim = p.dim,
            reset = p.reset,
            blank = "",
            pad = " ".repeat(col_chars),
            sev_color = sev_color,
            carets = "^".repeat(width_chars),
        )?;
    }

    if let Some(fix) = &d.fix {
        let tier = match fix.safety {
            FixSafety::Safe => "",
            FixSafety::Suggested => " (requires --fix-suggested)",
        };
        writeln!(
            out,
            "{dim}      = fix available{tier}: {desc}{reset}",
            dim = p.dim,
            reset = p.reset,
            desc = fix.description,
        )?;
    }
    Ok(())
}

/// Character count of the byte range `[from, to)` within `line` (relative
/// byte offsets, already clamped into the line). Falls back to byte distance
/// when the range is not on char boundaries (defensive; spans are
/// boundary-safe).
fn count_chars(line: &str, from: usize, to: usize) -> usize {
    line.get(from..to)
        .map_or(to.saturating_sub(from), |s| s.chars().count())
}

// ------------------------------------------------------------------
// GitHub Actions renderer
// ------------------------------------------------------------------

fn render_github_file(
    out: &mut impl Write,
    label: &str,
    report: &CheckReport,
) -> std::io::Result<()> {
    for d in &report.diagnostics {
        let kind = match d.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "notice",
        };
        let line = d.span.map(|s| s.start_line).unwrap_or(1).max(1);
        let code = d.code.as_deref().unwrap_or("----");
        // GitHub annotation values must keep the message on one line.
        let message = d.message.replace('\n', " ");
        writeln!(out, "::{kind} file={label},line={line}::{code}: {message}")?;
    }
    Ok(())
}

// ------------------------------------------------------------------
// fix
// ------------------------------------------------------------------

fn run_fix(
    paths: &[String],
    dry_run: bool,
    fix_suggested: bool,
    diff: bool,
    config: &ConfigSource,
) -> Result<u8, String> {
    let tier = if fix_suggested {
        FixSafety::Suggested
    } else {
        FixSafety::Safe
    };
    let inputs = collect_inputs(paths)?;
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let mut exit = EXIT_CLEAN;

    for input in inputs {
        let label = input.label();
        let cfg = config.for_input(&input)?;
        let content = match input.read() {
            Ok(c) => c,
            Err(msg) => {
                eprintln!("surf-lint: error: {msg}");
                exit = exit.max(EXIT_INTERNAL);
                continue;
            }
        };
        let outcome = apply_fixes_with(&content, tier, &cfg);
        let changed = outcome.source != content;
        let mut file_code = EXIT_CLEAN;

        if outcome.iterations == MAX_FIX_ITERATIONS {
            eprintln!(
                "surf-lint: warning: {label}: fixes did not converge after {MAX_FIX_ITERATIONS} passes; output may still contain fixable issues"
            );
            file_code = file_code.max(EXIT_ERRORS);
        }

        // Exit semantics: report issues REMAINING after the (real or
        // simulated) fix; --dry-run never fails just because edits exist.
        let remaining = check_with(&outcome.source, &cfg);
        if remaining.error_count > 0 {
            file_code = file_code.max(EXIT_ERRORS);
        }

        match &input {
            Input::Stdin => {
                // stdin mode: fixed source goes to stdout; reports to stderr.
                if dry_run {
                    report_fix(
                        &mut std::io::stderr().lock(),
                        &label,
                        &outcome,
                        dry_run,
                        &remaining,
                    )
                    .map_err(stderr_err)?;
                } else {
                    write!(out, "{}", outcome.source).map_err(stdout_err)?;
                }
                if diff && changed {
                    let rendered =
                        unified_diff(&content.replace("\r\n", "\n"), &outcome.source, &label);
                    write!(std::io::stderr().lock(), "{rendered}").map_err(stderr_err)?;
                }
            }
            Input::File(path) => {
                if changed
                    && !dry_run
                    && let Err(e) = fs::write(path, &outcome.source)
                {
                    eprintln!("surf-lint: error: cannot write '{label}': {e}");
                    exit = exit.max(EXIT_INTERNAL);
                    continue;
                }
                report_fix(&mut out, &label, &outcome, dry_run, &remaining).map_err(stdout_err)?;
                if diff && changed {
                    let rendered =
                        unified_diff(&content.replace("\r\n", "\n"), &outcome.source, &label);
                    write!(out, "{rendered}").map_err(stdout_err)?;
                }
            }
        }

        exit = exit.max(file_code);
    }
    Ok(exit)
}

fn stderr_err(e: std::io::Error) -> String {
    format!("cannot write to stderr: {e}")
}

fn report_fix(
    out: &mut impl Write,
    label: &str,
    outcome: &surf_parse::FixOutcome,
    dry_run: bool,
    remaining: &CheckReport,
) -> std::io::Result<()> {
    let verb = if dry_run { "would apply" } else { "applied" };
    if outcome.applied.is_empty() {
        writeln!(out, "{label}: no applicable fixes")?;
    } else {
        writeln!(
            out,
            "{label}: {verb} {n} fix(es) in {it} pass(es)",
            n = outcome.applied.len(),
            it = outcome.iterations,
        )?;
        for fix in &outcome.applied {
            writeln!(
                out,
                "  [{code}] {desc}",
                code = fix.code,
                desc = fix.description
            )?;
        }
    }
    for skip in &outcome.skipped {
        writeln!(
            out,
            "  skipped [{code}] {desc} — {reason}",
            code = skip.code,
            desc = skip.description,
            reason = skip.reason,
        )?;
    }
    if remaining.error_count > 0 || remaining.warning_count > 0 {
        writeln!(
            out,
            "  remaining: {e} error(s), {w} warning(s)",
            e = remaining.error_count,
            w = remaining.warning_count,
        )?;
    }
    Ok(())
}

// ------------------------------------------------------------------
// Hand-rolled unified-style line diff (no extra dependency)
// ------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum DiffOp {
    Equal,
    Delete,
    Insert,
}

/// Produce a unified-style diff between two sources. Hand-rolled: common
/// prefix/suffix are trimmed, the middle is diffed with an LCS table when
/// small enough (falls back to whole-block delete+insert for huge middles),
/// and a single hunk with `DIFF_CONTEXT` lines of context is emitted.
fn unified_diff(old: &str, new: &str, label: &str) -> String {
    if old == new {
        return String::new();
    }
    let old_lines: Vec<&str> = old.split('\n').collect();
    let new_lines: Vec<&str> = new.split('\n').collect();

    // Trim common prefix/suffix.
    let mut prefix = 0;
    while prefix < old_lines.len()
        && prefix < new_lines.len()
        && old_lines[prefix] == new_lines[prefix]
    {
        prefix += 1;
    }
    let mut suffix = 0;
    while suffix < old_lines.len() - prefix
        && suffix < new_lines.len() - prefix
        && old_lines[old_lines.len() - 1 - suffix] == new_lines[new_lines.len() - 1 - suffix]
    {
        suffix += 1;
    }
    let mid_old = &old_lines[prefix..old_lines.len() - suffix];
    let mid_new = &new_lines[prefix..new_lines.len() - suffix];

    // Ops covering the middle region.
    let mid_ops: Vec<(DiffOp, usize)> = if mid_old.len().saturating_mul(mid_new.len()) <= 1_000_000
    {
        lcs_ops(mid_old, mid_new)
    } else {
        let mut ops = Vec::with_capacity(mid_old.len() + mid_new.len());
        ops.extend((0..mid_old.len()).map(|i| (DiffOp::Delete, i)));
        ops.extend((0..mid_new.len()).map(|j| (DiffOp::Insert, j)));
        ops
    };

    // Context window inside the trimmed prefix/suffix.
    let ctx_before = prefix.min(DIFF_CONTEXT);
    let ctx_after = suffix.min(DIFF_CONTEXT);
    let old_start = prefix - ctx_before; // 0-based first old line in hunk
    let new_start = prefix - ctx_before;
    let old_count = ctx_before + mid_old.len() + ctx_after;
    let new_count = ctx_before + mid_new.len() + ctx_after;

    let mut buf = String::new();
    buf.push_str(&format!("--- {label}\n+++ {label} (fixed)\n"));
    buf.push_str(&format!(
        "@@ -{},{} +{},{} @@\n",
        old_start + 1,
        old_count,
        new_start + 1,
        new_count,
    ));
    for line in &old_lines[old_start..prefix] {
        buf.push_str(&format!(" {line}\n"));
    }
    for (op, idx) in &mid_ops {
        match op {
            DiffOp::Equal => buf.push_str(&format!(" {}\n", mid_old[*idx])),
            DiffOp::Delete => buf.push_str(&format!("-{}\n", mid_old[*idx])),
            DiffOp::Insert => buf.push_str(&format!("+{}\n", mid_new[*idx])),
        }
    }
    for line in &old_lines[prefix + mid_old.len()..prefix + mid_old.len() + ctx_after] {
        buf.push_str(&format!(" {line}\n"));
    }
    buf
}

/// Standard LCS dynamic program over the (small) middle region; returns ops
/// with indices into `old` (Equal/Delete) or `new` (Insert).
fn lcs_ops(old: &[&str], new: &[&str]) -> Vec<(DiffOp, usize)> {
    let (n, m) = (old.len(), new.len());
    // table[i][j] = LCS length of old[i..] vs new[j..]
    let mut table = vec![0u32; (n + 1) * (m + 1)];
    let idx = |i: usize, j: usize| i * (m + 1) + j;
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            table[idx(i, j)] = if old[i] == new[j] {
                table[idx(i + 1, j + 1)] + 1
            } else {
                table[idx(i + 1, j)].max(table[idx(i, j + 1)])
            };
        }
    }
    let mut ops = Vec::with_capacity(n + m);
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if old[i] == new[j] {
            ops.push((DiffOp::Equal, i));
            i += 1;
            j += 1;
        } else if table[idx(i + 1, j)] >= table[idx(i, j + 1)] {
            ops.push((DiffOp::Delete, i));
            i += 1;
        } else {
            ops.push((DiffOp::Insert, j));
            j += 1;
        }
    }
    while i < n {
        ops.push((DiffOp::Delete, i));
        i += 1;
    }
    while j < m {
        ops.push((DiffOp::Insert, j));
        j += 1;
    }
    ops
}

// ------------------------------------------------------------------
// rules
// ------------------------------------------------------------------

fn run_rules(format: RulesFormat) -> Result<u8, String> {
    let registry: &BTreeMap<String, _> = rule_registry();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    match format {
        RulesFormat::Json => {
            let rules: Vec<serde_json::Value> = registry
                .iter()
                .map(|(id, meta)| {
                    serde_json::json!({
                        "id": id,
                        "layer": meta.layer,
                        "severity": severity_name(meta.severity),
                        "fixable": meta.fixable,
                        "fix_safety": meta.fix_safety,
                        "message": meta.message,
                        "description": meta.description,
                    })
                })
                .collect();
            let envelope = serde_json::json!({
                "schema_version": JSON_SCHEMA_VERSION,
                "rules": rules,
            });
            let rendered = serde_json::to_string_pretty(&envelope)
                .map_err(|e| format!("cannot serialize JSON output: {e}"))?;
            writeln!(out, "{rendered}").map_err(stdout_err)?;
        }
        RulesFormat::Human => {
            writeln!(
                out,
                "{:<6} {:<7} {:<8} {:<10} DESCRIPTION",
                "ID", "LAYER", "SEVERITY", "FIXABLE"
            )
            .map_err(stdout_err)?;
            for (id, meta) in registry {
                let fixable = if meta.fixable {
                    meta.fix_safety.as_deref().unwrap_or("safe")
                } else {
                    "no"
                };
                writeln!(
                    out,
                    "{:<6} {:<7} {:<8} {:<10} {}",
                    id,
                    meta.layer,
                    severity_name(meta.severity),
                    fixable,
                    meta.description,
                )
                .map_err(stdout_err)?;
            }
            writeln!(
                out,
                "\nSchema-layer V-codes (V001–V343) live in src/validate.rs and are not listed here."
            )
            .map_err(stdout_err)?;
        }
    }
    Ok(EXIT_CLEAN)
}
