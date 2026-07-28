//! Layer-5 integration tests for the `surf-lint` CLI binary.
//!
//! These run ONLY under `cargo test --features cli` — the bin target has
//! `required-features = ["cli"]`, and cargo only provides
//! `CARGO_BIN_EXE_surf-lint` (and builds the binary) when the feature is on.
//! Under default features this file compiles to an empty test crate.
#![cfg(feature = "cli")]

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Path to the compiled surf-lint binary (set by cargo for required-features
/// bins when testing with `--features cli`).
const BIN: &str = env!("CARGO_BIN_EXE_surf-lint");

fn fixture(name: &str) -> String {
    format!("{}/tests/fixtures/lint/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn run(args: &[&str]) -> Output {
    Command::new(BIN)
        .args(args)
        .output()
        .expect("surf-lint binary should run")
}

fn run_stdin(args: &[&str], input: &str) -> Output {
    let mut child = Command::new(BIN)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("surf-lint binary should spawn");
    child
        .stdin
        .as_mut()
        .expect("stdin is piped")
        .write_all(input.as_bytes())
        .expect("write to child stdin");
    child.wait_with_output().expect("child completes")
}

fn exit_code(output: &Output) -> i32 {
    output.status.code().expect("process not killed by signal")
}

fn stdout_str(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Unique temp dir per test invocation (no tempfile dependency).
fn temp_dir(tag: &str) -> PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "surf-lint-cli-{tag}-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn copy_fixture_to(dir: &Path, name: &str) -> PathBuf {
    let dest = dir.join(name);
    fs::copy(fixture(name), &dest).expect("copy fixture");
    dest
}

// ------------------------------------------------------------------
// Exit codes (stable contract: 0 clean, 1 errors, 2 denied warnings, 3 IO)
// ------------------------------------------------------------------

#[test]
fn check_clean_file_exits_0() {
    let out = run(&["check", &fixture("clean.surf")]);
    assert_eq!(exit_code(&out), 0, "stdout: {}", stdout_str(&out));
    assert!(stdout_str(&out).contains("clean"));
}

#[test]
fn check_warnings_without_deny_exits_0() {
    let out = run(&["check", &fixture("l001-section.surf")]);
    assert_eq!(exit_code(&out), 0);
    assert!(stdout_str(&out).contains("warning[L001]"));
}

#[test]
fn check_warnings_with_deny_exits_2() {
    let out = run(&["check", &fixture("l001-section.surf"), "--deny-warnings"]);
    assert_eq!(exit_code(&out), 2);
}

#[test]
fn check_errors_exit_1() {
    // p002 fixture has an unclosed front matter -> P002 error.
    let out = run(&["check", &fixture("p002-unclosed-frontmatter.surf")]);
    assert_eq!(exit_code(&out), 1);
    assert!(stdout_str(&out).contains("error[P002]"));
}

#[test]
fn check_missing_path_exits_3() {
    let out = run(&["check", "definitely-does-not-exist.surf"]);
    assert_eq!(exit_code(&out), 3);
    assert!(String::from_utf8_lossy(&out.stderr).contains("cannot access"));
}

#[test]
fn multi_file_exit_code_is_max_across_files() {
    // clean (0) + p002 error (1) -> aggregate 1.
    let out = run(&[
        "check",
        &fixture("clean.surf"),
        &fixture("p002-unclosed-frontmatter.surf"),
    ]);
    assert_eq!(exit_code(&out), 1);
}

#[test]
fn directory_argument_recurses_for_surf_files() {
    let dir = temp_dir("dir-recurse");
    let nested = dir.join("nested");
    fs::create_dir_all(&nested).expect("create nested dir");
    copy_fixture_to(&dir, "clean.surf");
    copy_fixture_to(&nested, "l001-section.surf");
    // A non-.surf file must be ignored.
    fs::write(dir.join("notes.txt"), "::section\n").expect("write txt");

    let out = run(&["check", dir.to_str().expect("utf-8 temp path")]);
    let stdout = stdout_str(&out);
    assert_eq!(exit_code(&out), 0, "stdout: {stdout}");
    assert!(stdout.contains("l001-section.surf"));
    assert!(stdout.contains("checked 2 files"));
    assert!(!stdout.contains("notes.txt"));

    fs::remove_dir_all(&dir).ok();
}

// ------------------------------------------------------------------
// Output formats
// ------------------------------------------------------------------

#[test]
fn json_output_matches_schema_shape() {
    let path = fixture("l001-section.surf");
    let out = run(&["check", &path, "--format", "json"]);
    assert_eq!(exit_code(&out), 0);
    let v: serde_json::Value =
        serde_json::from_str(&stdout_str(&out)).expect("stdout is valid JSON");

    assert_eq!(v["schema_version"], 1);
    let files = v["files"].as_array().expect("files array");
    assert_eq!(files.len(), 1);
    let file = &files[0];
    assert_eq!(file["path"], path);
    assert_eq!(file["error_count"], 0);
    assert_eq!(file["warning_count"], 1);
    assert_eq!(file["fixable_count"], 1);

    let diags = file["diagnostics"].as_array().expect("diagnostics array");
    let l001 = diags
        .iter()
        .find(|d| d["code"] == "L001")
        .expect("L001 diagnostic present");
    // Spans carry BOTH line numbers and byte offsets.
    let span = &l001["span"];
    assert!(span["start_line"].is_u64());
    assert!(span["end_line"].is_u64());
    assert!(span["start_offset"].is_u64());
    assert!(span["end_offset"].is_u64());
    assert_eq!(l001["severity"], "warning");
    // Fixable diagnostics serialize their fix payload (edits + safety).
    assert_eq!(l001["fix"]["safety"], "safe");
    assert!(
        l001["fix"]["edits"]
            .as_array()
            .is_some_and(|e| !e.is_empty())
    );

    let summary = &v["summary"];
    assert_eq!(summary["files"], 1);
    assert_eq!(summary["warning_count"], 1);
    assert_eq!(summary["fixable_count"], 1);
}

#[test]
fn github_format_emits_workflow_annotations() {
    let path = fixture("l001-section.surf");
    let out = run(&["check", &path, "--format", "github"]);
    let stdout = stdout_str(&out);
    let line = stdout
        .lines()
        .find(|l| l.contains("L001"))
        .expect("an L001 annotation line");
    assert!(
        line.starts_with(&format!("::warning file={path},line=")),
        "line: {line}"
    );
    assert!(line.contains("::L001: "), "line: {line}");
}

#[test]
fn github_format_uses_error_kind_for_errors() {
    let path = fixture("p002-unclosed-frontmatter.surf");
    let out = run(&["check", &path, "--format", "github"]);
    assert_eq!(exit_code(&out), 1);
    assert!(stdout_str(&out).contains(&format!("::error file={path},line=1::P002: ")));
}

#[test]
fn human_format_shows_excerpt_caret_and_fix_hint() {
    let out = run(&["check", &fixture("l001-section.surf")]);
    let stdout = stdout_str(&out);
    assert!(stdout.contains("warning[L001]"), "stdout: {stdout}");
    assert!(
        stdout.contains("::section[title="),
        "excerpt missing: {stdout}"
    );
    assert!(stdout.contains("^^^"), "caret underline missing: {stdout}");
    assert!(
        stdout.contains("fix available:"),
        "fix hint missing: {stdout}"
    );
    assert!(
        stdout.contains("checked 1 file:"),
        "summary missing: {stdout}"
    );
}

// ------------------------------------------------------------------
// fix
// ------------------------------------------------------------------

#[test]
fn fix_dry_run_leaves_file_untouched() {
    let dir = temp_dir("dry-run");
    let target = copy_fixture_to(&dir, "l003-curly-attrs.surf");
    let before = fs::read_to_string(&target).expect("read target");

    let out = run(&["fix", target.to_str().expect("utf-8"), "--dry-run"]);
    let after = fs::read_to_string(&target).expect("read target");
    assert_eq!(before, after, "--dry-run must not modify the file");
    assert!(stdout_str(&out).contains("would apply 1 fix(es)"));
    assert!(stdout_str(&out).contains("[L003]"));
    // No remaining errors in the would-be-fixed source -> 0.
    assert_eq!(exit_code(&out), 0);

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn fix_writes_repaired_file() {
    let dir = temp_dir("fix-write");
    let target = copy_fixture_to(&dir, "l003-curly-attrs.surf");

    let out = run(&["fix", target.to_str().expect("utf-8")]);
    assert_eq!(exit_code(&out), 0, "stdout: {}", stdout_str(&out));
    assert!(stdout_str(&out).contains("applied 1 fix(es)"));

    let fixed = fs::read_to_string(&target).expect("read fixed file");
    let expected =
        fs::read_to_string(fixture("l003-curly-attrs.fixed.surf")).expect("read companion");
    assert_eq!(
        fixed, expected,
        "output must match the .fixed.surf companion"
    );

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn fix_diff_shows_unified_style_changes() {
    let dir = temp_dir("fix-diff");
    let target = copy_fixture_to(&dir, "l003-curly-attrs.surf");

    let out = run(&[
        "fix",
        target.to_str().expect("utf-8"),
        "--dry-run",
        "--diff",
    ]);
    let stdout = stdout_str(&out);
    assert!(stdout.contains("@@ "), "hunk header missing: {stdout}");
    assert!(
        stdout.contains("-::callout{type=info}"),
        "deleted line missing: {stdout}"
    );
    assert!(
        stdout.contains("+::callout[type=info]"),
        "inserted line missing: {stdout}"
    );

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn fix_suggested_tier_applies_only_with_flag() {
    // l010 fixture's only fix is the Suggested-tier summary stub.
    let dir = temp_dir("fix-suggested");
    let target = copy_fixture_to(&dir, "l010-missing-summary.surf");
    let path = target.to_str().expect("utf-8");

    let safe_only = run(&["fix", path, "--dry-run"]);
    assert!(stdout_str(&safe_only).contains("no applicable fixes"));

    let with_suggested = run(&["fix", path, "--dry-run", "--fix-suggested"]);
    assert!(stdout_str(&with_suggested).contains("would apply"));
    assert!(stdout_str(&with_suggested).contains("[L010]"));

    fs::remove_dir_all(&dir).ok();
}

// ------------------------------------------------------------------
// stdin
// ------------------------------------------------------------------

#[test]
fn check_reads_stdin_with_dash() {
    let out = run_stdin(&["check", "-"], "::section[title=\"X\"]\nbody\n::\n");
    assert_eq!(exit_code(&out), 0);
    let stdout = stdout_str(&out);
    assert!(
        stdout.contains("<stdin>:1: warning[L001]"),
        "stdout: {stdout}"
    );
}

#[test]
fn fix_stdin_writes_fixed_source_to_stdout() {
    let input =
        "---\ntitle: \"T\"\ntype: report\nstatus: active\n---\n\n::callout{type=info}\nhi\n::\n";
    let out = run_stdin(&["fix", "-"], input);
    assert_eq!(exit_code(&out), 0);
    let stdout = stdout_str(&out);
    assert!(stdout.contains("::callout[type=info]"), "stdout: {stdout}");
    assert!(
        !stdout.contains('{'),
        "curly braces should be fixed: {stdout}"
    );
}

// ------------------------------------------------------------------
// rules
// ------------------------------------------------------------------

#[test]
fn rules_lists_l001_in_table() {
    let out = run(&["rules"]);
    assert_eq!(exit_code(&out), 0);
    let stdout = stdout_str(&out);
    assert!(stdout.contains("L001"), "stdout: {stdout}");
    assert!(stdout.contains("ID"), "header missing: {stdout}");
}

#[test]
fn rules_json_has_schema_and_l001() {
    let out = run(&["rules", "--format", "json"]);
    assert_eq!(exit_code(&out), 0);
    let v: serde_json::Value =
        serde_json::from_str(&stdout_str(&out)).expect("stdout is valid JSON");
    assert_eq!(v["schema_version"], 1);
    let rules = v["rules"].as_array().expect("rules array");
    let l001 = rules
        .iter()
        .find(|r| r["id"] == "L001")
        .expect("L001 in registry");
    assert_eq!(l001["layer"], "style");
    assert_eq!(l001["severity"], "warning");
    assert_eq!(l001["fixable"], true);
    assert_eq!(l001["fix_safety"], "safe");
}

// ------------------------------------------------------------------
// usage errors
// ------------------------------------------------------------------

#[test]
fn bad_usage_exits_3_not_2() {
    // 2 is reserved for --deny-warnings; usage errors are internal (3).
    let out = run(&["check", &fixture("clean.surf"), "--format", "nope"]);
    assert_eq!(exit_code(&out), 3);
}

#[test]
fn help_exits_0() {
    let out = run(&["--help"]);
    assert_eq!(exit_code(&out), 0);
    assert!(stdout_str(&out).contains("surf-lint"));
}

// ------------------------------------------------------------------
// .surflint.toml configuration (chunk 6)
// ------------------------------------------------------------------

fn write_config(dir: &Path, body: &str) -> PathBuf {
    let path = dir.join(".surflint.toml");
    fs::write(&path, body).expect("write config");
    path
}

#[test]
fn config_severity_override_is_discovered_next_to_the_file() {
    let dir = temp_dir("cfg-discover");
    let file = copy_fixture_to(&dir, "l001-section.surf");
    write_config(&dir, "[severity]\nL001 = \"error\"\n");
    let out = run(&["check", file.to_str().unwrap()]);
    assert_eq!(exit_code(&out), 1, "L001 promoted to error → exit 1");
    assert!(stdout_str(&out).contains("error[L001]"));
}

#[test]
fn config_discovery_walks_up_to_a_parent_directory() {
    let dir = temp_dir("cfg-walkup");
    let sub = dir.join("nested").join("deeper");
    fs::create_dir_all(&sub).expect("create nested dirs");
    write_config(&dir, "[severity]\nL001 = \"error\"\n");
    let file = sub.join("doc.surf");
    fs::copy(fixture("l001-section.surf"), &file).expect("copy fixture");
    let out = run(&["check", file.to_str().unwrap()]);
    assert_eq!(exit_code(&out), 1, "config found in grandparent dir");
}

#[test]
fn config_off_disables_a_rule() {
    let dir = temp_dir("cfg-off");
    let file = copy_fixture_to(&dir, "l001-section.surf");
    write_config(&dir, "[severity]\nL001 = \"off\"\n");
    let out = run(&["check", file.to_str().unwrap()]);
    assert_eq!(exit_code(&out), 0);
    assert!(
        !stdout_str(&out).contains("L001"),
        "L001 must be disabled: {}",
        stdout_str(&out)
    );
}

#[test]
fn no_config_flag_ignores_a_discovered_config() {
    let dir = temp_dir("cfg-noconfig");
    let file = copy_fixture_to(&dir, "l001-section.surf");
    write_config(&dir, "[severity]\nL001 = \"error\"\n");
    let out = run(&["check", file.to_str().unwrap(), "--no-config"]);
    assert_eq!(exit_code(&out), 0, "default severities with --no-config");
    assert!(stdout_str(&out).contains("warning[L001]"));
}

#[test]
fn explicit_config_path_overrides_discovery() {
    let dir = temp_dir("cfg-explicit");
    let file = copy_fixture_to(&dir, "l001-section.surf");
    // Discovered config says error; the explicit one says off.
    write_config(&dir, "[severity]\nL001 = \"error\"\n");
    let other = dir.join("other.toml");
    fs::write(&other, "[severity]\nL001 = \"off\"\n").expect("write config");
    let out = run(&[
        "check",
        file.to_str().unwrap(),
        "--config",
        other.to_str().unwrap(),
    ]);
    assert_eq!(exit_code(&out), 0);
    assert!(!stdout_str(&out).contains("L001"));
}

#[test]
fn malformed_config_exits_3_with_message() {
    let dir = temp_dir("cfg-bad-toml");
    let file = copy_fixture_to(&dir, "clean.surf");
    write_config(&dir, "[severity\nL001 = error");
    let out = run(&["check", file.to_str().unwrap()]);
    assert_eq!(exit_code(&out), 3);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("invalid config"), "stderr: {stderr}");
    assert!(stderr.contains(".surflint.toml"), "stderr: {stderr}");
}

#[test]
fn config_with_bad_severity_value_exits_3() {
    let dir = temp_dir("cfg-bad-value");
    let file = copy_fixture_to(&dir, "clean.surf");
    write_config(&dir, "[severity]\nL001 = \"loud\"\n");
    let out = run(&["check", file.to_str().unwrap()]);
    assert_eq!(exit_code(&out), 3);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("expected"), "stderr: {stderr}");
}

#[test]
fn config_extra_types_suppresses_p005() {
    let dir = temp_dir("cfg-extra-types");
    let file = dir.join("legacy.surf");
    fs::write(&file, "---\ntitle: T\ntype: checkpoint\n---\nBody text.\n").expect("write doc");
    // Without config: P005 warning fires.
    let bare = run(&["check", file.to_str().unwrap(), "--no-config"]);
    assert_eq!(exit_code(&bare), 0);
    assert!(stdout_str(&bare).contains("warning[P005]"));
    // With the legacy value allowed: clean.
    write_config(&dir, "[frontmatter]\nextra_types = [\"checkpoint\"]\n");
    let out = run(&["check", file.to_str().unwrap()]);
    assert_eq!(exit_code(&out), 0);
    assert!(
        stdout_str(&out).contains("clean"),
        "stdout: {}",
        stdout_str(&out)
    );
}

#[test]
fn config_applies_to_fix_as_well() {
    let dir = temp_dir("cfg-fix");
    let file = copy_fixture_to(&dir, "l001-section.surf");
    let original = fs::read_to_string(&file).expect("read fixture");
    write_config(&dir, "[severity]\nL001 = \"off\"\n");
    let out = run(&["fix", file.to_str().unwrap()]);
    assert_eq!(exit_code(&out), 0);
    let after = fs::read_to_string(&file).expect("read back");
    assert_eq!(after, original, "disabled rule must not be fixed");
}
