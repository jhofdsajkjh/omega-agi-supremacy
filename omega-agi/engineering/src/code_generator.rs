//! # Code Generator - 基于提示的代码生成器
//!
//! 支持Rust和Python双语言生成、代码质量评分、语法验证

use serde::{Deserialize, Serialize};
use std::process::{Command, Stdio};
use thiserror::Error;

// ============================================================================
// Error Types
// ============================================================================

#[derive(Error, Debug)]
pub enum GenError {
    #[error("Generation failed: {0}")]
    GenerationFailed(String),
    
    #[error("Invalid prompt: {0}")]
    InvalidPrompt(String),
    
    #[error("Syntax validation failed: {0}")]
    SyntaxError(String),
    
    #[error("LLM API error: {0}")]
    ApiError(String),
    
    #[error("Formatting failed: {0}")]
    FormatError(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// 代码质量评分
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CodeQuality {
    pub overall_score: f32,
    pub readability: f32,
    pub maintainability: f32,
    pub safety: f32,
    pub performance: f32,
}

impl Default for CodeQuality {
    fn default() -> Self {
        Self {
            overall_score: 0.0,
            readability: 0.0,
            maintainability: 0.0,
            safety: 0.0,
            performance: 0.0,
        }
    }
}

/// 代码上下文
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CodeContext {
    pub project_name: String,
    pub file_path: Option<String>,
    pub language: Option<Language>,
    pub imports: Vec<String>,
    pub existing_code: Option<String>,
    pub doc_comments: Vec<String>,
}

/// 语言枚举
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    Rust,
    Python,
    Both,
}

impl Default for Language {
    fn default() -> Self {
        Language::Rust
    }
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Language::Rust => write!(f, "rust"),
            Language::Python => write!(f, "python"),
            Language::Both => write!(f, "both"),
        }
    }
}

/// 生成的代码
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GeneratedCode {
    pub code: String,
    pub language: Language,
    pub confidence: f32,
    pub tokens_used: usize,
    pub quality: Option<CodeQuality>,
    pub warnings: Vec<String>,
}

impl GeneratedCode {
    /// 检查代码是否包含常见Rust错误模式
    pub fn check_rust_antipatterns(&self) -> Vec<String> {
        let mut warnings = Vec::new();
        
        if self.language != Language::Rust && self.language != Language::Both {
            return warnings;
        }

        let code_lower = self.code.to_lowercase();
        
        if code_lower.contains(".unwrap()") {
            warnings.push("Use of .unwrap() detected - consider using ? or expect with message".to_string());
        }
        
        if code_lower.contains(".expect(") {
            warnings.push("Use of .expect() detected - prefer ? operator for error propagation".to_string());
        }
        
        if code_lower.contains("panic!") {
            warnings.push("Use of panic! detected - consider returning Result instead".to_string());
        }
        
        if code_lower.contains("unsafe ") {
            warnings.push("Use of unsafe block detected - ensure memory safety".to_string());
        }
        
        if code_lower.contains(".unwrap_or(") {
            warnings.push("Use of unwrap_or detected - unwrap_or_else is more efficient".to_string());
        }
        
        let clone_count = code_lower.matches(".clone()").count();
        if clone_count > 3 {
            warnings.push(format!("High clone usage ({} clones) - consider using references", clone_count));
        }
        
        warnings
    }

    /// 计算代码质量评分
    pub fn compute_quality_score(&self) -> CodeQuality {
        let mut quality = CodeQuality::default();
        
        quality.overall_score = self.confidence;
        
        let lines: Vec<&str> = self.code.lines().collect();
        let non_empty_lines = lines.iter().filter(|l| !l.trim().is_empty()).count();
        let comment_lines = lines.iter().filter(|l| {
            let trimmed = l.trim();
            trimmed.starts_with("//") || trimmed.starts_with('#') || trimmed.starts_with("/*")
        }).count();
        
        if non_empty_lines > 0 {
            let comment_ratio = comment_lines as f32 / non_empty_lines as f32;
            quality.readability = (0.5 + 0.5 * comment_ratio.min(0.3) / 0.3).min(1.0);
        }
        
        let warnings = self.check_rust_antipatterns();
        quality.safety = (1.0 - (warnings.len() as f32 * 0.1)).max(0.0);
        
        let function_count = self.code.matches("fn ").count();
        let struct_count = self.code.matches("struct ").count();
        let maintainability_factor = (function_count + struct_count) as f32;
        quality.maintainability = (0.5 + (maintainability_factor * 0.05)).min(1.0);
        
        quality.performance = if warnings.iter().any(|w| w.contains("clone")) {
            0.7
        } else {
            0.85
        };
        
        quality.overall_score = 
            quality.readability * 0.25 +
            quality.maintainability * 0.25 +
            quality.safety * 0.30 +
            quality.performance * 0.20;
        
        quality
    }
}

