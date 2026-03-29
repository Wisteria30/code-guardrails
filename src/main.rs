use std::collections::{BTreeSet, HashMap, VecDeque};
use std::env;
use std::fs;
use std::io::{self, BufRead, BufReader, Read as _, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use glob::Pattern;
use regex::Regex;
use serde::Deserialize;
use serde_json::json;

const APPROVAL_MODE_ADJACENT: &str = "adjacent_policy_comment";
const DEFAULT_TEST_GLOBS: &[&str] = &[
    "**/test/**",
    "**/tests/**",
    "**/*_test.py",
    "**/test_*.py",
    "**/conftest.py",
    "**/*.test.ts",
    "**/*.spec.ts",
    "**/__tests__/**",
];
const BATCH_SIZE: usize = 128;
const SUSPICIOUS_KEYWORDS: &str = "mock|stub|fake|fallback";
const RG_CANDIDATE_PATTERN: &str = r"mock|stub|fake|fallback|unittest\.mock|= .* or |\?\?|\|\||except.*pass|catch|suppress|getattr\(|getenv\(|os\.environ\.get\(|\.get\(|next\(|process\.env|@testing-library|vitest|from 'jest'";

fn approval_regex() -> &'static Regex {
    static APPROVAL_RE: OnceLock<Regex> = OnceLock::new();
    APPROVAL_RE.get_or_init(|| {
        Regex::new(r"(?i)policy-approved:\s*(REQ|ADR|SPEC)-[A-Za-z0-9._-]+").unwrap()
    })
}

// ---------------------------------------------------------------------------
// Policy registry: loads ownership.yml + defaults.yml for owner-layer guessing
// and registry-backed approval validation.
// ---------------------------------------------------------------------------

struct DefaultEntry {
    #[allow(dead_code)]
    symbol: String,
    #[allow(dead_code)]
    allowed_layers: Vec<String>,
}

struct PolicyRegistry {
    ownership: Vec<(String, Vec<Pattern>)>,
    defaults: HashMap<String, DefaultEntry>,
    defaults_file_exists: bool,
}

impl PolicyRegistry {
    fn load(policy_dir: &Path) -> Self {
        let ownership = Self::load_ownership(&policy_dir.join("ownership.yml"));
        let defaults_path = policy_dir.join("defaults.yml");
        let defaults_file_exists = defaults_path.is_file();
        let defaults = Self::load_defaults(&defaults_path);
        Self {
            ownership,
            defaults,
            defaults_file_exists,
        }
    }

    fn empty() -> Self {
        Self {
            ownership: Vec::new(),
            defaults: HashMap::new(),
            defaults_file_exists: false,
        }
    }

    fn load_ownership(path: &Path) -> Vec<(String, Vec<Pattern>)> {
        let text = match fs::read_to_string(path) {
            Ok(t) => t,
            Err(_) => return Vec::new(),
        };
        let mut layers = Vec::new();
        let mut current_layer: Option<String> = None;
        let mut current_patterns: Vec<Pattern> = Vec::new();
        let mut in_layers = false;

        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if trimmed == "layers:" {
                in_layers = true;
                continue;
            }
            if !in_layers {
                continue;
            }
            // Indented layer name (e.g., "  boundary:")
            if !line.starts_with("    ") && line.starts_with("  ") && trimmed.ends_with(':') {
                if let Some(layer) = current_layer.take() {
                    layers.push((layer, std::mem::take(&mut current_patterns)));
                }
                current_layer = Some(trimmed.trim_end_matches(':').to_string());
                continue;
            }
            // Pattern line (e.g., "    - src/api/**")
            if line.starts_with("    ") {
                if let Some(item) = trimmed.strip_prefix("- ") {
                    let pattern_str = strip_yaml_scalar(item);
                    if let Ok(p) = Pattern::new(&pattern_str) {
                        current_patterns.push(p);
                    }
                }
            }
        }
        if let Some(layer) = current_layer {
            layers.push((layer, current_patterns));
        }
        layers
    }

    fn load_defaults(path: &Path) -> HashMap<String, DefaultEntry> {
        let text = match fs::read_to_string(path) {
            Ok(t) => t,
            Err(_) => return HashMap::new(),
        };
        let mut defaults = HashMap::new();
        let mut in_defaults = false;
        let mut current_id: Option<String> = None;
        let mut current_symbol = String::new();
        let mut current_layers: Vec<String> = Vec::new();

        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if trimmed == "defaults:" || trimmed == "defaults: {}" {
                in_defaults = trimmed == "defaults:";
                continue;
            }
            if !in_defaults {
                continue;
            }
            // ID line (e.g., "  REQ-123:")
            if !line.starts_with("    ") && line.starts_with("  ") && trimmed.ends_with(':') {
                if let Some(id) = current_id.take() {
                    defaults.insert(
                        id,
                        DefaultEntry {
                            symbol: std::mem::take(&mut current_symbol),
                            allowed_layers: std::mem::take(&mut current_layers),
                        },
                    );
                }
                current_id = Some(trimmed.trim_end_matches(':').to_string());
                continue;
            }
            // Fields (e.g., "    symbol: LocalePolicy.default_locale")
            if line.starts_with("    ") {
                if let Some((key, value)) = trimmed.split_once(':') {
                    let key = key.trim();
                    let value = strip_yaml_scalar(value);
                    match key {
                        "symbol" => current_symbol = value,
                        "allowed_layers" => {
                            current_layers = value
                                .trim_start_matches('[')
                                .trim_end_matches(']')
                                .split(',')
                                .map(|s| s.trim().to_string())
                                .filter(|s| !s.is_empty())
                                .collect();
                        }
                        _ => {}
                    }
                }
            }
        }
        if let Some(id) = current_id {
            defaults.insert(
                id,
                DefaultEntry {
                    symbol: current_symbol,
                    allowed_layers: current_layers,
                },
            );
        }
        defaults
    }

    fn guess_layer(&self, file: &Path, root: &Path) -> Option<String> {
        let relative = file
            .strip_prefix(root)
            .unwrap_or(file)
            .to_string_lossy()
            .replace('\\', "/");
        for (layer, patterns) in &self.ownership {
            for pattern in patterns {
                if pattern.matches(&relative) {
                    return Some(layer.clone());
                }
            }
        }
        None
    }

    fn is_registered(&self, approval_id: &str) -> bool {
        // If defaults.yml does not exist, fail-open (accept any ID).
        // If it exists (even if empty), only registered IDs are valid.
        !self.defaults_file_exists || self.defaults.contains_key(approval_id)
    }
}

