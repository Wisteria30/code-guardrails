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

fn run_engine_with_stdin(args: &[&str], stdin_data: &str) -> std::process::Output {
    use std::io::Write;
    use std::process::Stdio;

    let mut child = Command::new(env!("CARGO_BIN_EXE_code-guardrails-engine"))
        .args(args)
        .current_dir(&project_root())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn engine");

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(stdin_data.as_bytes())
            .expect("failed to write to stdin");
    }
    child.wait_with_output().expect("failed to wait for engine")
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

fn run_scan_file(filepath: &Path) -> Vec<serde_json::Value> {
    let output = run_engine(&["scan-file", "--file", filepath.to_str().unwrap()]);
    let mut all_findings = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
            if let Some(findings) = v.get("findings").and_then(|f| f.as_array()) {
                all_findings.extend(findings.iter().cloned());
            }
        }
    }
    all_findings
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
fn json_output_has_valid_schema() {
    let fixture = fixture("python/fallback/should_fail/or_default.py");
    let output = run_engine(&["--changed-only", fixture.to_str().unwrap()]);

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
        let findings = payload
            .get("findings")
            .and_then(|v| v.as_array())
            .expect("missing key: findings");
        assert!(!findings.is_empty());
        for finding in findings {
            for key in ["file", "line", "rule_id", "message", "code"] {
                assert!(finding.get(key).is_some(), "missing key in finding: {key}");
            }
        }
    }
}

#[test]
fn metadata_parsing_approval_model_works() {
    let fixture = fixture("python/fallback/approved/approved_get_default.py");
    let output = run_engine(&["--changed-only", fixture.to_str().unwrap()]);

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
        let findings = run_scan_file(path);
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
        let findings = run_scan_file(path);
        if !findings.is_empty() {
            let rule_ids: Vec<String> = findings
                .iter()
                .filter_map(|f| f.get("rule_id").and_then(|v| v.as_str()).map(String::from))
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

#[test]
fn scan_hook_blocks_approval_injection_in_edit() {
    let tool_input = serde_json::json!({
        "tool_input": {
            "file_path": "app.py",
            "old_string": "x = getattr(obj, 'a', None)",
            "new_string": "x = getattr(obj, 'a', None)  # policy-approved: REQ-001 reason"
        }
    });
    let output = run_engine_with_stdin(&["scan-hook"], &tool_input.to_string());
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("engine-no-approval-injection"));
}

#[test]
fn scan_hook_blocks_approval_injection_in_write() {
    let tool_input = serde_json::json!({
        "tool_input": {
            "file_path": "app.py",
            "content": "x = val or 'default'  # policy-approved: SPEC-001 reason\n"
        }
    });
    let output = run_engine_with_stdin(&["scan-hook"], &tool_input.to_string());
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("engine-no-approval-injection"));
}

#[test]
fn scan_hook_allows_edit_without_approval_injection() {
    let clean_file = fixture("python/fallback/should_pass/no_fallback.py");
    let tool_input = serde_json::json!({
        "tool_input": {
            "file_path": clean_file.to_str().unwrap(),
            "old_string": "x = 1",
            "new_string": "x = 2"
        }
    });
    let output = run_engine_with_stdin(&["scan-hook"], &tool_input.to_string());
    assert_eq!(output.status.code(), Some(0));
}

#[test]
fn scan_hook_allows_existing_approval_comment_edits() {
    let approved_file = fixture("python/fallback/approved/approved_or_default.py");
    let tool_input = serde_json::json!({
        "tool_input": {
            "file_path": approved_file.to_str().unwrap(),
            "old_string": "x = val  # policy-approved: REQ-001 ok",
            "new_string": "y = val  # policy-approved: REQ-001 ok"
        }
    });
    let output = run_engine_with_stdin(&["scan-hook"], &tool_input.to_string());
    assert_eq!(output.status.code(), Some(0));
}
