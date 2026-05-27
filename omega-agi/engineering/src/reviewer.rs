//! # Code Reviewer — Rule-based Static Analysis
//!
//! Performs rule-based static analysis on Rust/Python source files.
//! Built-in rules detect hardcoded secrets, panic/unwrap calls,
//! expensive clone patterns, TODOs, and more.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

// ============================================================================
// Error types
// ============================================================================

#[derive(Error, Debug)]
pub enum ReviewError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Path is not a file or directory: {0}")]
    NotFound(String),

    #[error("Unsupported file type: {0}")]
    UnsupportedFile(String),
}

// ============================================================================
// Severity and rule types
// ============================================================================

/// Finding severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info     = 0,
    Warning  = 1,
    Error   = 2,
    Critical= 3,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Info      => write!(f, "info"),
            Severity::Warning  => write!(f, "warning"),
            Severity::Error    => write!(f, "error"),
            Severity::Critical => write!(f, "critical"),
        }
    }
}

/// A single finding inside one file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub rule: String,
    pub severity: Severity,
    pub message: String,
    pub line: usize,
    pub column: Option<usize>,
    pub code_snippet: Option<String>,
}

/// All findings for one file.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FileReview {
    pub file_path: String,
    pub language: Option<String>,
    pub total_lines: usize,
    pub findings: Vec<Finding>,
}

impl FileReview {
    pub fn severity_count(&self) -> HashMap<Severity, usize> {
        let mut counts = HashMap::new();
        for f in &self.findings {
            *counts.entry(f.severity).or_insert(0) += 1;
        }
        counts
    }

    pub fn has_critical(&self) -> bool {
        self.findings.iter().any(|f| f.severity == Severity::Critical)
    }

    pub fn is_clean(&self) -> bool {
        self.findings.is_empty()
    }
}

/// A single rule for automated review.
#[derive(Clone)]
pub struct ReviewRule {
    pub id: &'static str,
    pub name: &'static str,
    pub severity: Severity,
    pub description: &'static str,
    /// Patterns are OR-ed together; any match triggers a finding.
    pub patterns: Vec<Regex>,
    /// File extensions this rule applies to. Empty = all languages.
    pub extensions: Vec<&'static str>,
}

impl ReviewRule {
    /// Build a rule from a list of pattern strings.
    pub fn new(
        id: &'static str,
        name: &'static str,
        severity: Severity,
        description: &'static str,
        patterns: Vec<&'static str>,
        extensions: Vec<&'static str>,
    ) -> Self {
        let patterns = patterns
            .into_iter()
            .map(|p| Regex::new(p).expect("review rule regex must be valid"))
            .collect();
        Self { id, name, severity, description, patterns, extensions }
    }

    /// Returns true if this rule applies to a given file extension.
    pub fn applies_to(&self, ext: &str) -> bool {
        self.extensions.is_empty() || self.extensions.contains(&ext)
    }
}

// ============================================================================
// Built-in rules
// ============================================================================

