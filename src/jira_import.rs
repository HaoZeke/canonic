//! Free-tier Jira **platform** REST only (no Marketplace apps).
//!
//! Endpoint map (official Cloud + Server/DC docs):
//!
//! | Op | Endpoint | Notes |
//! |----|----------|-------|
//! | Probe | `GET /rest/api/2/myself` (+ `serverInfo`) | Cloud Free email+API token or Server PAT |
//! | Search | `GET /rest/api/2/search?jql=&fields=summary` | Free JQL; Server also allows POST search |
//! | Comments GET | `GET /rest/api/2/issue/{key}/comment` | Wiki string **or** Cloud ADF body |
//! | Comment POST | Server: `POST /rest/api/2/.../comment` wiki · Cloud: `POST /rest/api/3/.../comment` ADF | No paid formatters |
//!
//! Import writes drafts under `corpus/imports/` only. Write is explicit one-shot
//! comment post (review-before-migrate), not bulk library sync.

use crate::convert::convert_jira_to_markdown;
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Where imported drafts land by default (never auto-indexed or quality-checked).
pub fn default_import_dir() -> PathBuf {
    PathBuf::from("corpus/imports")
}

/// How to authenticate against the Jira REST API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JiraAuth {
    /// Jira Cloud convention: an account email plus an API token.
    Basic { user: String, token: String },
    /// A raw `Authorization` header value, e.g. `Bearer <personal-access-token>`
    /// for Jira Server/Data Center.
    Header(String),
}

/// Jira instance connection details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JiraConfig {
    pub base_url: String,
    pub auth: JiraAuth,
}

impl JiraConfig {
    pub fn new(base_url: impl Into<String>, auth: JiraAuth) -> Self {
        Self {
            base_url: base_url.into(),
            auth,
        }
    }

    /// Read `JIRA_BASE_URL` plus either `JIRA_AUTH_HEADER` (a raw `Authorization`
    /// value) or `JIRA_EMAIL` + `JIRA_API_TOKEN` (Basic auth).
    pub fn from_env() -> Result<Self> {
        let base_url = std::env::var("JIRA_BASE_URL")
            .context("JIRA_BASE_URL is not set (e.g. https://your-instance.atlassian.net)")?;
        if let Ok(header) = std::env::var("JIRA_AUTH_HEADER") {
            return Ok(Self::new(base_url, JiraAuth::Header(header)));
        }
        let user = std::env::var("JIRA_EMAIL")
            .context("set JIRA_AUTH_HEADER, or JIRA_EMAIL + JIRA_API_TOKEN")?;
        let token = std::env::var("JIRA_API_TOKEN").context("JIRA_API_TOKEN is not set")?;
        Ok(Self::new(base_url, JiraAuth::Basic { user, token }))
    }
}

fn apply_auth(
    req: reqwest::blocking::RequestBuilder,
    auth: &JiraAuth,
) -> reqwest::blocking::RequestBuilder {
    match auth {
        JiraAuth::Basic { user, token } => req.basic_auth(user, Some(token)),
        JiraAuth::Header(value) => req.header(reqwest::header::AUTHORIZATION, value),
    }
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    #[serde(rename = "startAt")]
    start_at: u32,
    total: u32,
    issues: Vec<SearchIssue>,
}

#[derive(Debug, Deserialize)]
struct SearchIssue {
    key: String,
    fields: SearchFields,
}

#[derive(Debug, Deserialize)]
struct SearchFields {
    summary: String,
}

/// One issue found by a JQL search: key plus its summary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueSummary {
    pub key: String,
    pub summary: String,
}

fn parse_search_response(json: &str) -> Result<(Vec<IssueSummary>, u32, u32)> {
    let parsed: SearchResponse =
        serde_json::from_str(json).context("parse Jira search response")?;
    let issues = parsed
        .issues
        .into_iter()
        .map(|i| IssueSummary {
            key: i.key,
            summary: i.fields.summary,
        })
        .collect();
    Ok((issues, parsed.start_at, parsed.total))
}

#[derive(Debug, Deserialize)]
struct CommentsResponse {
    comments: Vec<RawComment>,
}

#[derive(Debug, Deserialize)]
struct RawComment {
    author: RawUser,
    /// Cloud free REST may return ADF objects; Server/DC wiki is a string.
    body: serde_json::Value,
    created: String,
}

#[derive(Debug, Deserialize)]
struct RawUser {
    #[serde(rename = "displayName")]
    display_name: String,
}

