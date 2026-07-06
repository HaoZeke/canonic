//! canonic — versioned Jira canned-response corpus CLI.

use anyhow::{bail, Context, Result};
use canonic::convert::{convert_path_to_jira, tool_available as pandoc_available};
use canonic::corpus::{default_corpus_dir, walk_responses};
use canonic::doctor::{collect_statuses, critical_missing, format_doctor};
use canonic::index::{default_index_dir, reindex, search};
use canonic::lint::{format_report, lint_paths, LintEngine};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(
    name = "canonic",
    version,
    about = "Versioned markdown canned responses for Jira: convert, lint, Heed+BM25 search"
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
        /// Corpus directory (default: corpus/responses)
        #[arg(long)]
        corpus: Option<PathBuf>,
    },
    /// Convert markdown to Jira/Confluence wiki markup via pandoc
    Convert {
        /// Path to a markdown file (or omit to convert all under --corpus)
        path: Option<PathBuf>,
        /// Corpus directory when converting all
        #[arg(long)]
        corpus: Option<PathBuf>,
        /// Write output next to source as `.jira.txt` instead of stdout
        #[arg(long)]
        write: bool,
    },
    /// Lint the corpus with Vale and/or Harper (harper-core in-process)
    Lint {
        /// Corpus directory (default: corpus/responses)
        #[arg(long)]
        corpus: Option<PathBuf>,
        /// Optional single file instead of whole corpus
        path: Option<PathBuf>,
        /// Which engine(s) to run
        #[arg(long, value_enum, default_value_t = LintEngine::All)]
        engine: LintEngine,
        /// Emit JSON report
        #[arg(long)]
        json: bool,
    },
    /// Rebuild the Heed BM25 index from the markdown corpus
    Reindex {
        /// Corpus directory (default: corpus/responses)
        #[arg(long)]
        corpus: Option<PathBuf>,
        /// Index directory (default: .canonic-index)
        #[arg(long)]
        index: Option<PathBuf>,
    },
    /// BM25 search over the indexed corpus
    Search {
        /// Query string
        query: String,
        /// Max hits
        #[arg(long, short = 'n', default_value_t = 10)]
        limit: usize,
        /// Index directory (default: .canonic-index)
        #[arg(long)]
        index: Option<PathBuf>,
    },
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
                println!("{}\t{}\t{}", d.id, d.title, d.path.display());
            }
            Ok(ExitCode::SUCCESS)
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
    }
}