fn keyword_comment_regex() -> &'static Regex {
    static KEYWORD_COMMENT_RE: OnceLock<Regex> = OnceLock::new();
    KEYWORD_COMMENT_RE
        .get_or_init(|| Regex::new(&format!(r"(?i)\b({SUSPICIOUS_KEYWORDS})\b")).unwrap())
}

fn python_or_regex() -> &'static Regex {
    static PYTHON_OR_RE: OnceLock<Regex> = OnceLock::new();
    PYTHON_OR_RE.get_or_init(|| Regex::new(r"(?m)\bor\b").unwrap())
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--version") {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return;
    }
    let exit_code = match run(args) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("{message}");
            2
        }
    };
    std::process::exit(exit_code);
}

fn run(args: Vec<String>) -> Result<i32, String> {
    let cli = Cli::parse(args)?;
    let catalog = RuleCatalog::load(&cli.common.config_dir)?;

    // Load policy registry only if --policy-dir is explicitly provided.
    // The plugin ships policy/ as templates for users to customize;
    // auto-discovering it would activate strict mode prematurely.
    let registry = match &cli.common.policy_dir {
        Some(dir) if dir.is_dir() => PolicyRegistry::load(dir),
        _ => PolicyRegistry::empty(),
    };

    // Determine scan root for owner-layer guessing
    let scan_root = match &cli.mode {
        Mode::ScanTree { root } => root.canonicalize().unwrap_or_else(|_| root.clone()),
        _ => env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    };

    let findings = match cli.mode {
        Mode::ScanFile { file } => scan_file(&cli.common, &catalog, &file)?,
        Mode::ScanTree { root } => scan_tree(&cli.common, &catalog, &root)?,
        Mode::ScanHook => {
            let mut input = String::new();
            io::stdin()
                .read_to_string(&mut input)
                .map_err(|e| format!("failed to read stdin: {e}"))?;
            let ti: serde_json::Value = serde_json::from_str(&input)
                .map_err(|e| format!("failed to parse stdin JSON: {e}"))?;
            let ti = &ti["tool_input"];
            if let Some(injection) = detect_approval_injection(ti) {
                vec![injection]
            } else {
                let file_path = extract_hook_file_path(ti);
                if file_path.is_empty() {
                    return Ok(0);
                }
                scan_file(&cli.common, &catalog, &PathBuf::from(&file_path))?
            }
        }
    };

    // Compute owner_guess for each finding
    let findings: Vec<Finding> = findings
        .into_iter()
        .map(|mut f| {
            f.owner_guess = registry.guess_layer(&f.canonical_file, &scan_root);
            f
        })
        .collect();

    // Choke point suppression: if a rule has allowed_layers metadata and the
    // file IS in one of those layers, suppress the violation (it's allowed there).
    let findings: Vec<Finding> = findings
        .into_iter()
        .filter(|f| {
            if let Some(allowed) = f.metadata.get("allowed_layers") {
                let layers: Vec<&str> = allowed.split(',').map(str::trim).collect();
                !matches!(&f.owner_guess, Some(layer) if layers.contains(&layer.as_str()))
            } else {
                true
            }
        })
        .collect();

    let mut cache = HashMap::new();
    let mut unsuppressed = Vec::new();

    for finding in findings {
        if !is_approved(&finding, &mut cache, &registry) {
            unsuppressed.push(finding);
        }
    }

    format_json(&unsuppressed)?;

    Ok(if unsuppressed.is_empty() { 0 } else { 1 })
}

#[derive(Clone)]
struct CommonOptions {
    config_dir: PathBuf,
    ast_grep_bin: String,
    test_globs: Vec<String>,
    policy_dir: Option<PathBuf>,
}