/// One comment on a Jira issue, with its body still in Jira wiki markup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueComment {
    pub author: String,
    pub created: String,
    pub body_wiki: String,
}

/// Flatten free-platform comment bodies: wiki strings stay as-is; Cloud ADF
/// (Atlassian Document Format) is reduced to plain text without Marketplace apps.
pub fn comment_body_to_text(body: &serde_json::Value) -> String {
    match body {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Object(_) | serde_json::Value::Array(_) => adf_to_plain_text(body),
        other => other.to_string(),
    }
}

fn adf_to_plain_text(node: &serde_json::Value) -> String {
    let mut out = String::new();
    adf_walk(node, &mut out);
    out.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

fn adf_walk(node: &serde_json::Value, out: &mut String) {
    match node {
        serde_json::Value::Object(map) => {
            if let Some(serde_json::Value::String(text)) = map.get("text") {
                if !out.is_empty() && !out.ends_with(' ') && !out.ends_with('\n') {
                    out.push(' ');
                }
                out.push_str(text);
            }
            if let Some(children) = map.get("content").and_then(|c| c.as_array()) {
                for child in children {
                    adf_walk(child, out);
                }
                // paragraph/heading breaks
                if matches!(map.get("type").and_then(|t| t.as_str()), Some("paragraph" | "heading")) {
                    out.push('\n');
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                adf_walk(item, out);
            }
        }
        _ => {}
    }
}

/// Build minimal free ADF document from plain/wiki text (Cloud comment write path).
///
/// Official Cloud v3 comments expect Atlassian Document Format bodies
/// (developer.atlassian.com Cloud platform REST). This does not use paid apps.
pub fn plain_text_to_adf(text: &str) -> serde_json::Value {
    let paragraphs: Vec<serde_json::Value> = text
        .split("\n")
        .map(|line| line.trim_end())
        .filter(|line| !line.is_empty() || text.contains('\n'))
        .map(|line| {
            serde_json::json!({
                "type": "paragraph",
                "content": if line.is_empty() {
                    Vec::<serde_json::Value>::new()
                } else {
                    vec![serde_json::json!({"type": "text", "text": line})]
                }
            })
        })
        .collect();
    let content = if paragraphs.is_empty() {
        vec![serde_json::json!({
            "type": "paragraph",
            "content": [{"type": "text", "text": text}]
        })]
    } else {
        paragraphs
    };
    serde_json::json!({
        "type": "doc",
        "version": 1,
        "content": content
    })
}

/// How to encode free write comment bodies for platform REST.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CommentBodyFormat {
    /// Server/Data Center wiki string: `{"body":"h1. ..."}` on `/rest/api/2/...`.
    Wiki,
    /// Cloud Free-compatible ADF on `/rest/api/3/...` (no Marketplace formatter).
    Adf,
    /// `*.atlassian.net` → Adf, otherwise Wiki.
    #[default]
    Auto,
}

impl CommentBodyFormat {
    pub fn resolve(self, base_url: &str) -> CommentBodyFormat {
        match self {
            CommentBodyFormat::Auto => {
                if is_cloud_host(base_url) {
                    CommentBodyFormat::Adf
                } else {
                    CommentBodyFormat::Wiki
                }
            }
            other => other,
        }
    }
}

/// True when the base URL looks like Jira Cloud Free/Standard host.
pub fn is_cloud_host(base_url: &str) -> bool {
    let lower = base_url.to_ascii_lowercase();
    lower.contains("atlassian.net") || lower.contains("jira.com")
}

fn parse_comments_response(json: &str) -> Result<Vec<IssueComment>> {
    let parsed: CommentsResponse =
        serde_json::from_str(json).context("parse Jira comments response")?;
    Ok(parsed
        .comments
        .into_iter()
        .map(|c| IssueComment {
            author: c.author.display_name,
            created: c.created,
            body_wiki: comment_body_to_text(&c.body),
        })
        .collect())
}

fn build_search_request(
    client: &reqwest::blocking::Client,
    cfg: &JiraConfig,
    jql: &str,
    start_at: u32,
    max_results: u32,
) -> reqwest::Result<reqwest::blocking::Request> {
    let url = format!("{}/rest/api/2/search", cfg.base_url.trim_end_matches('/'));
    let start_at_s = start_at.to_string();
    let max_results_s = max_results.to_string();
    let req = client.get(&url).query(&[
        ("jql", jql),
        ("startAt", start_at_s.as_str()),
        ("maxResults", max_results_s.as_str()),
        ("fields", "summary"),
    ]);
    apply_auth(req, &cfg.auth).build()
}

fn build_comments_request(
    client: &reqwest::blocking::Client,
    cfg: &JiraConfig,
    issue_key: &str,
) -> reqwest::Result<reqwest::blocking::Request> {
    let url = format!(
        "{}/rest/api/2/issue/{issue_key}/comment",
        cfg.base_url.trim_end_matches('/')
    );
    apply_auth(client.get(&url), &cfg.auth).build()
}

/// Search for issues matching `jql`, paginating until Jira reports no more.
pub fn search_all_issues(
    cfg: &JiraConfig,
    jql: &str,
    page_size: u32,
) -> Result<Vec<IssueSummary>> {
    let client = reqwest::blocking::Client::new();
    let mut out = Vec::new();
    let mut start_at = 0u32;
    loop {
        let req = build_search_request(&client, cfg, jql, start_at, page_size)
            .context("build Jira search request")?;
        let resp = client.execute(req).context("send Jira search request")?;
        if !resp.status().is_success() {
            bail!("Jira search failed: HTTP {}", resp.status());
        }
        let body = resp.text().context("read Jira search response")?;
        let (mut issues, returned_start, total) = parse_search_response(&body)?;
        let got = issues.len() as u32;
        out.append(&mut issues);
        if got == 0 || returned_start + got >= total {
            break;
        }
        start_at = returned_start + got;
    }
    Ok(out)
}

/// Fetch all comments for one issue.
pub fn fetch_comments(cfg: &JiraConfig, issue_key: &str) -> Result<Vec<IssueComment>> {
    let client = reqwest::blocking::Client::new();
    let req = build_comments_request(&client, cfg, issue_key)
        .with_context(|| format!("build comments request for {issue_key}"))?;
    let resp = client
        .execute(req)
        .with_context(|| format!("fetch comments for {issue_key}"))?;
    if !resp.status().is_success() {
        bail!(
            "fetching comments for {issue_key} failed: HTTP {}",
            resp.status()
        );
    }
    let body = resp.text().context("read Jira comments response")?;
    parse_comments_response(&body)
}

/// Turn a title into a slug: lowercase ASCII alphanumerics, everything else
/// collapsed to a single `-`, with no leading, trailing, or repeated dash.
pub fn slugify(title: &str) -> String {
    let mut slug = String::new();
    let mut last_dash = true; // suppress a leading dash
    for c in title.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c);
            last_dash = false;
        } else if !last_dash {
            slug.push('-');
            last_dash = true;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    if slug.is_empty() {
        "topic".into()
    } else {
        slug
    }
}

