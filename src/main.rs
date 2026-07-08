//! canonic — versioned Jira canned-response corpus CLI.

use anyhow::{bail, Context, Result};
use canonic::check::{check_corpus, format_check_report};
use canonic::convert::{convert_path_to_jira, tool_available as pandoc_available};
use canonic::corpus::{default_corpus_dir, walk_responses};
use canonic::doctor::{collect_statuses, critical_missing, format_doctor};
use canonic::index::{default_index_dir, find_duplicates, reindex, search};
use canonic::jira_import::{
    default_import_dir, format_probe, import_jira, post_comment_from_markdown_with_format,
    probe_jira, CommentBodyFormat, JiraConfig,
};
use canonic::lint::{format_report, lint_paths, LintEngine};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(
    name = "canonic",
    version,
    about = "Versioned Jira canned responses: convert, quality check, Tantivy search/dedupe"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Report presence of pandoc, Vale, Harper CLI, and in-process harper-core
    Doctor,
    /// List canned responses in the corpus
    List {
        #[arg(long)]
        corpus: Option<PathBuf>,
    },
    /// Quality gate: resp- ids, prefix/sop front matter, personal sign-offs
    Check {
        #[arg(long)]
        corpus: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Convert markdown to Jira/Confluence wiki markup via pandoc
    Convert {
        path: Option<PathBuf>,
        #[arg(long)]
        corpus: Option<PathBuf>,
        #[arg(long)]
        write: bool,
    },
    /// Lint the corpus with Vale and/or Harper (harper-core in-process)
    Lint {
        #[arg(long)]
        corpus: Option<PathBuf>,
        path: Option<PathBuf>,
        #[arg(long, value_enum, default_value_t = LintEngine::All)]
        engine: LintEngine,
        #[arg(long)]
        json: bool,
    },
    /// Rebuild the Tantivy index from the markdown corpus
    Reindex {
        #[arg(long)]
        corpus: Option<PathBuf>,
        #[arg(long)]
        index: Option<PathBuf>,
    },
    /// BM25 full-text search over the indexed corpus
    Search {
        query: String,
        #[arg(long, short = 'n', default_value_t = 10)]
        limit: usize,
        #[arg(long)]
        index: Option<PathBuf>,
    },
    /// Near-duplicate detection (Tantivy self-query + jaccard in reason)
    Dedupe {
        #[arg(long)]
        corpus: Option<PathBuf>,
        #[arg(long)]
        index: Option<PathBuf>,
        /// Minimum Tantivy score to report a pair (tune after reindex)
        #[arg(long, default_value_t = 1.0)]
        threshold: f64,
        #[arg(long, default_value_t = 8)]
        per_doc: usize,
        /// Rebuild index before scanning
        #[arg(long)]
        reindex: bool,
        #[arg(long)]
        json: bool,
    },
    /// Import existing Jira issue comments as review drafts (read path only; never writes corpus/responses)
    ImportJira {
        /// JQL selecting candidate issues, e.g. `project = HSP AND labels = canned-response`
        jql: String,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long, default_value_t = 50)]
        max_results: u32,
        /// List issues that would be imported without fetching comments or writing files
        #[arg(long)]
        dry_run: bool,
    },
    /// Probe free Jira REST connectivity and identity (`/rest/api/2/myself`; no Marketplace apps)
    JiraProbe,
    /// Post a pandoc-jira comment on an issue (free REST only; explicit, not bulk library sync)
    JiraComment {
        /// Issue key, e.g. HSP-101
        #[arg(long)]
        issue: String,
        /// Markdown file to convert and post (pandoc jira writer)
        path: PathBuf,
        /// Print converted wiki markup without POSTing
        #[arg(long)]
        dry_run: bool,
        /// Comment body encoding for free platform REST: auto (Cloud→ADF, Server→wiki), wiki, adf
        #[arg(long, value_enum, default_value_t = CommentBodyCli::Auto)]
        body_format: CommentBodyCli,
    },
}

/// CLI mirror of free-tier [`CommentBodyFormat`] (Cloud Free ADF vs Server wiki).
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum, Default)]
enum CommentBodyCli {
    /// `*.atlassian.net` → ADF on `/rest/api/3`, else wiki on `/rest/api/2`
    #[default]
    Auto,
    /// Server/DC wiki string body on API v2
    Wiki,
    /// Cloud Free ADF body on API v3 (no Marketplace apps)
    Adf,
}

