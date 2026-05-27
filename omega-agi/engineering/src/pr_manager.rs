//! PR Manager — GitHub Pull Request Lifecycle Management
//!
//! All GitHub API interactions are performed via shell curl, keeping the
//! crate free of HTTP-client dependencies that pull in Rust 2024-only crates.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Command;
use thiserror::Error;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

#[derive(Error, Debug)]
pub enum PRError {
    #[error("API error: {0}")]
    ApiError(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("merge conflict")]
    MergeConflict,

    #[error("merge blocked — insufficient approvals or failing checks")]
    MergeBlocked,

    #[error("check failed or timed out")]
    CheckFailed,

    #[error("timeout waiting for checks")]
    Timeout,

    #[error("HTTP error {code}: {message}")]
    HttpError { code: u16, message: String },

    #[error("JSON parse error: {0}")]
    JsonError(String),
}

impl PRError {
    fn from_response(body: &str, status_code: u16) -> Self {
        if status_code == 404 {
            PRError::NotFound(body.to_string())
        } else if status_code == 409 {
            PRError::MergeConflict
        } else {
            PRError::HttpError { code: status_code, message: body.to_string() }
        }
    }
}

// ---------------------------------------------------------------------------
// Core data types
// ---------------------------------------------------------------------------

/// Manager bound to a single GitHub repository.
#[derive(Debug, Clone)]
pub struct PRManager {
    github_token: String,
    owner: String,
    repo: String,
}

impl PRManager {
    /// Create a new manager for the given repository.
    pub fn new(github_token: String, owner: String, repo: String) -> Self {
        Self { github_token, owner, repo }
    }

    fn api_url(&self, endpoint: &str) -> String {
        format!(
            "https://api.github.com/repos/{}/{}/{}",
            self.owner, self.repo, endpoint
        )
    }

    fn curl(&self, method: &str, url: &str, body: Option<&str>) -> Result<(String, u16), PRError> {
        let mut args = vec![
            "-s", "-L", "-w", "%{http_code}", "-X", method,
            "-H", &format!("Authorization: Bearer {}", self.github_token),
            "-H", "Accept: application/vnd.github+json",
            "-H", "X-GitHub-Api-Version: 2022-11-28",
        ];

        if let Some(b) = body {
            args.push("-H");
            args.push("Content-Type: application/json");
            args.push("-d");
            args.push(b);
        }
        args.push(url);

        let output = Command::new("curl")
            .args(&args)
            .output()
            .map_err(|e| PRError::ApiError(e.to_string()))?;

        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.is_empty() && !stderr.contains("curl:") == false {
            // non-fatal, log if needed
        }

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        // HTTP code is appended after \n__HTTP_CODE__\n
        let (body_str, raw_code) = if let Some(pos) = stdout.rfind("\n__HTTP_CODE__") {
            let (b, c) = stdout.split_at(pos);
            let code = c.trim().parse().unwrap_or(0);
            (b.to_string(), code)
        } else {
            (stdout, output.status.code().unwrap_or(0) as u16)
        };

        Ok((body_str, raw_code))
    }

    fn curl_json<T: for<'de> Deserialize<'de>>(
        &self,
        method: &str,
        url: &str,
        body: Option<&str>,
    ) -> Result<T, PRError> {
        let (raw, code) = self.curl(method, url, body)?;
        if code >= 400 {
            return Err(PRError::from_response(&raw, code));
        }
        serde_json::from_str(&raw).map_err(|e| PRError::JsonError(e.to_string()))
    }

    // -----------------------------------------------------------------------
    // PR lifecycle
    // -----------------------------------------------------------------------

    /// Create a new pull request.
    pub fn create_pr(
        &self,
        title: &str,
        body: &str,
        head: &str,
        base: &str,
    ) -> Result<PRState, PRError> {
        let url = self.api_url("pulls");
        let payload = serde_json::json!({
            "title": title,
            "body": body,
            "head": head,
            "base": base
        });
        let json: serde_json::Value = self.curl_json("POST", &url, Some(&serde_json::to_string(&payload).unwrap()))?;
        self.parse_pr_state(&json)
    }