/// Render one issue's comments as a review draft: front matter plus one
/// section per comment. `id` must already carry the `resp-` prefix.
pub fn draft_markdown(
    id: &str,
    issue_key: &str,
    title: &str,
    comments: &[(String, String, String)],
) -> String {
    let mut out = format!(
        "---\nid: {id}\ntitle: {title}\nprefix: resp\ntags: []\nsop: none\n---\n\n# {title}\n\n<!-- imported from {issue_key}; edit into a real answer, then move to corpus/responses/ -->\n"
    );
    for (author, created, body_md) in comments {
        out.push_str(&format!(
            "\n## Comment by {author} ({created})\n\n{}\n",
            body_md.trim()
        ));
    }
    out
}

/// Import issues matching `jql` into `out_dir` as review drafts (never
/// `corpus/responses/`). Returns the paths written, or, when `dry_run` is
/// set, the paths that would be written without touching the filesystem or
/// fetching comments.
pub fn import_jira(
    cfg: &JiraConfig,
    jql: &str,
    out_dir: &Path,
    max_results: u32,
    dry_run: bool,
) -> Result<Vec<PathBuf>> {
    let issues = search_all_issues(cfg, jql, max_results)?;
    if !dry_run {
        std::fs::create_dir_all(out_dir)
            .with_context(|| format!("create {}", out_dir.display()))?;
    }
    let mut written = Vec::new();
    for issue in issues {
        let id = format!(
            "resp-{}-{}",
            slugify(&issue.summary),
            issue.key.to_lowercase()
        );
        let path = out_dir.join(format!("{id}.md"));
        if dry_run {
            written.push(path);
            continue;
        }
        let comments = fetch_comments(cfg, &issue.key)?;
        let rendered = comments
            .into_iter()
            .map(|c| -> Result<(String, String, String)> {
                let md = convert_jira_to_markdown(&c.body_wiki)?;
                Ok((c.author, c.created, md))
            })
            .collect::<Result<Vec<_>>>()?;
        let text = draft_markdown(&id, &issue.key, &issue.summary, &rendered);
        std::fs::write(&path, text).with_context(|| format!("write {}", path.display()))?;
        written.push(path);
    }
    Ok(written)
}