/// All built-in review rules.
pub fn built_in_rules() -> Vec<ReviewRule> {
    vec![
        // ---- Critical ----
        ReviewRule::new(
            "SECRET_HARDCODED",
            "Hardcoded Secret Detected",
            Severity::Critical,
            "A plaintext API key, password, or token was found in source code.",
            vec![
                r"(?i)(api[_-]?key|apikey|api_secret|secret[_-]?key)\s*[=:]\s*['\"][A-Za-z0-9_\-]{8,}['\"]",
                r"(?i)password\s*[=:]\s*['\"][^'\"]{4,}['\"]",
                r"(?i)bearer\s+[A-Za-z0-9_\-\.]+",
                r"(?i)token\s*[=:]\s*['\"][A-Za-z0-9_\-\.]{16,}['\"]",
                r"(?i)aws[_-]?(access[_-]?key|secret)[_-]?id\s*[=:]\s*['\"][A-Z0-9]{16,}['\"]",
                r"(?i)sk-[A-Za-z0-9]{20,}",
                r"-----BEGIN (RSA |EC |DSA )?PRIVATE KEY-----",
            ],
            vec![],
        ),
        ReviewRule::new(
            "PANIC_UNWRAP",
            "Panic / Unwrap in Production Code",
            Severity::Critical,
            ".unwrap() or .expect() was found; prefer Result/Option handling.",
            vec![
                r"\.(expect|unwrap)\s*\(",
                r"\.unwrap\(\)\s*;?\s*(//|$)",
                r"\.expect\s*\(\s*[\'\"]",
            ],
            vec!["rs", "py"],
        ),
        // ---- Error ----
        ReviewRule::new(
            "TODO_REVIEW",
            "TODO/FIXME Without Owner",
            Severity::Error,
            "A TODO or FIXME comment lacks an owner tag (@user).",
            vec![
                r"//\s*TODO\s*(?!.*@[a-zA-Z0-9_])",
                r"//\s*FIXME\s*(?!.*@[a-zA-Z0-9_])",
                r"#\s*TODO\s*(?!.*@[a-zA-Z0-9_])",
                r"#\s*FIXME\s*(?!.*@[a-zA-Z0-9_])",
            ],
            vec!["rs", "py", "ts", "js"],
        ),
        ReviewRule::new(
            "DEBUG_PRINT",
            "Debug Print Remaining in Code",
            Severity::Error,
            "println!, print!, console.log, or debug print found.",
            vec![
                r"println!\s*\(",
                r"eprintln!\s*\(",
                r"print!\s*\(",
                r"console\.log\s*\(",
                r"print\s*\(\s*['\"]",
                r"puts\s*\(",
                r"Debug\.print",
                r"\.dbg\(\)",
            ],
            vec!["rs", "py", "ts", "js"],
        ),
        ReviewRule::new(
            "MUTEX_UNLOCK",
            "Missing Mutex Unlock / Drop",
            Severity::Error,
            "A mutex guard may not be released promptly; consider explicit drop.",
            vec![
                r"\.lock\(\)\s*;?\s*(//\s*$|\n\s*})",
            ],
            vec!["rs"],
        ),
        // ---- Warning ----
        ReviewRule::new(
            "CLONE_IN_LOOP",
            "Expensive Clone Inside Loop",
            Severity::Warning,
            "Calling .clone() inside a loop may cause unnecessary allocation.",
            vec![
                r"for\s+.*in\s+.*\{\s*\n\s*.*\.clone\(\)",
                r"while\s+.*\{\s*\n\s*.*\.clone\(\)",
                r"\.iter\(\)\s*\.cloned\(\)",
                r"\.into_iter\(\)\s*\.cloned\(\)",
            ],
            vec!["rs"],
        ),
        ReviewRule::new(
            "UNWRAP_IN_RESULT",
            "Unwrap on Result in Function Returning Result",
            Severity::Warning,
            "Using ? operator instead of unwrap maintains error propagation.",
            vec![
                r"fn\s+\w+\s*\([^)]*\)\s*->\s*Result<.*>\s*\{[^}]*\.unwrap\(\)",
                r"fn\s+\w+\s*\([^)]*\)\s*->\s*Option<.*>\s*\{[^}]*\.unwrap\(\)",
            ],
            vec!["rs"],
        ),
        ReviewRule::new(
            "IGNORED_ERROR",
            "Ignoring Error with underscore",
            Severity::Warning,
            "Assigning Result to _ discards the error; prefer error propagation.",
            vec![
                r"let\s+_\s*=\s*\w+\s*\(",
                r"let\s+_\s*:\s*\w+\s*=\s*\w+\s*\(",
            ],
            vec!["rs"],
        ),
        ReviewRule::new(
            "DEAD_CODE",
            "Dead / Unused Code",
            Severity::Warning,
            "An #[allow(dead_code)] annotation or an unreachable branch was found.",
            vec![
                r"#\[allow\s*\(\s*dead_code\s*\)\]",
                r"unreachable!\s*\(",
                r"unimplemented!\s*\(",
                r"todo!\s*\(",
            ],
            vec!["rs"],
        ),
        ReviewRule::new(
            "HARDCODE_PATH",
            "Hardcoded Absolute Path",
            Severity::Warning,
            "An absolute filesystem path is hardcoded; use env vars or config.",
            vec![
                r#"['"/]/(home|usr|var|etc|opt|tmp)/"#,
                r#"(C:\\|D:\\|E:\\)"#,
                r#"/workspace/"#,
                r#"/root/"#,
            ],
            vec![],
        ),
        ReviewRule::new(
            "SQL_INJECTION_RISK",
            "SQL / Command Injection Risk",
            Severity::Warning,
            "String formatting used for SQL or shell commands; use parameterized queries.",
            vec![
                r"format!\s*\(\s*[\'\"].*SELECT.*\{",
                r"format!\s*\(\s*[\'\"].*INSERT.*\{",
                r"format!\s*\(\s*[\'\"].*exec\s*\(",
                r"\.execute\s*\(\s*format\s*\(",
            ],
            vec!["rs", "py"],
        ),
        ReviewRule::new(
            "LARGE_ALLOC",
            "Large Allocation in Loop",
            Severity::Warning,
            "Allocating a large Vec or String inside a loop; consider pre-allocation.",
            vec![
                r"for\s+.*\{\s*\n\s*let\s+\w+\s*:\s*Vec<",
                r"for\s+.*\{\s*\n\s*let\s+\w+\s*:\s*String\s*=\s*String::new\(\)",
                r"for\s+.*in.*\.collect::<Vec<",
            ],
            vec!["rs"],
        ),
        // ---- Info ----
        ReviewRule::new(
            "TODO_OWNED",
            "TODO With Owner Tag",
            Severity::Info,
            "A TODO/FIXME comment that already has an owner; ok to keep.",
            vec![
                r"//\s*TODO\s+.*@[a-zA-Z0-9_]+",
                r"//\s*FIXME\s+.*@[a-zA-Z0-9_]+",
                r"#\s*TODO\s+.*@[a-zA-Z0-9_]+",
            ],
            vec!["rs", "py", "ts", "js"],
        ),
        ReviewRule::new(
            "SLOW_ITER",
            "Inefficient Iteration Pattern",
            Severity::Info,
            "Iterating with .iter().cloned().filter() can be simplified.",
            vec![
                r"\.iter\(\)\s*\.cloned\(\)\s*\.filter",
                r"\.iter\(\)\s*\.cloned\(\)\s*\.map",
            ],
            vec!["rs"],
        ),
    ]
}