#[allow(clippy::enum_variant_names)]
enum Mode {
    ScanFile { file: PathBuf },
    ScanTree { root: PathBuf },
    ScanHook,
}

struct Cli {
    common: CommonOptions,
    mode: Mode,
}

impl Cli {
    fn parse(args: Vec<String>) -> Result<Self, String> {
        let script_dir = env::current_exe()
            .map_err(|err| format!("failed to resolve current executable: {err}"))?
            .parent()
            .ok_or("failed to resolve executable directory")?
            .to_path_buf();

        let mut common = CommonOptions {
            config_dir: find_default_config_dir(&script_dir),
            ast_grep_bin: "ast-grep".to_string(),
            test_globs: DEFAULT_TEST_GLOBS
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
            policy_dir: None,
        };

        let mut iter: VecDeque<String> = args.into();
        let mut changed_only: Option<PathBuf> = None;
        let mut positional_root: Option<PathBuf> = None;
        let mut mode: Option<Mode> = None;

        while let Some(arg) = iter.pop_front() {
            match arg.as_str() {
                "scan-file" => {
                    let file = parse_subcommand_path(&mut iter, "--file")?;
                    mode = Some(Mode::ScanFile { file });
                }
                "scan-tree" => {
                    let root = parse_optional_subcommand_path(&mut iter, "--root")?
                        .unwrap_or_else(|| PathBuf::from("."));
                    mode = Some(Mode::ScanTree { root });
                }
                "scan-hook" => {
                    mode = Some(Mode::ScanHook);
                }
                "--ast-grep-bin" => {
                    common.ast_grep_bin = next_value(&mut iter, "--ast-grep-bin")?;
                }
                "--changed-only" => {
                    changed_only = Some(PathBuf::from(next_value(&mut iter, "--changed-only")?));
                }
                "--test-globs" => {
                    common.test_globs = next_value(&mut iter, "--test-globs")?
                        .split(',')
                        .filter(|part| !part.trim().is_empty())
                        .map(|part| part.trim().to_string())
                        .collect();
                }
                "--config-dir" => {
                    common.config_dir = PathBuf::from(next_value(&mut iter, "--config-dir")?);
                }
                "--policy-dir" => {
                    common.policy_dir = Some(PathBuf::from(next_value(&mut iter, "--policy-dir")?));
                }
                value if value.starts_with('-') => {
                    return Err(format!("unknown option: {value}"));
                }
                value => {
                    if positional_root.is_some() {
                        return Err("only one root path can be provided".to_string());
                    }
                    positional_root = Some(PathBuf::from(value));
                }
            }
        }

        common.config_dir = common
            .config_dir
            .canonicalize()
            .map_err(|err| format!("failed to resolve config dir: {err}"))?;

        let mode = match mode {
            Some(mode) => mode,
            None => match changed_only {
                Some(file) => Mode::ScanFile { file },
                None => Mode::ScanTree {
                    root: positional_root.unwrap_or_else(|| PathBuf::from(".")),
                },
            },
        };

        Ok(Self { common, mode })
    }
}

fn find_default_config_dir(script_dir: &Path) -> PathBuf {
    for candidate in script_dir.ancestors() {
        if candidate.join("sgconfig.yml").is_file() {
            return candidate.to_path_buf();
        }
    }
    if let Ok(cwd) = env::current_dir() {
        for candidate in cwd.ancestors() {
            if candidate.join("sgconfig.yml").is_file() {
                return candidate.to_path_buf();
            }
        }
    }
    script_dir.to_path_buf()
}

fn parse_subcommand_path(
    iter: &mut VecDeque<String>,
    expected_flag: &str,
) -> Result<PathBuf, String> {
    match iter.pop_front() {
        Some(flag) if flag == expected_flag => Ok(PathBuf::from(next_value(iter, expected_flag)?)),
        Some(flag) => Err(format!("expected {expected_flag}, got {flag}")),
        None => Err(format!("missing {expected_flag}")),
    }
}

fn parse_optional_subcommand_path(
    iter: &mut VecDeque<String>,
    expected_flag: &str,
) -> Result<Option<PathBuf>, String> {
    match iter.front() {
        Some(flag) if flag == expected_flag => {
            iter.pop_front();
            Ok(Some(PathBuf::from(next_value(iter, expected_flag)?)))
        }
        Some(_) | None => Ok(None),
    }
}

fn next_value(iter: &mut VecDeque<String>, option: &str) -> Result<String, String> {
    iter.pop_front()
        .ok_or_else(|| format!("missing value for {option}"))
}

#[derive(Clone)]
struct RuleInfo {
    path: PathBuf,
    metadata: HashMap<String, String>,
}

struct RuleCatalog {
    by_id: HashMap<String, RuleInfo>,
}