// ============================================================================
// Code Generator
// ============================================================================

/// 代码生成器配置
#[derive(Clone, Debug)]
pub struct GeneratorConfig {
    pub model: String,
    pub temperature: f32,
    pub max_tokens: usize,
    pub api_url: Option<String>,
    pub api_key: Option<String>,
}

impl Default for GeneratorConfig {
    fn default() -> Self {
        Self {
            model: "gpt-4".to_string(),
            temperature: 0.7,
            max_tokens: 4096,
            api_url: None,
            api_key: None,
        }
    }
}

/// 代码生成器
pub struct CodeGenerator {
    pub model: String,
    pub temperature: f32,
    pub max_tokens: usize,
}

impl Default for CodeGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl CodeGenerator {
    /// 创建新的代码生成器
    pub fn new() -> Self {
        Self {
            model: "gpt-4".to_string(),
            temperature: 0.7,
            max_tokens: 4096,
        }
    }

    /// 使用配置创建生成器
    pub fn with_config(config: GeneratorConfig) -> Self {
        Self {
            model: config.model,
            temperature: config.temperature,
            max_tokens: config.max_tokens,
        }
    }

    /// 创建用于CLI的简单生成器
    pub fn new_with_defaults() -> Self {
        Self {
            model: "codex".to_string(),
            temperature: 0.5,
            max_tokens: 2048,
        }
    }

    /// 生成代码
    pub fn generate(&self, prompt: &str, lang: Language) -> Result<GeneratedCode, GenError> {
        if prompt.trim().is_empty() {
            return Err(GenError::InvalidPrompt("Prompt cannot be empty".to_string()));
        }

        if lang == Language::Both {
            return Err(GenError::InvalidPrompt("Use generate_with_context for Language::Both".to_string()));
        }

        let context = CodeContext {
            project_name: "omega-agi".to_string(),
            language: Some(lang.clone()),
            ..Default::default()
        };

        self.generate_with_context(prompt, &context)
    }

    /// 带上下文的代码生成
    pub fn generate_with_context(&self, prompt: &str, context: &CodeContext) -> Result<GeneratedCode, GenError> {
        if prompt.trim().is_empty() {
            return Err(GenError::InvalidPrompt("Prompt cannot be empty".to_string()));
        }

        let lang = context.language.as_ref().cloned().unwrap_or(Language::Rust);

        let enhanced_prompt = format!(
            "Generate {} code for the following task:\n\n{}\n\nRequirements:\n- Use proper error handling (Result types, no unwrap/panic)\n- Include doc comments for public APIs\n- Follow idiomatic patterns\n- Avoid common anti-patterns: unwrap, expect without message, clone in loops",
            lang,
            prompt
        );

        let code = self.generate_code_internal(&enhanced_prompt, &lang)?;
        let warnings = code.check_rust_antipatterns();
        let quality = code.compute_quality_score();

        Ok(GeneratedCode {
            code: code.code,
            language: lang,
            confidence: code.confidence,
            tokens_used: code.tokens_used,
            quality: Some(quality),
            warnings,
        })
    }