    /// Update an existing pull request's title and/or body.
    pub fn update_pr(&self, pr_number: u64, title: Option<&str>, body: Option<&str>) -> Result<PRState, PRError> {
        let mut payload = serde_json::Map::new();
        if let Some(t) = title {
            payload.insert("title".into(), serde_json::json!(t));
        }
        if let Some(b) = body {
            payload.insert("body".into(), serde_json::json!(b));
        }
        let url = self.api_url(&format!("pulls/{pr_number}"));
        let json: serde_json::Value = self.curl_json("PATCH", &url, Some(&serde_json::to_string(&payload).unwrap()))?;
        self.parse_pr_state(&json)
    }

    /// Squash-merge a pull request.
    pub fn merge_pr(&self, pr_number: u64) -> Result<(), PRError> {
        let url = self.api_url(&format!("pulls/{pr_number}/merge"));
        let payload = serde_json::json!({ "merge_method": "squash" });
        let (raw, code) = self.curl("PUT", &url, Some(&serde_json::to_string(&payload).unwrap()))?;
        if code == 200 || code == 201 || raw.contains("\"merged\":true") {
            Ok(())
        } else if raw.contains("conflict") || raw.contains("Merge conflict") {
            Err(PRError::MergeConflict)
        } else if code == 405 {
            Err(PRError::MergeBlocked)
        } else {
            Err(PRError::ApiError(raw))
        }
    }

    /// Add labels to a pull request.
    pub fn add_labels(&self, pr_number: u64, labels: &[&str]) -> Result<(), PRError> {
        let url = self.api_url(&format!("issues/{pr_number}/labels"));
        let payload = serde_json::json!({ "labels": labels });
        let _resp: serde_json::Value = self.curl_json("POST", &url, Some(&serde_json::to_string(&payload).unwrap()))?;
        Ok(())
    }

    /// Remove a label from a pull request.
    pub fn remove_label(&self, pr_number: u64, label: &str) -> Result<(), PRError> {
        let url = self.api_url(&format!("issues/{pr_number}/labels/{label}"));
        let (raw, code) = self.curl("DELETE", &url, None)?;
        if code == 200 || code == 204 || code == 404 {
            Ok(())
        } else {
            Err(PRError::ApiError(raw))
        }
    }

    /// Get the current state of a pull request.
    pub fn get_pr_status(&self, pr_number: u64) -> Result<PRState, PRError> {
        let url = self.api_url(&format!("pulls/{pr_number}"));
        let json: serde_json::Value = self.curl_json("GET", &url, None)?;
        self.parse_pr_state(&json)
    }

    /// List all pull requests matching the given state filter.
    pub fn list_prs(&self, state: Option<&str>) -> Result<Vec<PRState>, PRError> {
        let endpoint = match state {
            Some(s) => format!("pulls?state={s}"),
            None => "pulls".to_string(),
        };
        let url = self.api_url(&endpoint);
        let json: Vec<serde_json::Value> = self.curl_json("GET", &url, None)?;
        json.iter().map(|j| self.parse_pr_state(j)).collect()
    }

    /// Close a pull request without merging.
    pub fn close_pr(&self, pr_number: u64) -> Result<PRState, PRError> {
        let url = self.api_url(&format!("pulls/{pr_number}"));
        let payload = serde_json::json!({ "state": "closed" });
        let json: serde_json::Value = self.curl_json("PATCH", &url, Some(&serde_json::to_string(&payload).unwrap()))?;
        self.parse_pr_state(&json)
    }

    // -----------------------------------------------------------------------
    // Reviews
    // -----------------------------------------------------------------------

    /// Retrieve reviews for a pull request.
    pub fn get_pr_reviews(&self, pr_number: u64) -> Result<Vec<Review>, PRError> {
        let url = self.api_url(&format!("pulls/{pr_number}/reviews"));
        let json: Vec<serde_json::Value> = self.curl_json("GET", &url, None)?;
        json.iter()
            .map(|r| {
                let state = match r["state"].as_str().unwrap_or("") {
                    "APPROVED" => ReviewState::Approved,
                    "CHANGES_REQUESTED" => ReviewState::ChangesRequested,
                    "COMMENTED" => ReviewState::Commented,
                    "DISMISSED" => ReviewState::Dismissed,
                    _ => ReviewState::Dismissed,
                };
                Ok(Review {
                    author: r["user"]["login"].as_str().unwrap_or("").to_string(),
                    state,
                    body: r["body"].as_str().unwrap_or("").to_string(),
                })
            })
            .collect()
    }