impl RuleCatalog {
    fn load(config_dir: &Path) -> Result<Self, String> {
        let sgconfig = config_dir.join("sgconfig.yml");
        let mut rule_dirs = vec!["rules".to_string()];

        if sgconfig.exists() {
            let doc = fs::read_to_string(&sgconfig)
                .map_err(|err| format!("failed to read {}: {err}", sgconfig.display()))?;
            let parsed = parse_rule_dirs(&doc);
            if !parsed.is_empty() {
                rule_dirs = parsed;
            }
        }

        let mut by_id = HashMap::new();
        for rule_dir in rule_dirs {
            let dir = config_dir.join(&rule_dir);
            if !dir.is_dir() {
                continue;
            }
            let entries = fs::read_dir(&dir)
                .map_err(|err| format!("failed to read {}: {err}", dir.display()))?;
            for entry in entries {
                let path = entry
                    .map_err(|err| {
                        format!("failed to read rule entry in {}: {err}", dir.display())
                    })?
                    .path();
                if path.extension().and_then(|ext| ext.to_str()) != Some("yml") {
                    continue;
                }
                let text = fs::read_to_string(&path)
                    .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
                let Some((rule_id, metadata)) = parse_rule_file(&text) else {
                    continue;
                };
                by_id.insert(
                    rule_id,
                    RuleInfo {
                        path: path.canonicalize().unwrap_or(path),
                        metadata,
                    },
                );
            }
        }
        Ok(Self { by_id })
    }

    fn rule_paths<'a>(&'a self, ids: impl IntoIterator<Item = &'a str>) -> Vec<PathBuf> {
        ids.into_iter()
            .filter_map(|id| self.by_id.get(id).map(|rule| rule.path.clone()))
            .collect()
    }

    fn metadata_for(&self, rule_id: &str) -> HashMap<String, String> {
        self.by_id
            .get(rule_id)
            .map(|rule| rule.metadata.clone())
            .unwrap_or_default()
    }
}

fn parse_rule_dirs(text: &str) -> Vec<String> {
    let mut rule_dirs = Vec::new();
    let mut in_rule_dirs = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed == "ruleDirs:" {
            in_rule_dirs = true;
            continue;
        }
        if in_rule_dirs {
            if let Some(item) = trimmed.strip_prefix("- ") {
                rule_dirs.push(strip_yaml_scalar(item));
                continue;
            }
            break;
        }
    }
    rule_dirs
}

fn parse_rule_file(text: &str) -> Option<(String, HashMap<String, String>)> {
    let mut rule_id: Option<String> = None;
    let mut metadata = HashMap::new();
    let mut in_metadata = false;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if !line.starts_with(' ') && !line.starts_with('\t') {
            in_metadata = trimmed == "metadata:";
            if let Some(value) = trimmed.strip_prefix("id:") {
                rule_id = Some(strip_yaml_scalar(value));
            }
            continue;
        }

        if in_metadata {
            if let Some((key, value)) = trimmed.split_once(':') {
                metadata.insert(key.trim().to_string(), strip_yaml_scalar(value));
            }
        }
    }

    rule_id.map(|rule_id| (rule_id, metadata))
}

fn strip_yaml_scalar(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .to_string()
}

#[derive(Clone)]
struct Finding {
    display_file: String,
    canonical_file: PathBuf,
    line0: usize,
    rule_id: String,
    message: String,
    text: String,
    metadata: HashMap<String, String>,
    owner_guess: Option<String>,
}

impl Finding {
    fn snippet(&self) -> String {
        self.text
            .trim()
            .replace('\n', " ")
            .chars()
            .take(200)
            .collect()
    }
}

#[derive(Deserialize)]
struct AstGrepFinding {
    #[serde(rename = "file")]
    file_path: String,
    #[serde(rename = "range")]
    range: AstRange,
    #[serde(rename = "ruleId")]
    rule_id: Option<String>,
    message: Option<String>,
    text: Option<String>,
    metadata: Option<HashMap<String, String>>,
}

#[derive(Deserialize)]
struct AstRange {
    start: AstPosition,
}

#[derive(Deserialize)]
struct AstPosition {
    line: usize,
}