impl From<CommentBodyCli> for CommentBodyFormat {
    fn from(v: CommentBodyCli) -> Self {
        match v {
            CommentBodyCli::Auto => CommentBodyFormat::Auto,
            CommentBodyCli::Wiki => CommentBodyFormat::Wiki,
            CommentBodyCli::Adf => CommentBodyFormat::Adf,
        }
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<ExitCode> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Doctor => {
            let statuses = collect_statuses();
            print!("{}", format_doctor(&statuses));
            if critical_missing(&statuses).is_empty() {
                Ok(ExitCode::SUCCESS)
            } else {
                Ok(ExitCode::from(1))
            }
        }
        Commands::List { corpus } => {
            let corpus = corpus.unwrap_or_else(default_corpus_dir);
            let docs = walk_responses(&corpus)?;
            if docs.is_empty() {
                println!("(no responses in {})", corpus.display());
            }
            for d in docs {
                let sop = d.sop.as_deref().unwrap_or("-");
                println!(
                    "{}\t{}\tsop={}\t{}",
                    d.id,
                    d.title,
                    sop,
                    d.path.display()
                );
            }
            Ok(ExitCode::SUCCESS)
        }
        Commands::Check { corpus, json } => {
            let corpus = corpus.unwrap_or_else(default_corpus_dir);
            let report = check_corpus(&corpus)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print!("{}", format_check_report(&report));
            }
            if report.ok() {
                Ok(ExitCode::SUCCESS)
            } else {
                Ok(ExitCode::from(1))
            }
        }
        Commands::Convert {
            path,
            corpus,
            write,
        } => {
            if !pandoc_available() {
                bail!(
                    "pandoc is not installed or not on PATH; install pandoc to convert markdown to jira markup"
                );
            }
            let paths: Vec<PathBuf> = if let Some(p) = path {
                vec![p]
            } else {
                let corpus = corpus.unwrap_or_else(default_corpus_dir);
                walk_responses(&corpus)?
                    .into_iter()
                    .map(|r| r.path)
                    .collect()
            };
            if paths.is_empty() {
                bail!("no markdown files to convert");
            }
            let multi = paths.len() > 1;
            for p in &paths {
                let jira = convert_path_to_jira(p)?;
                if write {
                    let out = p.with_extension("jira.txt");
                    std::fs::write(&out, &jira)
                        .with_context(|| format!("write {}", out.display()))?;
                    eprintln!("wrote {}", out.display());
                } else {
                    if multi {
                        println!("--- {} ---", p.display());
                    }
                    print!("{jira}");
                    if !jira.ends_with('\n') {
                        println!();
                    }
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        Commands::Lint {
            corpus,
            path,
            engine,
            json,
        } => {
            let paths: Vec<PathBuf> = if let Some(p) = path {
                vec![p]
            } else {
                let corpus = corpus.unwrap_or_else(default_corpus_dir);
                walk_responses(&corpus)?
                    .into_iter()
                    .map(|r| r.path)
                    .collect()
            };
            let report = lint_paths(&paths, engine)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                print!("{}", format_report(&report));
            }
            if !report.findings.is_empty() {
                Ok(ExitCode::from(1))
            } else {
                Ok(ExitCode::SUCCESS)
            }
        }
        Commands::Reindex { corpus, index } => {
            let corpus = corpus.unwrap_or_else(default_corpus_dir);
            let index = index.unwrap_or_else(default_index_dir);
            let n = reindex(&corpus, &index)?;
            println!("reindexed {n} document(s) into {}", index.display());
            Ok(ExitCode::SUCCESS)
        }
        Commands::Search {
            query,
            limit,
            index,
        } => {
            let index = index.unwrap_or_else(default_index_dir);
            if !index.exists() {
                bail!(
                    "index not found at {}; run `canonic reindex` first",
                    index.display()
                );
            }
            let hits = search(&index, &query, limit)?;
            if hits.is_empty() {
                println!("(no hits for {query:?})");
            }
            for (i, h) in hits.iter().enumerate() {
                println!(
                    "{}. {}  score={:.4}  {}\n   {}\n   {}",
                    i + 1,
                    h.id,
                    h.score,
                    h.title,
                    h.path.display(),
                    h.snippet
                );
            }
            Ok(ExitCode::SUCCESS)
        }
        Commands::Dedupe {
            corpus,
            index,
            threshold,
            per_doc,
            reindex: do_reindex,
            json,
        } => {
            let corpus = corpus.unwrap_or_else(default_corpus_dir);
            let index = index.unwrap_or_else(default_index_dir);
            if do_reindex || !index.exists() {
                let n = reindex(&corpus, &index)?;
                eprintln!("reindexed {n} document(s) into {}", index.display());
            }
            let pairs = find_duplicates(&index, &corpus, threshold, per_doc)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&pairs)?);
            } else if pairs.is_empty() {
                println!(
                    "(no near-duplicate pairs above threshold {threshold}; try lowering --threshold)"
                );
            } else {
                for (i, p) in pairs.iter().enumerate() {
                    println!(
                        "{}. {} ↔ {}  score={:.4}\n   {}\n   {}\n   {}",
                        i + 1,
                        p.left_id,
                        p.right_id,
                        p.score,
                        p.left_path,
                        p.right_path,
                        p.reason
                    );
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        Commands::ImportJira {
            jql,
            out,
            max_results,
            dry_run,
        } => {
            let cfg = JiraConfig::from_env()?;
            let out_dir = out.unwrap_or_else(default_import_dir);
            let paths = import_jira(&cfg, &jql, &out_dir, max_results, dry_run)?;
            let verb = if dry_run { "would import" } else { "imported" };
            println!("{verb} {} issue(s) into {}:", paths.len(), out_dir.display());
            for p in &paths {
                println!("  {}", p.display());
            }
            Ok(ExitCode::SUCCESS)
        }
        Commands::JiraProbe => {
            let cfg = JiraConfig::from_env()?;
            let probe = probe_jira(&cfg)?;
            print!("{}", format_probe(&probe));
            Ok(ExitCode::SUCCESS)
        }
        Commands::JiraComment {
            issue,
            path,
            dry_run,
            body_format,
        } => {
            let cfg = JiraConfig::from_env()?;
            let format = CommentBodyFormat::from(body_format);
            let posted = post_comment_from_markdown_with_format(
                &cfg, &issue, &path, dry_run, format,
            )?;
            let resolved = format.resolve(&cfg.base_url);
            if dry_run {
                println!(
                    "would post comment on {issue} (format={resolved:?}, {} bytes from {}):",
                    posted.body_wiki.len(),
                    path.display()
                );
                print!("{}", posted.body_wiki);
                if !posted.body_wiki.ends_with('\n') {
                    println!();
                }
            } else {
                println!(
                    "posted comment {} on {} (format={resolved:?}, {} bytes from {})",
                    posted.comment_id,
                    posted.issue_key,
                    posted.body_wiki.len(),
                    path.display()
                );
            }
            Ok(ExitCode::SUCCESS)
        }
    }
}
