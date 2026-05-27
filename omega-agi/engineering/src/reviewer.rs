//! Automated Code Reviewer
//!
//! Applies rule-based patterns to source files to flag issues such as hardcoded
//! secrets, panic-inducing code, debug prints, TODOs, and more.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

// ---------------------------------------------------------------------------
// Severity
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info     = 0,
    Warning  = 1,
    Error    = 2,
    Critical = 3,
}

impl Severity {
    fn label(&self) -> &'static str {
        match self {
            Severity::Info     => "INFO",
            Severity::Warning  => "WARNING",
            Severity::Error    => "ERROR",
            Severity::Critical => "CRITICAL",
        }
    }
}

// ---------------------------------------------------------------------------
// Finding
// ---------------------------------------------------------------------------

/// A single issue emitted by a rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    /// Rule identifier, e.g. "SECRET_HARDCODED".
    pub rule: String,
    pub severity: Severity,
    pub message: String,
    pub line: usize,
    pub column: Option<usize>,
    pub code_snippet: Option<String>,
}

// ---------------------------------------------------------------------------
// ReviewRule
// ---------------------------------------------------------------------------

/// A single rule for automated review.
#[derive(Clone)]
pub struct ReviewRule {
    pub id: String,
    pub name: String,
    pub severity: Severity,
    /// What the rule detects.
    pub description: String,
    patterns: Vec<Regex>,
    /// File extensions the rule applies to (empty = all).
    pub extensions: Vec<String>,
}

impl ReviewRule {
    /// Build a rule from regex pattern strings.
    ///
    /// All patterns are passed through `concat!` first so that hex escape
    /// sequences like `\x{22}` and `\x{27}` can be used for characters
    /// that are problematic in raw strings.
    pub fn new(
        id: &str,
        name: &str,
        severity: Severity,
        description: &str,
        patterns: Vec<&str>,
        extensions: Vec<&str>,
    ) -> Self {
        let patterns = patterns
            .iter()
            .filter_map(|&p| Regex::new(p).ok())
            .collect();
        Self {
            id: id.to_string(),
            name: name.to_string(),
            severity,
            description: description.to_string(),
            patterns,
            extensions: extensions.into_iter().map(String::from).collect(),
        }
    }

    fn applies_to(&self, path: &Path) -> bool {
        if self.extensions.is_empty() {
            return true;
        }
        if let Some(ext) = path.extension() {
            self.extensions
                .iter()
                .any(|e| e.eq_ignore_ascii_case(&ext.to_string_lossy()))
        } else {
            false
        }
    }

    fn scan_line(&self, line: &str, line_num: usize) -> Option<Finding> {
        for re in &self.patterns {
            if re.is_match(line) {
                let column = re.find(line).map(|m| m.start() + 1);
                let snippet = (line.len() < 150).then(|| line.to_string());
                return Some(Finding {
                    rule: self.id.clone(),
                    severity: self.severity,
                    message: self.description.clone(),
                    line: line_num,
                    column,
                    code_snippet: snippet,
                });
            }
        }
        None
    }
}

// ---------------------------------------------------------------------------
// FileReview
// ---------------------------------------------------------------------------

/// Aggregated findings for one file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileReview {
    pub file_path: PathBuf,
    pub language: Option<String>,
    pub total_lines: usize,
    pub findings: Vec<Finding>,
}

impl FileReview {
    /// True when no issues were found.
    pub fn is_clean(&self) -> bool {
        self.findings.is_empty()
    }
}

/// Summary across a collection of `FileReview` objects.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReviewSummary {
    pub total_files: usize,
    pub files_with_issues: usize,
    pub total_findings: usize,
    pub critical_count: usize,
    pub error_count: usize,
    pub warning_count: usize,
    pub info_count: usize,
    /// Fraction of files with no issues (0.0–1.0).
    pub resolution_rate: f64,
}

impl ReviewSummary {
    pub fn from_reviews(reviews: &[FileReview]) -> Self {
        let total_files = reviews.len();
        let files_with_issues = reviews.iter().filter(|r| !r.is_clean()).count();
        let total_findings = reviews.iter().map(|r| r.findings.len()).sum();

        let mut counts = HashMap::new();
        for review in reviews {
            for f in &review.findings {
                *counts.entry(f.severity).or_insert(0) += 1;
            }
        }

        let resolution_rate = if total_files > 0 {
            (total_files - files_with_issues) as f64 / total_files as f64
        } else {
            1.0
        };

        Self {
            total_files,
            files_with_issues,
            total_findings,
            critical_count: *counts.get(&Severity::Critical).unwrap_or(&0),
            error_count: *counts.get(&Severity::Error).unwrap_or(&0),
            warning_count: *counts.get(&Severity::Warning).unwrap_or(&0),
            info_count: *counts.get(&Severity::Info).unwrap_or(&0),
            resolution_rate,
        }
    }