fn scan_file(
    common: &CommonOptions,
    catalog: &RuleCatalog,
    file: &Path,
) -> Result<Vec<Finding>, String> {
    let test_matcher = build_patterns(&common.test_globs)?;
    let scan_root = env::current_dir().map_err(|err| format!("failed to read cwd: {err}"))?;
    let canonical_file = file
        .canonicalize()
        .map_err(|err| format!("failed to resolve {}: {err}", file.display()))?;

    if matches_test_globs(&test_matcher, &canonical_file, &scan_root) {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(&canonical_file)
        .map_err(|err| format!("failed to read {}: {err}", canonical_file.display()))?;

    let mut findings = Vec::new();

    // rg-based keyword-in-comment detection
    // Prefilter: skip rg spawn when no keywords in file (~4ms savings on clean files)
    if keyword_comment_regex().is_match(&content) {
        findings.extend(ripgrep_keyword_comments(
            std::slice::from_ref(&canonical_file),
            &scan_root,
        )?);
    }

    // ast-grep-based detection
    let selected_ids = detect_rule_ids(&canonical_file, &content);
    if !selected_ids.is_empty() {
        let rule_paths = catalog.rule_paths(selected_ids.iter().map(String::as_str));
        let inline_rules = read_inline_rules(&rule_paths)?;
        findings.extend(run_ast_grep(
            common,
            catalog,
            &scan_root,
            &[canonical_file],
            &inline_rules,
        )?);
    }

    Ok(findings)
}

fn scan_tree(
    common: &CommonOptions,
    catalog: &RuleCatalog,
    root: &Path,
) -> Result<Vec<Finding>, String> {
    let scan_root = root
        .canonicalize()
        .map_err(|err| format!("failed to resolve {}: {err}", root.display()))?;
    let test_matcher = build_patterns(&common.test_globs)?;
    let mut groups: HashMap<Vec<String>, Vec<PathBuf>> = HashMap::new();
    let mut all_source_files: Vec<PathBuf> = Vec::new();

    for path in ripgrep_candidate_files(&scan_root)? {
        if !is_supported_source(&path) || matches_test_globs(&test_matcher, &path, &scan_root) {
            continue;
        }

        all_source_files.push(path.clone());

        let content = fs::read_to_string(&path)
            .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
        let selected_ids = detect_rule_ids(&path, &content);
        if selected_ids.is_empty() {
            continue;
        }
        groups.entry(selected_ids).or_default().push(path);
    }

    let mut findings = Vec::new();

    // rg-based keyword-in-comment detection (single rg call for all files)
    findings.extend(ripgrep_keyword_comments(&all_source_files, &scan_root)?);

    // ast-grep-based detection
    for (rule_ids, files) in groups {
        let rule_paths = catalog.rule_paths(rule_ids.iter().map(String::as_str));
        let inline_rules = read_inline_rules(&rule_paths)?;
        for chunk in files.chunks(BATCH_SIZE) {
            findings.extend(run_ast_grep(
                common,
                catalog,
                &scan_root,
                chunk,
                &inline_rules,
            )?);
        }
    }
    Ok(findings)
}

fn ripgrep_candidate_files(scan_root: &Path) -> Result<Vec<PathBuf>, String> {
    let output = Command::new("rg")
        .args([
            "--files-with-matches",
            "-e",
            RG_CANDIDATE_PATTERN,
            "-g",
            "*.py",
            "-g",
            "*.ts",
            "-g",
            "*.cts",
            "-g",
            "*.mts",
            ".",
        ])
        .current_dir(scan_root)
        .output()
        .map_err(|err| format!("failed to execute ripgrep: {err}"))?;

    check_exit_ok(&output, "ripgrep candidate scan failed")?;

    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| {
            scan_root
                .join(line)
                .canonicalize()
                .unwrap_or_else(|_| scan_root.join(line))
        })
        .collect())
}

fn build_patterns(patterns: &[String]) -> Result<Vec<Pattern>, String> {
    patterns
        .iter()
        .map(|pattern| {
            Pattern::new(pattern).map_err(|err| format!("invalid test glob {pattern}: {err}"))
        })
        .collect()
}

fn matches_test_globs(matcher: &[Pattern], path: &Path, scan_root: &Path) -> bool {
    let relative = path.strip_prefix(scan_root).unwrap_or(path);
    matcher.iter().any(|pattern| pattern.matches_path(relative))
}

fn is_supported_source(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("py" | "ts" | "cts" | "mts")
    )
}

fn detect_rule_ids(path: &Path, content: &str) -> Vec<String> {
    let mut ids = BTreeSet::new();
    let lower = content.to_ascii_lowercase();
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default();
    match extension {
        "py" => {
            if contains_any(&lower, &["mock", "stub", "fake"]) {
                ids.insert("py-no-test-double-identifier".to_string());
            }
            if lower.contains("fallback") {
                ids.insert("py-no-fallback-identifier".to_string());
            }
            if lower.contains("unittest.mock") {
                ids.insert("py-no-test-double-unittest-mock".to_string());
            }
            if lower.contains("except") && lower.contains("pass") {
                ids.insert("py-no-swallowing-except-pass".to_string());
            }
            if lower.contains('=') && python_or_regex().is_match(&lower) {
                ids.insert("py-no-fallback-bool-or".to_string());
            }
            if lower.contains(".get") {
                ids.insert("py-no-fallback-get-default".to_string());
            }
            if lower.contains("getattr(") {
                ids.insert("py-no-fallback-getattr-default".to_string());
            }
            if lower.contains("next(") {
                ids.insert("py-no-fallback-next-default".to_string());
            }
            if lower.contains("getenv(") || lower.contains("os.environ.get(") {
                ids.insert("py-no-fallback-os-getenv-default".to_string());
            }
            if lower.contains("os.getenv") || lower.contains("os.environ") {
                ids.insert("py-no-env-outside-settings".to_string());
            }
            if lower.contains("unittest.mock") || lower.contains("from unittest import mock") {
                ids.insert("py-no-test-import-in-runtime".to_string());
            }
            if lower.contains("suppress") {
                ids.insert("py-no-fallback-contextlib-suppress".to_string());
            }
        }
        "ts" | "cts" | "mts" => {
            if contains_any(&lower, &["mock", "stub", "fake"]) {
                ids.insert("ts-no-test-double-identifier".to_string());
            }
            if lower.contains("fallback") {
                ids.insert("ts-no-fallback-identifier".to_string());
            }
            if lower.contains("??=") {
                ids.insert("ts-no-fallback-nullish-assign".to_string());
            }
            if lower.contains("||=") {
                ids.insert("ts-no-fallback-or-assign".to_string());
            }
            if lower.contains("??") {
                ids.insert("ts-no-fallback-nullish".to_string());
            }
            if lower.contains("||") {
                ids.insert("ts-no-fallback-or".to_string());
            }
            if lower.contains("catch") {
                ids.insert("ts-no-empty-catch".to_string());
                ids.insert("ts-no-catch-return-default".to_string());
                ids.insert("ts-no-promise-catch-default".to_string());
            }
            if lower.contains("process.env") {
                ids.insert("ts-no-env-outside-settings".to_string());
            }
            if contains_any(
                &lower,
                &["from 'jest'", "from 'vitest'", "@testing-library"],
            ) {
                ids.insert("ts-no-test-import-in-runtime".to_string());
            }
        }
        _ => {}
    }

    ids.into_iter().collect()
}