/// Result of a free-tier connectivity probe (`/myself` + optional serverInfo).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JiraProbe {
    pub base_url: String,
    pub display_name: String,
    pub account: String,
    pub server_title: Option<String>,
    pub server_version: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MyselfResponse {
    #[serde(default)]
    name: Option<String>,
    #[serde(rename = "displayName", default)]
    display_name: Option<String>,
    #[serde(rename = "accountId", default)]
    account_id: Option<String>,
    #[serde(rename = "emailAddress", default)]
    email: Option<String>,
}

fn parse_myself_response(json: &str) -> Result<(String, String)> {
    let parsed: MyselfResponse =
        serde_json::from_str(json).context("parse Jira /myself response")?;
    let display = parsed
        .display_name
        .or_else(|| parsed.name.clone())
        .or_else(|| parsed.email.clone())
        .unwrap_or_else(|| "(unknown)".into());
    let account = parsed
        .account_id
        .or(parsed.name)
        .or(parsed.email)
        .unwrap_or_else(|| "(unknown)".into());
    Ok((display, account))
}

#[derive(Debug, Deserialize)]
struct ServerInfoResponse {
    #[serde(rename = "serverTitle", default)]
    server_title: Option<String>,
    #[serde(default)]
    version: Option<String>,
}

fn parse_server_info_response(json: &str) -> Result<(Option<String>, Option<String>)> {
    let parsed: ServerInfoResponse =
        serde_json::from_str(json).context("parse Jira /serverInfo response")?;
    Ok((parsed.server_title, parsed.version))
}

fn build_myself_request(
    client: &reqwest::blocking::Client,
    cfg: &JiraConfig,
) -> reqwest::Result<reqwest::blocking::Request> {
    let url = format!("{}/rest/api/2/myself", cfg.base_url.trim_end_matches('/'));
    apply_auth(client.get(&url), &cfg.auth).build()
}

fn build_server_info_request(
    client: &reqwest::blocking::Client,
    cfg: &JiraConfig,
) -> reqwest::Result<reqwest::blocking::Request> {
    let url = format!(
        "{}/rest/api/2/serverInfo",
        cfg.base_url.trim_end_matches('/')
    );
    apply_auth(client.get(&url), &cfg.auth).build()
}

/// Probe free Jira REST: authenticated identity + optional server banner.
///
/// Uses only platform endpoints (`/rest/api/2/myself`, `/serverInfo`) — no
/// Marketplace apps. Exit path for CLI: non-zero when HTTP fails or auth rejects.
pub fn probe_jira(cfg: &JiraConfig) -> Result<JiraProbe> {
    let client = reqwest::blocking::Client::new();
    let req = build_myself_request(&client, cfg).context("build /myself request")?;
    let resp = client.execute(req).context("send /myself request")?;
    if !resp.status().is_success() {
        bail!(
            "Jira probe failed (auth or reachability): HTTP {} on /rest/api/2/myself — free Cloud needs email+API token; Server/DC may use JIRA_AUTH_HEADER",
            resp.status()
        );
    }
    let body = resp.text().context("read /myself body")?;
    let (display_name, account) = parse_myself_response(&body)?;

    let mut server_title = None;
    let mut server_version = None;
    if let Ok(req) = build_server_info_request(&client, cfg) {
        if let Ok(resp) = client.execute(req) {
            if resp.status().is_success() {
                if let Ok(text) = resp.text() {
                    if let Ok((t, v)) = parse_server_info_response(&text) {
                        server_title = t;
                        server_version = v;
                    }
                }
            }
        }
    }

    Ok(JiraProbe {
        base_url: cfg.base_url.trim_end_matches('/').to_string(),
        display_name,
        account,
        server_title,
        server_version,
    })
}