    /// Compact one-line summary.
    pub fn brief(&self) -> String {
        format!(
            "reviewed {} files · {} issues (C:{} E:{} W:{} I:{}) · {:.1}% clean",
            self.total_files,
            self.total_findings,
            self.critical_count,
            self.error_count,
            self.warning_count,
            self.info_count,
            self.resolution_rate * 100.0,
        )
    }
}

// ---------------------------------------------------------------------------
// Reviewer
// ---------------------------------------------------------------------------

#[derive(thiserror::Error, Debug)]
pub enum ReviewError {
    #[error("IO error: {0}")]
    IoError(String),

    #[error("regex error: {0}")]
    RegexError(String),

    #[error("path not found: {0}")]
    NotFound(String),

    #[error("unsupported file type: {0}")]
    UnsupportedFile(String),
}

/// Main review engine.
pub struct Reviewer {
    rules: Vec<ReviewRule>,
    extensions: Vec<String>,
}

impl Default for Reviewer {
    fn default() -> Self {
        Self::new()
    }
}

impl Reviewer {
    /// Create with the built-in rule set.
    pub fn new() -> Self {
        Self { rules: Self::builtin_rules(), extensions: Vec::new() }
    }

    /// Add a custom rule.
    pub fn add_rule(&mut self, rule: ReviewRule) {
        self.rules.push(rule);
    }

    /// Restrict review to specific file extensions (without leading dot).
    pub fn set_extensions(&mut self, exts: Vec<&str>) {
        self.extensions = exts.into_iter().map(String::from).collect();
    }

    fn language_for(ext: &std::ffi::OsStr) -> Option<String> {
        match ext.to_string_lossy().to_lowercase().as_str() {
            "rs" => Some("Rust".into()),
            "py" => Some("Python".into()),
            "js" => Some("JavaScript".into()),
            "ts" => Some("TypeScript".into()),
            "go" => Some("Go".into()),
            "java" => Some("Java".into()),
            "c" | "h" => Some("C".into()),
            "cpp" | "cc" | "cxx" => Some("C++".into()),
            "cs" => Some("C#".into()),
            "rb" => Some("Ruby".into()),
            "php" => Some("PHP".into()),
            "swift" => Some("Swift".into()),
            "kt" => Some("Kotlin".into()),
            "sh" => Some("Shell".into()),
            _ => None,
        }
    }

    /// Analyze a single file.
    pub fn review_file(&self, path: &Path) -> Result<FileReview, ReviewError> {
        if !path.exists() {
            return Err(ReviewError::NotFound(path.display().to_string()));
        }
        if !path.is_file() {
            return Err(ReviewError::UnsupportedFile(path.display().to_string()));
        }
        let ext = path.extension().unwrap_or_default();
        if !self.extensions.is_empty()
            && !self.extensions.iter().any(|e| e.eq_ignore_ascii_case(&ext.to_string_lossy()))
        {
            return Ok(FileReview {
                file_path: path.to_path_buf(),
                language: Self::language_for(ext),
                total_lines: 0,
                findings: Vec::new(),
            });
        }

        let source =
            fs::read_to_string(path).map_err(|e| ReviewError::IoError(e.to_string()))?;
        let total_lines = source.lines().count();
        let language = Self::language_for(ext);

        let mut findings = Vec::new();
        for rule in &self.rules {
            if !rule.applies_to(path) {
                continue;
            }
            for (line_num, line) in source.lines().enumerate() {
                if let Some(finding) = rule.scan_line(line, line_num + 1) {
                    if rule.id == "TODO_REVIEW"
                        && (line.trim_start().starts_with("//")
                            || line.trim_start().starts_with('#'))
                    {
                        continue;
                    }
                    findings.push(finding);
                }
            }
        }

        findings.sort_by_key(|f| f.line);

        Ok(FileReview { file_path: path.to_path_buf(), language, total_lines, findings })
    }

    /// Recursively analyze all files under a directory.
    pub fn review_dir(&self, dir: &Path) -> Result<Vec<FileReview>, ReviewError> {
        if !dir.exists() || !dir.is_dir() {
            return Err(ReviewError::NotFound(dir.display().to_string()));
        }

        let mut results = Vec::new();
        for entry in WalkDir::new(dir)
            .follow_links(false)
            .same_file_system(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            match self.review_file(path) {
                Ok(review) => {
                    if !review.is_clean() {
                        results.push(review);
                    }
                }
                Err(ReviewError::UnsupportedFile(_)) => {}
                Err(e) => return Err(e),
            }
        }
        Ok(results)
    }