fn contains_any(content: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| content.contains(needle))
}

fn run_ast_grep(
    common: &CommonOptions,
    catalog: &RuleCatalog,
    scan_root: &Path,
    targets: &[PathBuf],
    inline_rules: &str,
) -> Result<Vec<Finding>, String> {
    let mut command = Command::new(&common.ast_grep_bin);
    command.arg("scan").arg("--json=stream");
    if !inline_rules.is_empty() {
        command.arg("--inline-rules").arg(inline_rules);
    }
    for target in targets {
        command.arg(target);
    }
    command.current_dir(&common.config_dir);
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    let mut child = command
        .spawn()
        .map_err(|err| format!("could not execute {:?}: {err}", common.ast_grep_bin))?;
    let stdout = child
        .stdout
        .take()
        .ok_or("failed to capture ast-grep stdout".to_string())?;
    let reader = BufReader::new(stdout);
    let mut findings = Vec::new();

    for line in reader.lines() {
        let line = line.map_err(|err| format!("failed to read ast-grep output: {err}"))?;
        if line.trim().is_empty() {
            continue;
        }
        let raw: AstGrepFinding = serde_json::from_str(&line)
            .map_err(|err| format!("failed to parse ast-grep JSON: {err}"))?;
        findings.push(to_finding(raw, catalog, &common.config_dir, scan_root));
    }

    let output = child
        .wait_with_output()
        .map_err(|err| format!("failed to wait for ast-grep: {err}"))?;
    check_exit_ok(&output, "ast-grep scan failed")?;

    Ok(findings)
}

fn check_exit_ok(output: &std::process::Output, fallback_msg: &str) -> Result<(), String> {
    match output.status.code() {
        Some(0) | Some(1) => Ok(()),
        _ => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            Err(if stderr.is_empty() {
                fallback_msg.to_string()
            } else {
                stderr
            })
        }
    }
}

fn read_inline_rules(rule_paths: &[PathBuf]) -> Result<String, String> {
    let mut parts = Vec::new();
    for rule_path in rule_paths {
        parts.push(
            fs::read_to_string(rule_path)
                .map_err(|err| format!("failed to read {}: {err}", rule_path.display()))?,
        );
    }
    Ok(parts.join("\n---\n"))
}

fn to_finding(
    raw: AstGrepFinding,
    catalog: &RuleCatalog,
    config_dir: &Path,
    scan_root: &Path,
) -> Finding {
    let rule_id = raw.rule_id.unwrap_or_else(|| "<unknown-rule>".to_string());
    let mut metadata = catalog.metadata_for(&rule_id);
    if let Some(extra) = raw.metadata {
        metadata.extend(extra);
    }

    let is_abs = Path::new(&raw.file_path).is_absolute();
    let joined = if is_abs {
        PathBuf::from(&raw.file_path)
    } else {
        config_dir.join(&raw.file_path)
    };
    let canonical_file = joined.canonicalize().unwrap_or(joined);

    let display_file = resolve_display_path(&canonical_file, scan_root, &raw.file_path);

    Finding {
        display_file,
        canonical_file,
        line0: raw.range.start.line,
        rule_id,
        message: raw.message.unwrap_or_default(),
        text: raw.text.unwrap_or_default(),
        metadata,
        owner_guess: None,
    }
}

fn rg_comment_pattern(comment_prefix: &str) -> String {
    format!(r"^\s*{comment_prefix}.*\b({SUSPICIOUS_KEYWORDS})\b")
}