/// Format a probe result for CLI stdout.
pub fn format_probe(probe: &JiraProbe) -> String {
    let mut lines = vec![
        format!("jira: ok — free REST platform API (no Marketplace apps)"),
        format!("  base: {}", probe.base_url),
        format!("  user: {} ({})", probe.display_name, probe.account),
    ];
    if let Some(ref t) = probe.server_title {
        lines.push(format!("  server: {t}"));
    }
    if let Some(ref v) = probe.server_version {
        lines.push(format!("  version: {v}"));
    }
    lines.push(
        "  note: write uses POST issue comment only; bulk library sync stays human-gated"
            .into(),
    );
    lines.join("\n") + "\n"
}

/// One comment created via free REST POST.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostedComment {
    pub issue_key: String,
    pub comment_id: String,
    pub body_wiki: String,
}

#[derive(Debug, Deserialize)]
struct CommentCreateResponse {
    id: Option<String>,
    #[serde(default)]
    body: Option<String>,
}

fn parse_comment_create_response(json: &str) -> Result<(String, Option<String>)> {
    let parsed: CommentCreateResponse =
        serde_json::from_str(json).context("parse Jira comment create response")?;
    let id = parsed
        .id
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "(unknown)".into());
    Ok((id, parsed.body))
}

/// Free platform comment endpoints (official Cloud/Server REST — no Marketplace).
///
/// | Host | Format | Endpoint |
/// |------|--------|----------|
/// | Server/DC | wiki string | `POST /rest/api/2/issue/{key}/comment` `{"body":"wiki"}` |
/// | Cloud Free | ADF | `POST /rest/api/3/issue/{key}/comment` ADF `body` object |
///
/// See developer.atlassian.com Cloud platform REST (issue comments) and Server
/// REST API examples. Auth: Cloud email+API token Basic, or Server PAT header.
fn comment_post_url(base_url: &str, issue_key: &str, format: CommentBodyFormat) -> String {
    let base = base_url.trim_end_matches('/');
    let api = match format.resolve(base_url) {
        CommentBodyFormat::Adf => "3",
        _ => "2",
    };
    format!("{base}/rest/api/{api}/issue/{issue_key}/comment")
}

fn comment_post_payload(body_wiki: &str, format: CommentBodyFormat, base_url: &str) -> serde_json::Value {
    match format.resolve(base_url) {
        CommentBodyFormat::Adf => serde_json::json!({ "body": plain_text_to_adf(body_wiki) }),
        _ => serde_json::json!({ "body": body_wiki }),
    }
}

fn build_comment_post_request(
    client: &reqwest::blocking::Client,
    cfg: &JiraConfig,
    issue_key: &str,
    body_wiki: &str,
    format: CommentBodyFormat,
) -> reqwest::Result<reqwest::blocking::Request> {
    let url = comment_post_url(&cfg.base_url, issue_key, format);
    let payload = comment_post_payload(body_wiki, format, &cfg.base_url);
    apply_auth(client.post(&url).json(&payload), &cfg.auth).build()
}

/// POST a comment via free platform REST (wiki on Server/DC, ADF on Cloud Free).
///
/// `body_wiki` is pandoc jira markup (or plain text). On Cloud Free, it is wrapped
/// in minimal ADF. No Marketplace apps.
pub fn post_issue_comment(
    cfg: &JiraConfig,
    issue_key: &str,
    body_wiki: &str,
) -> Result<PostedComment> {
    post_issue_comment_with_format(cfg, issue_key, body_wiki, CommentBodyFormat::Auto)
}