    /// Print a human-readable report.
    pub fn print_report(&self, reviews: &[FileReview]) {
        for review in reviews {
            println!("\nFile: {}", review.file_path.display());
            for finding in &review.findings {
                let sev = finding.severity.label();
                let col = finding.column.map_or(String::new(), |c| format!(":{c}"));
                println!("  {}{col} {sev} [{}] {}", finding.line, finding.rule, finding.message);
                if let Some(ref snippet) = finding.code_snippet {
                    println!("        {snippet}");
                }
            }
        }
        println!("\n{}", ReviewSummary::from_reviews(reviews).brief());
    }

    /// Severity totals across reviews.
    pub fn severity_totals(&self, reviews: &[FileReview]) -> HashMap<Severity, usize> {
        let mut totals: HashMap<Severity, usize> = HashMap::new();
        for review in reviews {
            for f in &review.findings {
                *totals.entry(f.severity).or_insert(0) += 1;
            }
        }
        totals
    }

    // ------------------------------------------------------------------
    // Built-in rules
    //
    // All patterns use `concat!` so that hex escape sequences can represent
    // characters that are difficult or impossible to put in raw strings:
    //   \x{22} = "  \x{27} = '  \x{28} = (  \x{29} = )
    //   \x{2e} = .  \x{21} = !  \x{3a} = :  \x{7b} = {
    //   \x{7d} = }  \x{3b} = ;  \x{3f} = ?  \x{2c} = ,
    //   \x{3d} = =  \x{3a}\x{3a} = ::  \x{2f} = /
    //
    // Rule descriptions avoid ending with a lowercase word followed by ",
    // which the Rust 2021 string prefix rule would misparse.
    // ------------------------------------------------------------------