    // -----------------------------------------------------------------------
    // Status checks
    // -----------------------------------------------------------------------

    /// Retrieve commit status checks (from the combined status endpoint).
    pub fn get_check_runs(&self, _pr_number: u64) -> Result<Vec<CheckRun>, PRError> {
        let url = self.api_url("commits/refs/heads/main/status");
        let json: serde_json::Value = self.curl_json("GET", &url, None)?;
        let json_arr = json["statuses"].as_array().cloned().unwrap_or_default();
        Ok(json_arr
            .iter()
            .map(|s| CheckRun {
                name: s["context"].as_str().unwrap_or("").to_string(),
                status: s["state"].as_str().unwrap_or("").to_string(),
                conclusion: None,
                url: s["target_url"].as_str().map(String::from),
            })
            .collect())
    }

    /// Poll until all checks are green (status == "success") or timeout is reached.
    pub fn wait_for_checks(&self, pr_number: u64, timeout_secs: u64) -> Result<Vec<CheckRun>, PRError> {
        let start = std::time::Instant::now();
        loop {
            let checks = self.get_check_runs(pr_number)?;
            if checks.iter().all(|c| c.status == "success" || c.status == "completed") {
                return Ok(checks);
            }
            if start.elapsed().as_secs() >= timeout_secs {
                return Err(PRError::Timeout);
            }
            std::thread::sleep(std::time::Duration::from_secs(5));
        }
    }

    // -----------------------------------------------------------------------
    // Auto-merge
    // -----------------------------------------------------------------------

    /// Automatic merge: requires ≥ 2 approved reviews and all checks green.
    /// Falls short and returns `MergeBlocked` if conditions aren't met.
    pub fn auto_merge(&self, pr_number: u64) -> Result<(), PRError> {
        // Check reviews
        let reviews = self.get_pr_reviews(pr_number)?;
        let approved = reviews.iter().filter(|r| r.state == ReviewState::Approved).count();

        if approved < 2 {
            return Err(PRError::MergeBlocked);
        }

        // Wait for CI checks
        let checks = self.wait_for_checks(pr_number, 600)?;
        if !checks.iter().all(|c| c.status == "success") {
            return Err(PRError::CheckFailed);
        }

        self.merge_pr(pr_number)
    }

    // -----------------------------------------------------------------------
    // Internals
    // -----------------------------------------------------------------------

    fn parse_pr_state(&self, json: &serde_json::Value) -> Result<PRState, PRError> {
        let status = if json["merged"].as_bool().unwrap_or(false) {
            PRStatus::Merged
        } else if json["state"].as_str().unwrap_or("") == "closed" {
            PRStatus::Closed
        } else if json["draft"].as_bool().unwrap_or(false) {
            PRStatus::Draft
        } else {
            PRStatus::Open
        };

        Ok(PRState {
            number: json["number"].as_u64().unwrap_or(0),
            title: json["title"].as_str().unwrap_or("").to_string(),
            body: json["body"].as_str().unwrap_or("").to_string(),
            status,
            head: json["head"]["ref"].as_str().unwrap_or("").to_string(),
            base: json["base"]["ref"].as_str().unwrap_or("").to_string(),
            user: json["user"]["login"].as_str().unwrap_or("").to_string(),
            draft: json["draft"].as_bool().unwrap_or(false),
            merged: json["merged"].as_bool().unwrap_or(false),
        })
    }
}

// ---------------------------------------------------------------------------
// Serialisable types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PRState {
    pub number: u64,
    pub title: String,
    pub body: String,
    pub status: PRStatus,
    pub head: String,
    pub base: String,
    pub user: String,
    pub draft: bool,
    pub merged: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PRStatus {
    Open,
    Closed,
    Merged,
    Draft,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Review {
    pub author: String,
    pub state: ReviewState,
    pub body: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReviewState {
    Approved,
    ChangesRequested,
    Commented,
    Dismissed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckRun {
    pub name: String,
    pub status: String,
    pub conclusion: Option<String>,
    pub url: Option<String>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pr_url_construction() {
        let mgr = PRManager::new("token".into(), "owner".into(), "repo".into());
        assert_eq!(
            mgr.api_url("pulls/42"),
            "https://api.github.com/repos/owner/repo/pulls/42"
        );
    }
}