fn ripgrep_keyword_comments(targets: &[PathBuf], scan_root: &Path) -> Result<Vec<Finding>, String> {
    if targets.is_empty() {
        return Ok(Vec::new());
    }

    let mut py_targets = Vec::new();
    let mut ts_targets = Vec::new();
    for t in targets {
        match t.extension().and_then(|e| e.to_str()) {
            Some("py") => py_targets.push(t.as_path()),
            Some("ts" | "cts" | "mts") => ts_targets.push(t.as_path()),
            _ => {}
        }
    }

    let py_pattern = rg_comment_pattern("#");
    let ts_pattern = rg_comment_pattern("//");

    let mut findings = Vec::new();
    for (targets, pattern, rule_id) in [
        (&py_targets, py_pattern.as_str(), "py-no-keyword-comment"),
        (&ts_targets, ts_pattern.as_str(), "ts-no-keyword-comment"),
    ] {
        if targets.is_empty() {
            continue;
        }
        let mut cmd = Command::new("rg");
        cmd.args(["--json", "-i", "-e", pattern]);
        for t in targets {
            cmd.arg(t);
        }
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|err| format!("failed to execute ripgrep for keyword comments: {err}"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or("failed to capture rg stdout".to_string())?;
        let reader = BufReader::new(stdout);

        for line in reader.lines() {
            let line = line.map_err(|err| format!("failed to read rg output: {err}"))?;
            if let Some(f) = parse_rg_json_match(&line, rule_id, scan_root) {
                findings.push(f);
            }
        }

        let output = child
            .wait_with_output()
            .map_err(|err| format!("failed to wait for rg: {err}"))?;
        check_exit_ok(&output, "ripgrep keyword comment scan failed")?;
    }
    Ok(findings)
}

fn resolve_display_path(canonical: &Path, scan_root: &Path, raw_fallback: &str) -> String {
    canonical
        .strip_prefix(scan_root)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| raw_fallback.to_string())
}

fn parse_rg_json_match(json_line: &str, rule_id: &str, scan_root: &Path) -> Option<Finding> {
    let v: serde_json::Value = serde_json::from_str(json_line).ok()?;
    if v["type"].as_str()? != "match" {
        return None;
    }
    let data = &v["data"];
    let file_path = data["path"]["text"].as_str()?;
    let line_number = data["line_number"].as_u64()? as usize;
    let text = data["lines"]["text"].as_str().unwrap_or_default().trim();

    let keyword = keyword_comment_regex().find(text)?.as_str().to_lowercase();

    let canonical = Path::new(file_path).canonicalize().ok()?;
    let display = resolve_display_path(&canonical, scan_root, file_path);

    Some(Finding {
        display_file: display,
        canonical_file: canonical,
        line0: line_number.saturating_sub(1),
        rule_id: rule_id.to_string(),
        message: format!(
            "Comment contains suspicious keyword \"{}\" — may indicate AI-introduced placeholder",
            keyword
        ),
        text: text.to_string(),
        metadata: HashMap::from([
            ("policy_group".to_string(), "keyword".to_string()),
            (
                "approval_mode".to_string(),
                APPROVAL_MODE_ADJACENT.to_string(),
            ),
            (
                "semantic_class".to_string(),
                "keyword_placeholder".to_string(),
            ),
        ]),
        owner_guess: None,
    })
}

fn is_approved(
    finding: &Finding,
    cache: &mut HashMap<PathBuf, Vec<String>>,
    registry: &PolicyRegistry,
) -> bool {
    if finding.metadata.get("approval_mode").map(String::as_str) != Some(APPROVAL_MODE_ADJACENT) {
        return false;
    }

    let lines = cache
        .entry(finding.canonical_file.clone())
        .or_insert_with(|| {
            fs::read_to_string(&finding.canonical_file)
                .unwrap_or_default()
                .lines()
                .map(|line| line.to_string())
                .collect()
        });

    let candidates = [
        Some(finding.line0),
        finding.line0.checked_sub(1),
        finding.line0.checked_sub(2),
    ];

    candidates
        .into_iter()
        .flatten()
        .filter_map(|index| lines.get(index))
        .any(|line| {
            let trimmed = line.trim();
            if let Some(m) = approval_regex().find(trimmed) {
                // Extract the approval ID (e.g., "REQ-123") from the match
                let matched = m.as_str();
                if let Some(id_start) = matched.find(|c: char| c.is_ascii_uppercase()) {
                    let id = &matched[id_start..];
                    registry.is_registered(id)
                } else {
                    false
                }
            } else {
                false
            }
        })
}

fn write_err(err: io::Error) -> String {
    format!("failed to write output: {err}")
}

fn extract_hook_file_path(ti: &serde_json::Value) -> String {
    if let Some(fp) = ti["file_path"].as_str().filter(|s| !s.is_empty()) {
        return fp.to_string();
    }
    if let Some(c) = ti["content"].as_str().filter(|s| !s.is_empty()) {
        return c.to_string();
    }
    String::new()
}

fn detect_approval_injection(ti: &serde_json::Value) -> Option<Finding> {
    let new_content = ti["new_string"]
        .as_str()
        .or_else(|| ti["content"].as_str())
        .unwrap_or_default();
    let old_content = ti["old_string"].as_str().unwrap_or_default();

    let new_has = new_content.to_ascii_lowercase().contains("policy-approved");
    let old_has = old_content.to_ascii_lowercase().contains("policy-approved");

    if new_has && !old_has {
        let file_path = extract_hook_file_path(ti);
        let canonical_file = PathBuf::from(&file_path)
            .canonicalize()
            .unwrap_or_else(|_| PathBuf::from(&file_path));
        return Some(Finding {
            display_file: file_path,
            canonical_file,
            line0: 0,
            rule_id: "engine-no-approval-injection".to_string(),
            message: "AI agents cannot add policy-approved comments. These must be added by human developers. Fix the violation by rewriting the code.".to_string(),
            text: new_content.chars().take(200).collect(),
            metadata: HashMap::from([
                ("policy_group".to_string(), "approval_injection".to_string()),
                (
                    "semantic_class".to_string(),
                    "approval_injection".to_string(),
                ),
            ]),
            owner_guess: None,
        });
    }

    None
}

