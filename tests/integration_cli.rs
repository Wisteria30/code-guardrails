use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const FIXTURE_ROOT: &str = "fixtures";

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn fixture(path: &str) -> PathBuf {
    project_root().join(FIXTURE_ROOT).join(path)
}

fn run_engine(args: &[&str]) -> std::process::Output {
    run_engine_in_dir(args, &project_root())
}

fn run_engine_in_dir(args: &[&str], cwd: &std::path::Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_code-guardrails-engine"))
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("failed to run code-guardrails-engine")
}

fn unique_temp_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock went backwards")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("code-guardrails-{nanos}"));
    fs::create_dir_all(&dir).expect("failed to create temp dir");
    dir
}

fn cleanup_temp_dir(path: &std::path::Path) {
    if path.exists() {
        fs::remove_dir_all(path).expect("failed to clean temp dir");
    }
}

fn stdout_lines(output: &std::process::Output) -> Vec<String> {
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(ToString::to_string)
        .collect()
}

fn collect_fixtures(expect: &str) -> Vec<(PathBuf, String)> {
    let fixture_root = project_root().join(FIXTURE_ROOT);
    let mut items = Vec::new();
    let subdir = match expect {
        "fail" => "should_fail",
        "pass" => "should_pass",
        "approved" => "approved",
        _ => panic!("unknown expect: {expect}"),
    };
    let Ok(lang_dirs) = fs::read_dir(&fixture_root) else {
        return items;
    };
    let mut lang_dirs: Vec<_> = lang_dirs.filter_map(Result::ok).collect();
    lang_dirs.sort_by_key(|e| e.file_name());
    for lang_entry in lang_dirs {
        let lang_dir = lang_entry.path();
        if !lang_dir.is_dir() {
            continue;
        }
        let Ok(group_dirs) = fs::read_dir(&lang_dir) else {
            continue;
        };
        let mut group_dirs: Vec<_> = group_dirs.filter_map(Result::ok).collect();
        group_dirs.sort_by_key(|e| e.file_name());
        for group_entry in group_dirs {
            let group_dir = group_entry.path();
            if !group_dir.is_dir() {
                continue;
            }
            let target_dir = group_dir.join(subdir);
            if !target_dir.exists() {
                continue;
            }
            let Ok(files) = fs::read_dir(&target_dir) else {
                continue;
            };
            let mut files: Vec<_> = files.filter_map(Result::ok).collect();
            files.sort_by_key(|e| e.file_name());
            for file_entry in files {
                let path = file_entry.path();
                if path.is_file()
                    && !path
                        .file_name()
                        .map_or(true, |n| n.to_string_lossy().starts_with('.'))
                {
                    let lang = lang_dir.file_name().unwrap().to_string_lossy().to_string();
                    let group = group_dir.file_name().unwrap().to_string_lossy().to_string();
                    let name = path.file_name().unwrap().to_string_lossy().to_string();
                    let test_id = format!("{lang}/{group}/{subdir}/{name}");
                    items.push((path, test_id));
                }
            }
        }
    }
    items
}