    fn builtin_rules() -> Vec<ReviewRule> {
        vec![
            // ---- Critical ----

            ReviewRule::new(
                "SECRET_HARDCODED",
                "Hardcoded Secret",
                Severity::Critical,
                "A plaintext API key, password, or token was found in source code.",
                vec![
                    // Key=VALUE patterns — no quotes in the pattern
                    concat!(r"(?i)api[-_]?key\s*[=:]\s*\S+"),
                    concat!(r"(?i)apikey\s*[=:]\s*\S+"),
                    concat!(r"(?i)password\s*[=:]\s*\S+"),
                    concat!(r"(?i)secret[-_]?key\s*[=:]\s*\S+"),
                    concat!(r"(?i)bearer\s+[A-Za-z0-9_.-]+"),
                    concat!(r"(?i)token\s*[=:]\s*[A-Za-z0-9_.-]{10,}"),
                    concat!(r"(?i)aws[-_]?(access[-_]?key|secret)[-_]?id\s*[=:]\s*[A-Z0-9]{16,}"),
                    concat!(r"sk-[A-Za-z0-9]{20,}"),
                    // BEGIN PRIVATE KEY — no quotes needed
                    concat!(r"-----BEGIN PRIVATE KEY-----"),
                    // Quoted string value: use character class for quotes
                    concat!(r#"(?i)(api[-_]?key|password|secret)\s*[=:]\s*[\x{22}\x{27}][^"\x{27}]{8,}[\x{22}\x{27}]"#),
                ],
                vec!["rs", "py", "js", "ts", "go", "java"],
            ),

            ReviewRule::new(
                "PANIC_UNWRAP",
                "Panic or Unwrap",
                Severity::Critical,
                "A .unwrap() or .expect() call was found; prefer Result or Option handling.",
                vec![
                    // Match ".unwrap" and ".expect" — \x{2e} for dot, \x{28} for (
                    concat!(r"\x{2e}unwrap\x{28}"),
                    concat!(r"\x{2e}expect\x{28}"),
                ],
                vec!["rs"],
            ),

            // ---- Error ----

            ReviewRule::new(
                "TODO_REVIEW",
                "TODO Missing Owner",
                Severity::Error,
                "TODO/FIXME comment does not tag an owner; add @user to assign it.",
                vec![
                    concat!(r"(?i)TODO(?!.*@[a-zA-Z0-9_])"),
                    concat!(r"(?i)FIXME(?!.*@[a-zA-Z0-9_])"),
                ],
                vec!["rs", "py", "js", "ts", "go", "java"],
            ),

            ReviewRule::new(
                "DEBUG_PRINT",
                "Debug Print Detected",
                Severity::Error,
                "A debug print or console.log call was found in production code.",
                vec![
                    // println!, eprintln!, print! — \x{21} for !
                    concat!(r"println!\x{28}"),
                    concat!(r"eprintln!\x{28}"),
                    concat!(r"print!\x{28}"),
                    // console.log — \x{2e} for dot
                    concat!(r"console\x{2e}log\x{28}"),
                    concat!(r"printStackTrace\x{28}"),
                ],
                vec!["rs", "py", "js", "ts", "go", "java"],
            ),

            ReviewRule::new(
                "MUTEX_UNLOCK",
                "Missing Mutex Unlock",
                Severity::Error,
                "A Mutex guard is dropped immediately; prefer explicit drop.",
                vec![
                    // let mut X = mutex.lock()?; — hex-encode special chars
                    concat!(r"let\s+mut\s+\w+\s*=\s*\w+\x{2e}lock\x{28}\x{29}\x{3f}\x{3b}"),
                ],
                vec!["rs"],
            ),

            ReviewRule::new(
                "CLONE_IN_LOOP",
                "Clone Inside Loop",
                Severity::Error,
                "A .clone() call appears inside a loop; move clones outside when safe.",
                vec![
                    concat!(r"for\s*\{[^}]*\x{2e}clone\x{28}"),
                    concat!(r"while\s*\{[^}]*\x{2e}clone\x{28}"),
                ],
                vec!["rs"],
            ),

            ReviewRule::new(
                "UNWRAP_IN_RESULT",
                "Unwrap in Result Function",
                Severity::Error,
                "unwrap() called in a function returning Result; use the ? operator.",
                vec![
                    concat!(r"fn\s+\w+\s*\x{28}[^)]*\x{29}\s*->\s*Result<[^>]>\s*\{[^}]*\x{2e}unwrap\x{28}"),
                ],
                vec!["rs"],
            ),

            ReviewRule::new(
                "IGNORED_ERROR",
                "Ignored Error Value",
                Severity::Error,
                "A Result/Option value is silently discarded; use let () = or if let Some.",
                vec![
                    concat!(r"^\s*_\s*=\s*[^;]+;\s*$"),
                ],
                vec!["rs", "py"],
            ),

            ReviewRule::new(
                "HARDCODE_PATH",
                "Hardcoded Absolute Path",
                Severity::Error,
                "An absolute filesystem path is hardcoded; use env vars or config instead.",
                vec![
                    // Match "path" or 'path' — \x{22}=double-quote, \x{27}=single-quote
                    concat!(r"[\x{22}\x{27}][/][\w.-]+[/][\w.-]+[/][\w./-]+[\x{22}\x{27}]"),
                ],
                vec!["rs", "py", "js", "sh"],
            ),

            ReviewRule::new(
                "SQL_INJECTION_RISK",
                "SQL Injection Risk",
                Severity::Error,
                "String concatenation used to build a SQL query; use parameterized queries.",
                vec![
                    // format! with SELECT/INSERT/exec — \x{21}=!
                    concat!(r"format!\x{28}.*SELECT"),
                    concat!(r"format!\x{28}.*INSERT"),
                    concat!(r"format!\x{28}.*exec"),
                    concat!(r"\x{2e}execute\x{28}\s*format!\x{28}"),
                ],
                vec!["rs", "py", "js", "java", "go"],
            ),

            // ---- Warning ----

            ReviewRule::new(
                "DEAD_CODE",
                "Dead Code Detected",
                Severity::Warning,
                "A function or block is never used; consider removing it.",
                vec![
                    concat!(r"fn\s+\w+\s*\([^)]*\)\s*\{[^}]*never\s+returns"),
                    concat!(r"^\s*fn\s+_\w+\s*\x{28}"),
                ],
                vec!["rs"],
            ),

            ReviewRule::new(
                "LARGE_ALLOC",
                "Large Allocation in Loop",
                Severity::Warning,
                "A large allocation occurs inside a loop; move it outside when safe.",
                vec![
                    // vec![...] — \x{21}=!  \x{7b}={  \x{7d}=}
                    concat!(r"for\s*\{[^}]*vec!\x{21}\s*\{[^]]\{50,\}\}[^}]*\x{7d}"),
                    concat!(r"for\s*\{[^}]*String\x{3a}\x{3a}from"),
                ],
                vec!["rs", "py"],
            ),

            ReviewRule::new(
                "TODO_OWNED",
                "Unowned TODO",
                Severity::Warning,
                "A TODO or FIXME comment exists but has no owner tag.",
                vec![
                    concat!(r"(?i)(TODO|FIXME|HACK|XXX)"),
                ],
                vec!["rs", "py", "js", "ts"],
            ),

            ReviewRule::new(
                "SLOW_ITER",
                "Slow Iterator Pattern",
                Severity::Warning,
                "An iterator collects results then loops; iterate directly instead.",
                vec![
                    // \x{3a}\x{3a} = ::
                    concat!(r"\.collect\x{3a}\x{3a}<Vec<[^>]>>\x{28}\x{29}.*\.iter\x{28}"),
                ],
                vec!["rs"],
            ),

            ReviewRule::new(
                "UNSAFE_BLOCK",
                "Unsafe Code Block",
                Severity::Warning,
                "An unsafe block was found; confirm safety invariants are documented.",
                vec![
                    concat!(r"unsafe\s*\{"),
                ],
                vec!["rs"],
            ),

            // ---- Info ----

            ReviewRule::new(
                "EMPTY_CATCH",
                "Empty Catch Block",
                Severity::Info,
                "An empty catch/except block silently swallows errors.",
                vec![
                    concat!(r"catch\s*\([^)]*\)\s*\{\s*\}"),
                    concat!(r"except[^:]*:\s*pass"),
                ],
                vec!["java", "py", "js"],
            ),

            ReviewRule::new(
                "COMPLEX_EXPR",
                "Overly Complex Expression",
                Severity::Info,
                "A single expression exceeds 120 characters; consider extracting a helper.",
                vec![
                    concat!(r"^.{121,}$"),
                ],
                vec!["rs", "py", "js", "ts", "go"],
            ),
        ]
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn tmp(ext: &str, src: &str) -> PathBuf {
        let mut f = NamedTempFile::with_suffix(&format!(".{}", ext)).unwrap();
        f.write_all(src.as_bytes()).unwrap();
        f.into_temp_path()
    }

    #[test]
    fn test_detect_hardcoded_secret() {
        let r = Reviewer::new();
        let t = tmp("rs", r#"const API_KEY: &str = "sk-abc123def456ghi789jkl";"#);
        let rev = r.review_file(&t).unwrap();
        assert!(rev.findings.iter().any(|f| f.rule == "SECRET_HARDCODED"));
    }

    #[test]
    fn test_detect_unwrap() {
        let r = Reviewer::new();
        let t = tmp("rs", "let x = some_result.unwrap();");
        let rev = r.review_file(&t).unwrap();
        assert!(rev.findings.iter().any(|f| f.rule == "PANIC_UNWRAP"));
    }

    #[test]
    fn test_detect_debug_print() {
        let r = Reviewer::new();
        let t = tmp("rs", r#"println!("{:?}", value);"#);
        let rev = r.review_file(&t).unwrap();
        assert!(rev.findings.iter().any(|f| f.rule == "DEBUG_PRINT"));
    }

    #[test]
    fn test_clean_file_empty() {
        let r = Reviewer::new();
        let t = tmp("rs", "pub fn add(a: i32, b: i32) -> i32 { a + b }");
        let rev = r.review_file(&t).unwrap();
        assert!(rev.is_clean());
    }

    #[test]
    fn test_summary_counts() {
        let rev = FileReview {
            file_path: PathBuf::from("test.rs"),
            language: Some("Rust".into()),
            total_lines: 10,
            findings: vec![
                Finding {
                    rule: "PANIC_UNWRAP".into(),
                    severity: Severity::Critical,
                    message: "".into(),
                    line: 5,
                    column: None,
                    code_snippet: None,
                },
                Finding {
                    rule: "DEBUG_PRINT".into(),
                    severity: Severity::Error,
                    message: "".into(),
                    line: 10,
                    column: None,
                    code_snippet: None,
                },
            ],
        };
        let s = ReviewSummary::from_reviews(&[rev]);
        assert_eq!(s.total_files, 1);
        assert_eq!(s.critical_count, 1);
        assert_eq!(s.error_count, 1);
    }

    #[test]
    fn test_severity_order() {
        assert!(Severity::Critical > Severity::Error);
        assert!(Severity::Error > Severity::Warning);
        assert!(Severity::Warning > Severity::Info);
    }

    #[test]
    fn test_file_not_found() {
        let r = Reviewer::new();
        assert!(r.review_file(Path::new("nonexistent.rs")).is_err());
    }

    #[test]
    fn test_language_detection() {
        let r = Reviewer::new();
        let t = tmp("py", "x = 1");
        let rev = r.review_file(&t).unwrap();
        assert_eq!(rev.language, Some("Python".into()));
    }
}