fn format_json(unsuppressed: &[Finding]) -> Result<(), String> {
    let mut groups: Vec<(String, Vec<&Finding>)> = Vec::new();
    for f in unsuppressed {
        let pg = f.metadata.get("policy_group").cloned().unwrap_or_default();
        if let Some(entry) = groups.iter_mut().find(|(g, _)| g == &pg) {
            entry.1.push(f);
        } else {
            groups.push((pg, vec![f]));
        }
    }

    let mut out = io::stdout().lock();
    for (policy_group, findings) in &groups {
        let items: Vec<serde_json::Value> = findings
            .iter()
            .map(|f| {
                json!({
                    "file": f.display_file,
                    "line": f.line0 + 1,
                    "rule_id": f.rule_id,
                    "message": f.message,
                    "code": f.snippet(),
                    "semantic_class": f.metadata.get("semantic_class").unwrap_or(&String::new()),
                    "owner_guess": f.owner_guess,
                })
            })
            .collect();
        let group = json!({
            "policy_group": policy_group,
            "findings": items,
        });
        writeln!(out, "{}", group).map_err(write_err)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{detect_approval_injection, detect_rule_ids};
    use std::path::Path;

    #[test]
    fn python_clean_file_skips_all_rules() {
        let ids = detect_rule_ids(Path::new("sample.py"), "value = config['name']\n");
        assert!(ids.is_empty());
    }

    #[test]
    fn python_fallbacks_select_expected_rules() {
        let ids = detect_rule_ids(
            Path::new("sample.py"),
            "value = payload.get('lang') or 'ja-JP'\nport = os.getenv('PORT', '8080')\n",
        );
        assert!(ids.iter().any(|id| id == "py-no-fallback-bool-or"));
        assert!(ids.iter().any(|id| id == "py-no-fallback-get-default"));
        assert!(ids
            .iter()
            .any(|id| id == "py-no-fallback-os-getenv-default"));
    }

    #[test]
    fn typescript_catch_selects_catch_rules() {
        let ids = detect_rule_ids(Path::new("sample.ts"), "fetch().catch(() => null);\n");
        assert!(ids.iter().any(|id| id == "ts-no-empty-catch"));
        assert!(ids.iter().any(|id| id == "ts-no-catch-return-default"));
        assert!(ids.iter().any(|id| id == "ts-no-promise-catch-default"));
    }

    fn parse_tool_input(json: &str) -> serde_json::Value {
        let v: serde_json::Value = serde_json::from_str(json).unwrap();
        v["tool_input"].clone()
    }

    #[test]
    fn approval_injection_detected_in_edit_new_string() {
        let ti = parse_tool_input(
            r#"{"tool_input":{"file_path":"app.py","old_string":"x = getattr(obj, 'a', None)","new_string":"x = getattr(obj, 'a', None)  # policy-approved: REQ-001 reason"}}"#,
        );
        let finding = detect_approval_injection(&ti);
        assert!(finding.is_some());
        assert_eq!(finding.unwrap().rule_id, "engine-no-approval-injection");
    }

    #[test]
    fn approval_injection_detected_in_write_content() {
        let ti = parse_tool_input(
            r#"{"tool_input":{"file_path":"app.py","content":"x = val or 'default'  # policy-approved: SPEC-001 reason\n"}}"#,
        );
        let finding = detect_approval_injection(&ti);
        assert!(finding.is_some());
    }

    #[test]
    fn approval_injection_not_triggered_when_already_present() {
        let ti = parse_tool_input(
            r#"{"tool_input":{"file_path":"app.py","old_string":"x = val  # policy-approved: REQ-001 ok","new_string":"y = val  # policy-approved: REQ-001 ok"}}"#,
        );
        let finding = detect_approval_injection(&ti);
        assert!(finding.is_none());
    }

    #[test]
    fn approval_injection_not_triggered_on_clean_edit() {
        let ti = parse_tool_input(
            r#"{"tool_input":{"file_path":"app.py","old_string":"x = 1","new_string":"x = 2"}}"#,
        );
        let finding = detect_approval_injection(&ti);
        assert!(finding.is_none());
    }

    #[test]
    fn approval_injection_detected_case_insensitive() {
        let ti = parse_tool_input(
            r#"{"tool_input":{"file_path":"app.py","old_string":"x = 1","new_string":"x = 1  # Policy-Approved: REQ-999 bypass"}}"#,
        );
        let finding = detect_approval_injection(&ti);
        assert!(finding.is_some());
    }
}
