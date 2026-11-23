mod browser;
mod lang;
mod resolver;
use std::collections::HashSet;

use clap::Parser;

use crate::lang::{Lang, LangConfig};

#[derive(Parser)]
struct Args {
    /// Game language: FR (default) or EN
    #[arg(
        short = 'l',
        long,
        value_enum,
        ignore_case = true,
        default_value = "fr"
    )]
    lang: Lang,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    browser::open_browser(args.lang).await?;
    Ok(())
}

fn load_word_list(cfg: &LangConfig) -> anyhow::Result<String> {
    let main = std::fs::read_to_string(cfg.words_file).map_err(|e| {
        anyhow::anyhow!(
            "Missing {}, see the README to get a word list ({e})",
            cfg.words_file
        )
    })?;
    // The extra file is optional
    println!("file loaded: {}", cfg.words_file);
    let extra = std::fs::read_to_string(cfg.extra_file).unwrap_or_default();
    Ok(format!("{}\n{}", main, extra))
}

fn load_banned(path: &str) -> HashSet<String> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(|l| l.trim().to_uppercase())
        .filter(|l| !l.is_empty())
        .collect()
}

fn save_banned(path: &str, word: &str) -> anyhow::Result<()> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(file, "{}", word)?;
    Ok(())
}