// ============================================================================
// Reviewer
// ============================================================================

/// Rule-based code reviewer.
pub struct Reviewer {
    rules: Vec<ReviewRule>,
    /// File extensions to include (e.g. ["rs", "py"]). Empty = all.
    extensions: Vec<String>,
}

impl Default for Reviewer {
    fn default() -> Self {
        Self::new()
    }
}

impl Reviewer {
    /// Create a reviewer with all built-in rules.
    pub fn new() -> Self {
        Self::with_rules(built_in_rules())
    }

    /// Create a reviewer with a custom set of rules.
    pub fn with_rules(rules: Vec<ReviewRule>) -> Self {
        Self { rules, extensions: Vec::new() }
    }

    /// Restrict to specific file extensions (e.g. ["rs", "py"]).
    pub fn with_extensions(mut self, exts: Vec<&str>) -> Self {
        self.extensions = exts.into_iter().map(|s| s.to_string()).collect();
        self
    }

    /// Check if a file extension is allowed.
    fn is_allowed_ext(&self, ext: &str) -> bool {
        self.extensions.is_empty() || self.extensions.iter().any(|e| e == ext)
    }

    /// Infer language from file extension.
    fn language_for_ext(ext: &str) -> &'static str {
        match ext {
            "rs" => "Rust",
            "py" => "Python",
            "ts" | "tsx" => "TypeScript",
            "js" | "jsx" => "JavaScript",
            "go" => "Go",
            "java" => "Java",
            "cpp" | "cc" | "cxx" | "c" | "h" | "hpp" => "C/C++",
            "cs" => "C#",
            "rb" => "Ruby",
            "php" => "PHP",
            "swift" => "Swift",
            "kt" => "Kotlin",
            "scala" => "Scala",
            _ => "Unknown",
        }
    }

    /// Determine the file extension from a path (without the leading dot).
    fn ext_of(path: &Path) -> Option<String> {
        path.extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_lowercase())
    }

    /// Run all rules against the contents of one file.
    pub fn review_file(&self, path: &Path) -> Result<FileReview, ReviewError> {
        if !path.is_file() {
            return Err(ReviewError::NotFound(path.display().to_string()));
        }

        let ext = Self::ext_of(path).unwrap_or_default();
        if !self.is_allowed_ext(&ext) {
            return Ok(FileReview {
                file_path: path.display().to_string(),
                language: None,
                total_lines: 0,
                findings: Vec::new(),
            });
        }

        let source = fs::read_to_string(path)?;
        let total_lines = source.lines().count();
        let language = Some(Self::language_for_ext(&ext).to_string());

        let mut findings = Vec::new();

        for rule in &self.rules {
            if !rule.applies_to(&ext) {
                continue;
            }
            for pat in &rule.patterns {
                for (line_num, line) in source.lines().enumerate() {
                    if pat.is_match(line) {
                        // Avoid flagging commented-out TODOs with the Error rule
                        let is_commented = line.trim_start().starts_with("//")
                            || line.trim_start().starts_with('#');
                        if rule.id == "TODO_REVIEW" && is_commented {
                            continue;
                        }

                        let code_snippet = Some(line.chars().take(120).collect());

                        findings.push(Finding {
                            rule: rule.id.to_string(),
                            severity: rule.severity,
                            message: format!("[{}] {}", rule.name, rule.description),
                            line: line_num + 1,
                            column: pat.find(line).map(|m| m.start() + 1),
                            code_snippet,
                        });
                    }
                }
            }
        }

        // Sort by severity descending, then by line number
        findings.sort_by(|a, b| {
            b.severity.cmp(&a.severity)
                .then_with(|| a.line.cmp(&b.line))
        });

        Ok(FileReview { file_path: path.display().to_string(), language, total_lines, findings })
    }

    /// Recursively walk a directory and review every allowed source file.
    /// Returns only files that had at least one finding.
    pub fn review_dir(&self, dir: &Path) -> Result<Vec<FileReview>, ReviewError> {
        if !dir.is_dir() {
            return Err(ReviewError::NotFound(dir.display().to_string()));
        }

        let mut results = Vec::new();
        let walker = ignore::WalkBuilder::new(dir)
            .follow_links(true)
            .standard_filters(true)
            .build();

        for entry in walker.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if let Some(ref ext) = Self::ext_of(path) {
                if !self.is_allowed_ext(ext) {
                    continue;
                }
            } else {
                continue;
            }

            match self.review_file(path) {
                Ok(review) => {
                    if !review.findings.is_empty() {
                        results.push(review);
                    }
                }
                Err(ReviewError::NotFound(_)) => {}
                Err(e) => return Err(e),
            }
        }

        Ok(results)
    }

    /// Review multiple specific paths (files or directories).
    pub fn review_paths(&self, paths: &[PathBuf]) -> Result<Vec<FileReview>, ReviewError> {
        let mut results = Vec::new();
        for path in paths {
            if path.is_dir() {
                results.extend(self.review_dir(path)?);
            } else if path.is_file() {
                let review = self.review_file(path)?;
                if !review.is_clean() {
                    results.push(review);
                }
            }
        }
        Ok(results)
    }

    /// Build a human-readable text report for a set of file reviews.
    pub fn format_report(&self, reviews: &[FileReview]) -> String {
        if reviews.is_empty() {
            return "✅ No issues found.".to_string();
        }

        let mut out = String::new();
        let mut total_findings = 0usize;
        let mut severity_totals: HashMap<Severity, usize> = HashMap::new();

        for review in reviews {
            total_findings += review.findings.len();
            for f in &review.findings {
                *severity_totals.entry(f.severity).or_insert(0) += 1;
            }
        }

        out.push_str(&format!("📋 Code Review Report — {} file(s) with issues\n",
            reviews.len()));
        out.push_str(&format!("   Total findings: {total_findings}\n"));
        for sev in [Severity::Critical, Severity::Error, Severity::Warning, Severity::Info] {
            if let Some(&cnt) = severity_totals.get(&sev) {
                out.push_str(&format!("   {sev}: {cnt}\n"));
            }
        }
        out.push('\n');

        for review in reviews {
            out.push_str(&format!("▶ {} ({}, {} lines)\n",
                review.file_path,
                review.language.as_deref().unwrap_or("?"),
                review.total_lines));
            for f in &review.findings {
                out.push_str(&format!(
                    "  [{:>8}] L{}: {}\n    {}\n",
                    f.severity, f.line,
                    f.rule,
                    f.code_snippet.as_deref().unwrap_or("")
                ));
            }
            out.push('\n');
        }

        out
    }
}