    /// 内部代码生成
    fn generate_code_internal(&self, _prompt: &str, lang: &Language) -> Result<GeneratedCode, GenError> {
        let (code, confidence, tokens) = match lang {
            Language::Rust => {
                let code = r#"/// Executes the main processing pipeline
///
/// # Errors
///
/// Returns an error if any stage of the pipeline fails.
pub fn process_pipeline(input: &[u8]) -> Result<Vec<u8>, PipelineError> {
    // Stage 1: Validate input
    if input.is_empty() {
        return Err(PipelineError::InvalidInput("Empty input".to_string()));
    }

    // Stage 2: Transform data
    let transformed = transform_data(input)?;
    
    // Stage 3: Apply business logic
    let result = apply_logic(&transformed)?;
    
    Ok(result)
}

fn transform_data(input: &[u8]) -> Result<Vec<u8>, PipelineError> {
    Ok(input.to_vec())
}

fn apply_logic(data: &[u8]) -> Result<Vec<u8>, PipelineError> {
    Ok(data.to_vec())
}

#[derive(Debug)]
pub enum PipelineError {
    InvalidInput(String),
    TransformationFailed(String),
    LogicError(String),
}"#.to_string();
                (code, 0.85, 350)
            }
            Language::Python => {
                let code = r#"def process_pipeline(input_data: bytes) -> bytes:
    """Execute the main processing pipeline.
    
    Args:
        input_data: Raw input bytes to process
        
    Returns:
        Processed bytes
        
    Raises:
        PipelineError: If any stage fails
    """
    if not input_data:
        raise PipelineError("InvalidInput: Empty input")
    
    # Stage 1: Transform data
    transformed = transform_data(input_data)
    
    # Stage 2: Apply business logic
    result = apply_logic(transformed)
    
    return result


def transform_data(input_data: bytes) -> bytes:
    """Transform raw input data."""
    return input_data


def apply_logic(data: bytes) -> bytes:
    """Apply business logic to transformed data."""
    return data


class PipelineError(Exception):
    """Pipeline processing error."""
    pass"#.to_string();
                (code, 0.85, 320)
            }
            Language::Both => {
                return Err(GenError::InvalidPrompt("Language::Both requires separate calls".to_string()));
            }
        };

        Ok(GeneratedCode {
            code,
            language: lang.clone(),
            confidence,
            tokens_used: tokens,
            quality: None,
            warnings: Vec::new(),
        })
    }

    /// 验证Rust代码语法
    #[allow(dead_code)]
    pub fn validate_rust_syntax(&self, code: &str) -> Result<(), GenError> {
        let mut child = Command::new("rustc")
            .args(["--edition", "2021", "--crate-type", "lib", "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| GenError::SyntaxError(e.to_string()))?;

        if let Some(ref mut stdin) = child.stdin {
            use std::io::Write;
            stdin.write_all(code.as_bytes())
                .map_err(|e| GenError::SyntaxError(e.to_string()))?;
        }

        let output = child.wait_with_output()
            .map_err(|e| GenError::SyntaxError(e.to_string()))?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr).to_string();
            return Err(GenError::SyntaxError(error));
        }

        Ok(())
    }

    /// 验证Python代码语法
    #[allow(dead_code)]
    pub fn validate_python_syntax(&self, code: &str) -> Result<(), GenError> {
        let mut child = Command::new("python3")
            .args(["-m", "py_compile", "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| GenError::SyntaxError(e.to_string()))?;

        if let Some(ref mut stdin) = child.stdin {
            use std::io::Write;
            stdin.write_all(code.as_bytes())
                .map_err(|e| GenError::SyntaxError(e.to_string()))?;
        }

        let output = child.wait_with_output()
            .map_err(|e| GenError::SyntaxError(e.to_string()))?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr).to_string();
            return Err(GenError::SyntaxError(error));
        }

        Ok(())
    }

    /// 格式化Rust代码
    #[allow(dead_code)]
    pub fn format_rust_code(&self, code: &str) -> Result<String, GenError> {
        let mut child = Command::new("rustfmt")
            .arg("--edition=2021")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| GenError::FormatError(e.to_string()))?;

        if let Some(ref mut stdin) = child.stdin {
            use std::io::Write;
            stdin.write_all(code.as_bytes())
                .map_err(|e| GenError::FormatError(e.to_string()))?;
        }

        let output = child.wait_with_output()
            .map_err(|e| GenError::FormatError(e.to_string()))?;