/// Like [`post_issue_comment`] with an explicit body format.
pub fn post_issue_comment_with_format(
    cfg: &JiraConfig,
    issue_key: &str,
    body_wiki: &str,
    format: CommentBodyFormat,
) -> Result<PostedComment> {
    if issue_key.trim().is_empty() {
        bail!("issue key is empty");
    }
    if body_wiki.trim().is_empty() {
        bail!("comment body is empty");
    }
    let resolved = format.resolve(&cfg.base_url);
    let client = reqwest::blocking::Client::new();
    let req = build_comment_post_request(&client, cfg, issue_key, body_wiki, resolved)
        .with_context(|| format!("build comment POST for {issue_key}"))?;
    let resp = client
        .execute(req)
        .with_context(|| format!("POST comment on {issue_key}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let detail = resp.text().unwrap_or_default();
        bail!(
            "posting comment on {issue_key} failed: HTTP {status} (format={resolved:?}){}",
            if detail.is_empty() {
                String::new()
            } else {
                format!(" — {detail}")
            }
        );
    }
    let text = resp.text().context("read comment create body")?;
    let (comment_id, _) = parse_comment_create_response(&text)?;
    Ok(PostedComment {
        issue_key: issue_key.to_string(),
        comment_id,
        body_wiki: body_wiki.to_string(),
    })
}

/// Convert a markdown corpus file with pandoc jira writer, then POST as a comment.
///
/// When `dry_run` is true, returns the wiki body that would be posted without
/// calling Jira (still requires pandoc for a real conversion).
pub fn post_comment_from_markdown(
    cfg: &JiraConfig,
    issue_key: &str,
    markdown_path: &Path,
    dry_run: bool,
) -> Result<PostedComment> {
    post_comment_from_markdown_with_format(
        cfg,
        issue_key,
        markdown_path,
        dry_run,
        CommentBodyFormat::Auto,
    )
}

/// Like [`post_comment_from_markdown`] with an explicit free-tier body format.
pub fn post_comment_from_markdown_with_format(
    cfg: &JiraConfig,
    issue_key: &str,
    markdown_path: &Path,
    dry_run: bool,
    format: CommentBodyFormat,
) -> Result<PostedComment> {
    let wiki = crate::convert::convert_path_to_jira(markdown_path)
        .with_context(|| format!("convert {} to jira markup", markdown_path.display()))?;
    if dry_run {
        return Ok(PostedComment {
            issue_key: issue_key.to_string(),
            comment_id: "(dry-run)".into(),
            body_wiki: wiki,
        });
    }
    post_issue_comment_with_format(cfg, issue_key, &wiki, format)
}


#[cfg(test)]
mod tests {
    use super::*;

    const SEARCH_FIXTURE: &str = r#"{
        "expand": "names,schema",
        "startAt": 0,
        "maxResults": 50,
        "total": 1,
        "issues": [
            {
                "id": "10000",
                "key": "HSP-1",
                "self": "https://example.atlassian.net/rest/api/2/issue/10000",
                "fields": { "summary": "Project space is not a backup" }
            }
        ]
    }"#;

    const COMMENTS_FIXTURE: &str = r#"{
        "startAt": 0,
        "maxResults": 50,
        "total": 1,
        "comments": [
            {
                "id": "10000",
                "author": { "name": "fred", "displayName": "Fred F. User" },
                "body": "h1. Heads up\n\nUse *self-service* for backups.",
                "created": "2026-07-06T18:30:00.000+0000",
                "updated": "2026-07-06T18:30:00.000+0000"
            }
        ]
    }"#;

    #[test]
    fn parses_search_response_fixture() {
        let (issues, start_at, total) = parse_search_response(SEARCH_FIXTURE).unwrap();
        assert_eq!(start_at, 0);
        assert_eq!(total, 1);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].key, "HSP-1");
        assert_eq!(issues[0].summary, "Project space is not a backup");
    }

    #[test]
    fn parses_comments_response_fixture() {
        let comments = parse_comments_response(COMMENTS_FIXTURE).unwrap();
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].author, "Fred F. User");
        assert_eq!(comments[0].created, "2026-07-06T18:30:00.000+0000");
        assert!(comments[0].body_wiki.contains("self-service"));
    }

    #[test]
    fn slugify_collapses_punctuation_and_case() {
        assert_eq!(
            slugify("Project space is not a backup!"),
            "project-space-is-not-a-backup"
        );
        assert_eq!(slugify("  ---  "), "topic");
        assert_eq!(slugify("SBU / GPU-hours (2026)"), "sbu-gpu-hours-2026");
    }

    #[test]
    fn search_request_targets_v2_search_with_auth() {
        let client = reqwest::blocking::Client::new();
        let cfg = JiraConfig::new(
            "https://example.atlassian.net/",
            JiraAuth::Basic {
                user: "advisor@example.org".into(),
                token: "tok".into(),
            },
        );
        let req = build_search_request(&client, &cfg, "project = HSP", 0, 50).unwrap();
        assert_eq!(req.url().path(), "/rest/api/2/search");
        let query = req.url().query().unwrap_or_default();
        assert!(query.contains("jql=project"), "{query}");
        assert!(query.contains("maxResults=50"), "{query}");
        assert!(req.headers().contains_key(reqwest::header::AUTHORIZATION));
    }

    #[test]
    fn comments_request_targets_issue_path_with_bearer_header() {
        let client = reqwest::blocking::Client::new();
        let cfg = JiraConfig::new(
            "https://jira.example.org",
            JiraAuth::Header("Bearer some-pat".into()),
        );
        let req = build_comments_request(&client, &cfg, "HSP-1").unwrap();
        assert_eq!(req.url().path(), "/rest/api/2/issue/HSP-1/comment");
        assert_eq!(
            req.headers()
                .get(reqwest::header::AUTHORIZATION)
                .unwrap(),
            "Bearer some-pat"
        );
    }

    #[test]
    fn draft_markdown_has_required_front_matter_and_sections() {
        let text = draft_markdown(
            "resp-example-hsp-1",
            "HSP-1",
            "Example topic",
            &[(
                "Fred F. User".into(),
                "2026-07-06T18:30:00.000+0000".into(),
                "Use **self-service** for backups.".into(),
            )],
        );
        assert!(text.contains("id: resp-example-hsp-1"));
        assert!(text.contains("prefix: resp"));
        assert!(text.contains("sop: none"));
        assert!(text.contains("imported from HSP-1"));
        assert!(text.contains("## Comment by Fred F. User"));
        assert!(text.contains("self-service"));
    }

    #[test]
    fn from_env_requires_base_url() {
        // Isolated by variable name; does not touch real Jira config.
        std::env::remove_var("JIRA_BASE_URL");
        let err = JiraConfig::from_env().unwrap_err().to_string();
        assert!(err.contains("JIRA_BASE_URL"));
    }

    const MYSELF_FIXTURE: &str = r#"{
        "self": "https://example.atlassian.net/rest/api/2/user?username=advisor",
        "name": "advisor",
        "emailAddress": "advisor@example.org",
        "displayName": "Advisor User",
        "active": true,
        "accountId": "abc-123"
    }"#;

    const SERVER_INFO_FIXTURE: &str = r#"{
        "baseUrl": "https://example.atlassian.net",
        "version": "9.12.15",
        "versionNumbers": [9, 12, 15],
        "deploymentType": "Server",
        "serverTitle": "canonic-smoke"
    }"#;

    const COMMENT_CREATE_FIXTURE: &str = r#"{
        "id": "30001",
        "author": { "name": "advisor", "displayName": "Advisor User" },
        "body": "h1. Smoke\n\nUse *self-service*.",
        "created": "2026-07-08T12:00:00.000+0000"
    }"#;

    #[test]
    fn parses_myself_for_probe() {
        let (display, account) = parse_myself_response(MYSELF_FIXTURE).unwrap();
        assert_eq!(display, "Advisor User");
        assert_eq!(account, "abc-123");
    }

    #[test]
    fn parses_server_info_for_probe() {
        let (title, ver) = parse_server_info_response(SERVER_INFO_FIXTURE).unwrap();
        assert_eq!(title.as_deref(), Some("canonic-smoke"));
        assert_eq!(ver.as_deref(), Some("9.12.15"));
    }

    #[test]
    fn format_probe_mentions_free_rest() {
        let text = format_probe(&JiraProbe {
            base_url: "https://example.atlassian.net".into(),
            display_name: "Advisor User".into(),
            account: "abc-123".into(),
            server_title: Some("canonic-smoke".into()),
            server_version: Some("9.12.15".into()),
        });
        assert!(text.contains("free REST"));
        assert!(text.contains("Advisor User"));
        assert!(text.contains("Marketplace") || text.contains("no Marketplace"));
        assert!(text.contains("https://example.atlassian.net"));
    }

    #[test]
    fn myself_request_targets_v2_myself() {
        let client = reqwest::blocking::Client::new();
        let cfg = JiraConfig::new(
            "https://example.atlassian.net/",
            JiraAuth::Basic {
                user: "a@b.c".into(),
                token: "t".into(),
            },
        );
        let req = build_myself_request(&client, &cfg).unwrap();
        assert_eq!(req.url().path(), "/rest/api/2/myself");
        assert!(req.headers().contains_key(reqwest::header::AUTHORIZATION));
    }

    #[test]
    fn comment_post_request_is_post_with_json_body() {
        let client = reqwest::blocking::Client::new();
        let cfg = JiraConfig::new(
            "https://jira.example.org",
            JiraAuth::Header("Bearer pat".into()),
        );
        let wiki = "h1. Smoke\n\nBody *bold*.";
        let req = build_comment_post_request(&client, &cfg, "HSP-101", wiki, CommentBodyFormat::Wiki).unwrap();
        assert_eq!(req.method(), reqwest::Method::POST);
        assert_eq!(req.url().path(), "/rest/api/2/issue/HSP-101/comment");
        let body = String::from_utf8_lossy(req.body().unwrap().as_bytes().unwrap());
        assert!(body.contains("body"), "{body}");
        assert!(body.contains("Smoke") || body.contains("h1"), "{body}");
        assert_eq!(
            req.headers().get(reqwest::header::AUTHORIZATION).unwrap(),
            "Bearer pat"
        );
    }

    #[test]
    fn parses_comment_create_response() {
        let (id, body) = parse_comment_create_response(COMMENT_CREATE_FIXTURE).unwrap();
        assert_eq!(id, "30001");
        let body = body.unwrap_or_default();
        assert!(body.contains("self-service") || body.contains("h1"), "{body}");
    }

    #[test]
    fn post_comment_from_markdown_dry_run_uses_pandoc_convert() {
        if !crate::convert::tool_available() {
            return;
        }
        let base = std::env::var_os("CARGO_TARGET_TMPDIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/test-tmp"));
        let _ = std::fs::create_dir_all(&base);
        let md = base.join("resp-smoke-jira-comment.md");
        std::fs::write(
            &md,
            "---\nid: resp-smoke\ntitle: Smoke\nprefix: resp\nsop: none\n---\n\n# Smoke\n\nUse *self-service* for backups.\n\nRegards,\nSupport Team\n",
        )
        .expect("write smoke markdown under target/");
        let cfg = JiraConfig::new(
            "http://127.0.0.1:9",
            JiraAuth::Basic {
                user: "x".into(),
                token: "y".into(),
            },
        );
        let posted = post_comment_from_markdown(&cfg, "HSP-101", &md, true).expect("dry-run");
        assert_eq!(posted.comment_id, "(dry-run)");
        assert_eq!(posted.issue_key, "HSP-101");
        let via_convert = crate::convert::convert_path_to_jira(&md).expect("convert");
        assert_eq!(posted.body_wiki, via_convert);
        assert!(
            posted.body_wiki.contains("self-service")
                || posted.body_wiki.contains("Smoke")
                || posted.body_wiki.contains("h1"),
            "unexpected wiki: {:?}",
            posted.body_wiki
        );
        let _ = std::fs::remove_file(&md);
    }

    #[test]
    fn cloud_host_detection() {
        assert!(is_cloud_host("https://acme.atlassian.net"));
        assert!(!is_cloud_host("https://jira.example.org"));
        assert_eq!(
            CommentBodyFormat::Auto.resolve("https://x.atlassian.net"),
            CommentBodyFormat::Adf
        );
        assert_eq!(
            CommentBodyFormat::Auto.resolve("http://localhost:8080"),
            CommentBodyFormat::Wiki
        );
    }

    #[test]
    fn plain_text_to_adf_is_minimal_doc() {
        let adf = plain_text_to_adf("line one\n\nline two");
        assert_eq!(adf["type"], "doc");
        assert_eq!(adf["version"], 1);
        assert!(adf["content"].as_array().unwrap().len() >= 1);
        let s = adf.to_string();
        assert!(s.contains("line one"));
        assert!(s.contains("line two"));
    }

    #[test]
    fn comment_body_to_text_reads_wiki_string_and_adf() {
        let wiki = serde_json::json!("h1. Hello");
        assert_eq!(comment_body_to_text(&wiki), "h1. Hello");
        let adf = serde_json::json!({
            "type": "doc",
            "version": 1,
            "content": [{
                "type": "paragraph",
                "content": [{"type": "text", "text": "Hello ADF"}]
            }]
        });
        assert!(comment_body_to_text(&adf).contains("Hello ADF"));
    }

    #[test]
    fn cloud_comment_post_uses_api3_and_adf() {
        let client = reqwest::blocking::Client::new();
        let cfg = JiraConfig::new(
            "https://acme.atlassian.net",
            JiraAuth::Basic {
                user: "a@b.c".into(),
                token: "t".into(),
            },
        );
        let req = build_comment_post_request(
            &client,
            &cfg,
            "HSP-1",
            "hello *world*",
            CommentBodyFormat::Auto,
        )
        .unwrap();
        assert_eq!(req.url().path(), "/rest/api/3/issue/HSP-1/comment");
        let body = String::from_utf8_lossy(req.body().unwrap().as_bytes().unwrap());
        assert!(body.contains("\"type\":\"doc\"") || body.contains("\"doc\""), "{body}");
        assert!(body.contains("hello") || body.contains("world"), "{body}");
    }
}