/// Aggregated summary across multiple file reviews.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewSummary {
    pub total_files: usize,
    pub files_with_issues: usize,
    pub total_findings: usize,
    pub critical_count: usize,
    pub error_count: usize,
    pub warning_count: usize,
    pub info_count: usize,
    pub resolution_rate: f64,
}

impl ReviewSummary {
    pub fn from_reviews(reviews: &[FileReview]) -> Self {
        let total_files = reviews.len();
        let files_with_issues = reviews.iter().filter(|r| !r.is_clean()).count();
        let mut critical_count = 0usize;
        let mut error_count = 0usize;
        let mut warning_count = 0usize;
        let mut info_count = 0usize;
        let mut total_findings = 0usize;

        for review in reviews {
            for f in &review.findings {
                total_findings += 1;
                match f.severity {
                    Severity::Critical => critical_count += 1,
                    Severity::Error    => error_count += 1,
                    Severity::Warning  => warning_count += 1,
                    Severity::Info     => info_count += 1,
                }
            }
        }

        let resolution_rate = if files_with_issues > 0 {
            (total_files - files_with_issues) as f64 / total_files as f64
        } else {
            1.0
        };

        Self {
            total_files,
            files_with_issues,
            total_findings,
            critical_count,
            error_count,
            warning_count,
            info_count,
            resolution_rate,
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn temp_file(ext: &str, content: &str) -> PathBuf {
        let mut f = NamedTempFile::with_suffix(&format!(".{}", ext)).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f.into_temp_path()
    }

    #[test]
    fn test_hardcoded_secret_detection() {
        let reviewer = Reviewer::new();
        let tmp = temp_file("rs", r#"const API_KEY: &str = "sk-abc123def456xyz";"#);
        let review = reviewer.review_file(&tmp).unwrap();
        assert!(review.findings.iter().any(|f| f.rule == "SECRET_HARDCODED"));
    }

    #[test]
    fn test_unwrap_detection() {
        let reviewer = Reviewer::new();
        let tmp = temp_file("rs", r#"let x = some_result.unwrap();"#);
        let review = reviewer.review_file(&tmp).unwrap();
        assert!(review.findings.iter().any(|f| f.rule == "PANIC_UNWRAP"));
    }

    #[test]
    fn test_debug_print_detection() {
        let reviewer = Reviewer::new();
        let tmp = temp_file("rs", r#"println!("{:?}", value);"#);
        let review = reviewer.review_file(&tmp).unwrap();
        assert!(review.findings.iter().any(|f| f.rule == "DEBUG_PRINT"));
    }

    #[test]
    fn test_clean_file_returns_empty() {
        let reviewer = Reviewer::new();
        let tmp = temp_file("rs", r#"pub fn add(a: i32, b: i32) -> i32 { a + b }"#);
        let review = reviewer.review_file(&tmp).unwrap();
        assert!(review.is_clean());
    }

    #[test]
    #[ignore] // requires directory with files
    fn test_review_dir() {
        let reviewer = Reviewer::new();
        let dir = PathBuf::from("/tmp");
        let results = reviewer.review_dir(&dir);
        assert!(results.is_ok());
    }

    #[test]
    fn test_review_summary() {
        let review = FileReview {
            file_path: "test.rs".into(),
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
        let summary = ReviewSummary::from_reviews(&[review]);
        assert_eq!(summary.total_files, 1);
        assert_eq!(summary.critical_count, 1);
        assert_eq!(summary.error_count, 1);
    }

    #[test]
    fn test_severity_order() {
        assert!(Severity::Critical > Severity::Error);
        assert!(Severity::Error > Severity::Warning);
        assert!(Severity::Warning > Severity::Info);
    }
}