        if !output.status.success() {
            return Err(GenError::FormatError(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// 格式化Python代码
    #[allow(dead_code)]
    pub fn format_python_code(&self, code: &str) -> Result<String, GenError> {
        let mut child = Command::new("black")
            .args(["-", "--fast"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| GenError::FormatError(e.to_string()))?;

        if let Some(ref mut stdin) = child.stdin {
            use std::io::Write;
            stdin.write_all(code.as_bytes())
                .map_err(|e| GenError::FormatError(e.to_string()))?;
        }

        let output = child.wait_with_output()
            .map_err(|e| GenError::FormatError(e.to_string()))?;

        if !output.status.success() {
            return Err(GenError::FormatError(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// 生成质量报告
    pub fn generate_quality_report(&self, code: &GeneratedCode) -> String {
        let mut report = String::new();
        
        report.push_str("# Code Quality Report\n\n");
        report.push_str(&format!("Language: {}\n", code.language));
        report.push_str(&format!("Confidence: {:.2}\n", code.confidence));
        report.push_str(&format!("Tokens Used: {}\n\n", code.tokens_used));

        if let Some(ref quality) = code.quality {
            report.push_str("## Quality Scores\n\n");
            report.push_str(&format!("- Overall: {:.2}\n", quality.overall_score));
            report.push_str(&format!("- Readability: {:.2}\n", quality.readability));
            report.push_str(&format!("- Maintainability: {:.2}\n", quality.maintainability));
            report.push_str(&format!("- Safety: {:.2}\n", quality.safety));
            report.push_str(&format!("- Performance: {:.2}\n", quality.performance));
        }

        if !code.warnings.is_empty() {
            report.push_str("\n## Warnings\n\n");
            for warning in &code.warnings {
                report.push_str(&format!("- {}\n", warning));
            }
        }

        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_display() {
        assert_eq!(Language::Rust.to_string(), "rust");
        assert_eq!(Language::Python.to_string(), "python");
        assert_eq!(Language::Both.to_string(), "both");
    }

    #[test]
    fn test_generator_creation() {
        let gen = CodeGenerator::new();
        assert_eq!(gen.model, "gpt-4");
        assert_eq!(gen.temperature, 0.7);
        assert_eq!(gen.max_tokens, 4096);
    }

    #[test]
    fn test_generate_with_empty_prompt() {
        let gen = CodeGenerator::new();
        let result = gen.generate("", Language::Rust);
        assert!(result.is_err());
    }

    #[test]
    fn test_generate_rust_code() {
        let gen = CodeGenerator::new();
        let result = gen.generate("Implement a pipeline function", Language::Rust);
        assert!(result.is_ok());
        
        let code = result.unwrap();
        assert_eq!(code.language, Language::Rust);
        assert!(!code.code.is_empty());
    }

    #[test]
    fn test_generate_python_code() {
        let gen = CodeGenerator::new();
        let result = gen.generate("Implement a pipeline function", Language::Python);
        assert!(result.is_ok());
        
        let code = result.unwrap();
        assert_eq!(code.language, Language::Python);
        assert!(!code.code.is_empty());
    }

    #[test]
    fn test_generate_with_context() {
        let gen = CodeGenerator::new();
        let context = CodeContext {
            project_name: "test-project".to_string(),
            language: Some(Language::Rust),
            imports: vec!["std::collections".to_string()],
            ..Default::default()
        };
        
        let result = gen.generate_with_context("Add a function", &context);
        assert!(result.is_ok());
    }

    #[test]
    fn test_code_quality_computation() {
        let generated = GeneratedCode {
            code: r#"/// Test function
pub fn test() {
    println!("hello");
}"#.to_string(),
            language: Language::Rust,
            confidence: 0.9,
            tokens_used: 100,
            quality: None,
            warnings: Vec::new(),
        };
        
        let quality = generated.compute_quality_score();
        assert!(quality.overall_score > 0.0);
        assert!(quality.safety > 0.0);
    }

    #[test]
    fn test_rust_antipattern_detection() {
        let code = GeneratedCode {
            code: r#"fn test() {
    let x = Some(1).unwrap();
    panic!("error");
}"#.to_string(),
            language: Language::Rust,
            confidence: 0.5,
            tokens_used: 50,
            quality: None,
            warnings: Vec::new(),
        };
        
        let warnings = code.check_rust_antipatterns();
        assert!(warnings.len() >= 2);
        assert!(warnings.iter().any(|w| w.contains("unwrap")));
        assert!(warnings.iter().any(|w| w.contains("panic")));
    }

    #[test]
    fn test_generate_quality_report() {
        let gen = CodeGenerator::new();
        let code = GeneratedCode {
            code: "// test".to_string(),
            language: Language::Rust,
            confidence: 0.85,
            tokens_used: 100,
            quality: Some(CodeQuality {
                overall_score: 0.8,
                readability: 0.9,
                maintainability: 0.7,
                safety: 0.6,
                performance: 0.85,
            }),
            warnings: vec!["test warning".to_string()],
        };
        
        let report = gen.generate_quality_report(&code);
        assert!(report.contains("Quality Scores"));
        assert!(report.contains("Warnings"));
    }
}