fn run_ast_grep(filepath: &Path) -> Vec<serde_json::Value> {
    let output = Command::new("ast-grep")
        .args(["scan", "--json=stream"])
        .arg(filepath)
        .current_dir(project_root())
        .output()
        .expect("failed to run ast-grep");
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

#[test]
fn changed_only_violation_exits_1() {
    let fixture = fixture("python/fallback/should_fail/or_default.py");
    let output = run_engine(&["--changed-only", fixture.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(1));
}

#[test]
fn changed_only_clean_exits_0() {
    let fixture = fixture("python/fallback/should_pass/no_fallback.py");
    let output = run_engine(&["--changed-only", fixture.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(0));
}

#[test]
fn changed_only_approved_exits_0() {
    let fixture = fixture("python/fallback/approved/approved_or_default.py");
    let output = run_engine(&["--changed-only", fixture.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(0));
}

#[test]
fn format_json_outputs_valid_json_lines() {
    let fixture = fixture("python/fallback/should_fail/or_default.py");
    let output = run_engine(&[
        "--changed-only",
        fixture.to_str().unwrap(),
        "--format",
        "json",
    ]);

    assert_eq!(output.status.code(), Some(1));
    let lines = stdout_lines(&output);
    assert!(!lines.is_empty());

    for line in lines {
        let payload: serde_json::Value =
            serde_json::from_str(&line).expect("expected valid JSON output");
        assert!(
            payload.get("policy_group").is_some(),
            "missing key: policy_group"
        );
        assert!(payload.get("count").is_some(), "missing key: count");
        let findings = payload
            .get("findings")
            .and_then(|v| v.as_array())
            .expect("missing key: findings");
        assert!(!findings.is_empty());
        for finding in findings {
            for key in [
                "file", "line", "column", "rule_id", "severity", "message", "code",
            ] {
                assert!(finding.get(key).is_some(), "missing key in finding: {key}");
            }
        }
    }
}

#[test]
fn metadata_parsing_approval_model_works() {
    let fixture = fixture("python/fallback/approved/approved_get_default.py");
    let output = run_engine(&[
        "--changed-only",
        fixture.to_str().unwrap(),
        "--format",
        "json",
    ]);

    assert_eq!(output.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&output.stdout).trim().is_empty());
}

#[test]
fn test_globs_skip_matching_files() {
    let fixture = fixture("python/fallback/should_fail/or_default.py");
    let output = run_engine(&[
        "--changed-only",
        fixture.to_str().unwrap(),
        "--test-globs",
        "**/should_fail/**",
    ]);

    assert_eq!(output.status.code(), Some(0));
}

#[test]
fn full_scan_works_when_root_differs_from_config_dir() {
    let temp_dir = unique_temp_dir();
    let sample_root = temp_dir.join("sample-project");
    fs::create_dir_all(&sample_root).expect("failed to create sample project");

    let source = fixture("typescript/fallback/should_fail/nullishCoalescing.ts");
    let target = sample_root.join("nullishCoalescing.ts");
    fs::copy(&source, &target).expect("failed to copy fixture");

    let config_dir = project_root()
        .canonicalize()
        .expect("failed to canonicalize config dir");
    let output = run_engine(&[
        sample_root.to_str().unwrap(),
        "--config-dir",
        config_dir.to_str().unwrap(),
        "--format",
        "json",
    ]);

    assert_eq!(output.status.code(), Some(1));
    let lines = stdout_lines(&output);
    assert!(!lines.is_empty());
    let payload: serde_json::Value =
        serde_json::from_str(&lines[0]).expect("expected valid JSON output");
    let findings = payload
        .get("findings")
        .and_then(|v| v.as_array())
        .expect("missing findings");
    assert_eq!(
        findings[0].get("file").and_then(|value| value.as_str()),
        Some("nullishCoalescing.ts")
    );

    cleanup_temp_dir(&temp_dir);
}

#[test]
fn fixture_should_fail_has_violations() {
    let fixtures = collect_fixtures("fail");
    assert!(!fixtures.is_empty(), "no should_fail fixtures found");
    let mut failures = Vec::new();
    for (path, test_id) in &fixtures {
        let findings = run_ast_grep(path);
        if findings.is_empty() {
            failures.push(format!("{test_id}: expected violations but got 0"));
        }
    }
    assert!(
        failures.is_empty(),
        "should_fail fixtures with no violations:\n  {}",
        failures.join("\n  ")
    );
}

#[test]
fn fixture_should_pass_has_no_violations() {
    let fixtures = collect_fixtures("pass");
    assert!(!fixtures.is_empty(), "no should_pass fixtures found");
    let mut failures = Vec::new();
    for (path, test_id) in &fixtures {
        let findings = run_ast_grep(path);
        if !findings.is_empty() {
            let rule_ids: Vec<String> = findings
                .iter()
                .filter_map(|f| f.get("ruleId").and_then(|v| v.as_str()).map(String::from))
                .collect();
            failures.push(format!(
                "{test_id}: expected 0 violations, got {}: {}",
                findings.len(),
                rule_ids.join(", ")
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "should_pass fixtures with violations:\n  {}",
        failures.join("\n  ")
    );
}

#[test]
fn fixture_approved_exits_0() {
    let fixtures = collect_fixtures("approved");
    assert!(!fixtures.is_empty(), "no approved fixtures found");
    let mut failures = Vec::new();
    for (path, test_id) in &fixtures {
        let output = run_engine(&["--changed-only", path.to_str().unwrap()]);
        if output.status.code() != Some(0) {
            failures.push(format!(
                "{test_id}: expected exit 0, got {:?}",
                output.status.code()
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "approved fixtures with non-zero exit:\n  {}",
        failures.join("\n  ")
    );
}

#[test]
fn changed_only_relative_path_uses_caller_workdir() {
    let temp_dir = unique_temp_dir();
    let sample_root = temp_dir.join("sample-project");
    fs::create_dir_all(&sample_root).expect("failed to create sample project");

    let source = fixture("python/fallback/should_fail/or_default.py");
    let target = sample_root.join("or_default.py");
    fs::copy(&source, &target).expect("failed to copy fixture");

    let config_dir = project_root()
        .canonicalize()
        .expect("failed to canonicalize config dir");
    let output = run_engine_in_dir(
        &[
            "--changed-only",
            "or_default.py",
            "--config-dir",
            config_dir.to_str().unwrap(),
            "--format",
            "json",
        ],
        &sample_root,
    );

    assert_eq!(output.status.code(), Some(1));
    let lines = stdout_lines(&output);
    assert!(!lines.is_empty());
    let payload: serde_json::Value =
        serde_json::from_str(&lines[0]).expect("expected valid JSON output");
    let findings = payload
        .get("findings")
        .and_then(|v| v.as_array())
        .expect("missing findings");
    assert_eq!(
        findings[0].get("file").and_then(|value| value.as_str()),
        Some("or_default.py")
    );

    cleanup_temp_dir(&temp_dir);
}
