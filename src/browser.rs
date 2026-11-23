use std::collections::HashSet;

use crate::lang::Lang;
use crate::resolver::{Row, State, Tile};
use crate::{load_banned, load_word_list, save_banned};
use chromiumoxide::{Browser, BrowserConfig, Page};
use futures::StreamExt;
use serde::Deserialize;



// Popups and elements that may appear on the page:
// <span class="tile__label">Infini</span>
// <button class="lx-news__later">Plus tard</button>
// <button class="btn btn--red w-full">C'est parti !</button>

pub async fn open_browser(lang: Lang) -> anyhow::Result<()> {
    let cfg = lang.config();
    let (browser, mut handler) = Browser::launch(
        BrowserConfig::builder()
            .with_head()
            .build()
            .map_err(|e| anyhow::anyhow!(e))?,
    )
    .await?;
    let words = load_word_list(&cfg)?;
    // Poll the browser events in the background, stop cleanly when the connection closes
    let _handle = tokio::spawn(async move { while let Some(_event) = handler.next().await {} });

    let page = browser.new_page(cfg.url).await?;
    install_popup_killer(&page).await?;
    println!("Clicking on tile '{}'", cfg.mode_label);
    page.find_xpath(&format!(
        // Hardcoded XPath to find the tile with the correct label, in French or English, I will make it based on the config later.
        "//span[@class='tile__label' and normalize-space(text())='Infini' or normalize-space(text())='Infinite']",
        
    ))
    .await?
    .click()
    .await?;

    // Wait until the grid is displayed
    wait_until(
        &page,
        "document.querySelectorAll('.cell').length > 0",
        15000,
    )
    .await?;

    // Words refused by Tusmo, kept across games and restarts
    let mut banned: HashSet<String> = load_banned(&cfg.banned_file);
    println!("{} banned words loaded", banned.len());

    // One iteration = one game
    loop {
        // Reset everything for each new game
        let (len, first) = read_current_row(&page).await?;

        let mut candidates: Vec<String> = crate::resolver::load_words(&words, len, first)
            .into_iter()
            .filter(|w| !banned.contains(w))
            .collect();
        // Probe words: same length and same first letter (Tusmo locks the first letter)
        let mut all_words: Vec<String> = candidates.clone();

        let mut solved = false;
        // Safety cap only: rejected words do not use real attempts
        let mut tries = 0;

        loop {
            tries += 1;
            if tries > 40 {
                println!("Too many rejected words, skipping this game");
                break;
            }

            let history = read_rows(&page).await?;
            // Real attempts = rows revealed on the site
            if history.len() >= 6 {
                break;
            }

            candidates = crate::resolver::filter_candidates(&candidates, &history);
            let t = std::time::Instant::now();
            let Some(guess) = crate::resolver::best_guess(&candidates, &all_words) else {
                println!("No candidate left, skipping this game");
                break;
            };
            println!("best_guess took {:?}", t.elapsed());
            println!("{} candidates left, playing {}", candidates.len(), guess);

            type_and_submit(&page, &guess).await?;

            let revealed = match wait_for_row_reveal(&page, history.len(), &cfg.rejected_toast).await {
                Ok(r) => r,
                Err(e) => {
                    // Not a confirmed rejection: clear the row and retry without banning
                    println!("{}", e);
                    clear_row(&page).await?;
                    continue;
                }
            };

            if !revealed {
                // Word refused by Tusmo ("Mot inconnu" toast): ban it and retry
                println!("{} rejected", guess);
                if banned.insert(guess.clone()) {
                    save_banned(&cfg.banned_file, &guess)?;
                }
                candidates.retain(|w| w != &guess);
                all_words.retain(|w| w != &guess);
                clear_row(&page).await?;
                // Wait until the toast disappears so it is not read again
                // the toast may be in French or English, depending on the game language, I will make it based on the config but rn it is hardcoded to French and English
                wait_until(
                    &page,
                    "!Array.from(document.querySelectorAll('.toast')).some(t => t.innerText.trim().toLowerCase() === 'mot inconnu') && !Array.from(document.querySelectorAll('.toast')).some(t => t.innerText.trim().toLowerCase() === 'unknown word')",
                    5000,
                )
                .await?;
                continue;
            }

            if is_solved(&page).await? {
                println!("Solved: {}", guess);
                solved = true;
                break;
            }
        }

        if !solved {
            // Log the lost game to find out why it failed
            let history = read_rows(&page).await?;
            let played: Vec<String> = history
                .iter()
                .map(|row| row.iter().map(|t| t.letter).collect())
                .collect();
            println!(
                "LOST: played {:?}, {} candidates left: {:?}",
                played,
                candidates.len(),
                candidates.iter().take(10).collect::<Vec<_>>()
            );
        }

        let t = std::time::Instant::now();
        wait_for_new_game(&page).await?;
        println!("Next game ready after {:?}", t.elapsed());
    }
}

// Poll a JS condition every 20 ms and return as soon as it is true
async fn wait_until(page: &Page, js: &str, timeout_ms: u128) -> anyhow::Result<()> {
    let start = std::time::Instant::now();
    while start.elapsed().as_millis() < timeout_ms {
        let ok: bool = page.evaluate(js).await?.into_value()?;
        if ok {
            return Ok(());
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    anyhow::bail!("Timeout waiting for: {}", js)
}

// Type a word with the keyboard and validate it with Enter
pub async fn type_and_submit(page: &Page, text: &str) -> anyhow::Result<()> {
    let body = page.find_element("body").await?;
    for ch in text.chars() {
        body.press_key(&ch.to_string()).await?;
    }
    body.press_key("Enter").await?;
    Ok(())
}

// Remove every typed letter from the current row
async fn clear_row(page: &Page) -> anyhow::Result<()> {
    let body = page.find_element("body").await?;
    // Extra presses do nothing, so we send more than needed
    for _ in 0..12 {
        body.press_key("Backspace").await?;
    }
    Ok(())
}

#[derive(Deserialize)]
struct RawTile {
    i: usize,
    letter: String,
    state: String,
}

// Read every fully revealed row of the grid
pub async fn read_rows(page: &Page) -> anyhow::Result<Vec<Row>> {
    let js = r#"
    Array.from(document.querySelectorAll('.cell')).map(cell => {
        // Position of the letter in the word, read from the inline style
        const i = parseInt(cell.style.getPropertyValue('--i'), 10);
        const letter = cell.querySelector('.cell__letter')?.innerText.trim() ?? '';
        let state = 'empty';
        if (cell.classList.contains('cell--absent')) state = 'absent';
        else if (cell.querySelector('.cell__mark--correct')) state = 'correct';
        else if (cell.querySelector('.cell__mark--present')) state = 'present';
        else if (cell.classList.contains('cell--dot')) state = letter === '.' ? 'dot' : 'typed';
        return { i, letter, state };
    })
    "#;
    let raw: Vec<RawTile> = page.evaluate(js).await?.into_value()?;

    // Split the flat list into rows: a new row starts every time i == 0
    let mut rows: Vec<Vec<RawTile>> = Vec::new();
    for tile in raw {
        if tile.i == 0 {
            rows.push(Vec::new());
        }
        if let Some(row) = rows.last_mut() {
            row.push(tile);
        }
    }

    // Keep only fully revealed rows (every cell is correct / present / absent)
    let history: Vec<Row> = rows
        .iter()
        .filter(|row| {
            row.iter()
                .all(|t| matches!(t.state.as_str(), "correct" | "present" | "absent"))
        })
        .map(|row| {
            row.iter()
                .map(|t| Tile {
                    letter: t.letter.chars().next().unwrap_or('?').to_ascii_uppercase(),
                    state: match t.state.as_str() {
                        "correct" => State::Correct,
                        "present" => State::Present,
                        _ => State::Absent,
                    },
                })
                .collect()
        })
        .collect();
    Ok(history)
}

// Returns (word length, known first letter) from the current unrevealed row
pub async fn read_current_row(page: &Page) -> anyhow::Result<(usize, Option<char>)> {
    let js = r#"
    (() => {
        const cells = Array.from(document.querySelectorAll('.cell'));

        // Split the flat list of cells into rows: a new row starts at --i == 0
        const rows = [];
        cells.forEach(c => {
            const i = parseInt(c.style.getPropertyValue('--i'), 10);
            if (i === 0) rows.push([]);
            if (rows.length) rows[rows.length - 1].push(c);
        });

        // A cell is played when it is revealed (correct / present) or absent
        const isPlayed = c =>
            c.classList.contains('cell--revealed') || c.classList.contains('cell--absent');

        // Current row = first row that is not fully played
        const current = rows.find(r => !r.every(isPlayed)) ?? [];
        const first = current[0]?.querySelector('.cell__letter')?.innerText.trim() ?? '';
        return { len: current.length, first };
    })()
    "#;
    #[derive(Deserialize)]
    struct Current {
        len: usize,
        first: String,
    }
    let c: Current = page.evaluate(js).await?.into_value()?;
    println!("New game: length = {}, first letter = {:?}", c.len, c.first);
    Ok((
        c.len,
        c.first.chars().next().map(|ch| ch.to_ascii_uppercase()),
    ))
}

// Install an in-page watcher that automatically dismisses popups
pub async fn install_popup_killer(page: &Page) -> anyhow::Result<()> {
    let js = r#"
        (() => {
            // Avoid installing the watcher twice
            if (window.__popupKiller) return;
            window.__popupKiller = true;

            // Button texts that dismiss a popup (lowercase)
           const LABELS = ["plus tard", "c'est parti !", "fermer", "non merci", "ignorer", "passer",
                            "later", "let's go!", "close", "no thanks", "skip"];
            // CSS selectors of known dismiss buttons
            const SELECTORS = [".lx-news__later"];

            // Names of the popups closed since the last read from Rust
            window.__killedPopups = [];
            // Elements already reported, so a popup is logged once even if it is clicked several times
            const seen = new WeakSet();

            const kill = (el, name) => {
            if (!seen.has(el)) {
                seen.add(el);
                window.__killedPopups.push(name);
            }
            el.click();
        };

        const sweep = () => {
            SELECTORS.forEach(sel =>
            document.querySelectorAll(sel).forEach(el =>
             kill(el, sel + ' | ' + el.innerText.trim())
                )
            );
            document.querySelectorAll('button').forEach(b => {
                const text = b.innerText.trim();
                if (LABELS.includes(text.toLowerCase())) kill(b, text);
            });
        };

            // Run on every DOM change, and once right now
            new MutationObserver(sweep).observe(document.body, { childList: true, subtree: true });
            sweep();
        })()
    "#;
    page.evaluate(js).await?;
    Ok(())
}

// Returns true if the last revealed row is fully correct
async fn is_solved(page: &Page) -> anyhow::Result<bool> {
    let history = read_rows(page).await?;
    Ok(history
        .last()
        .map_or(false, |r| r.iter().all(|t| t.state == State::Correct)))
}

// Wait until a new game starts: grid displayed and no revealed row left
async fn wait_for_new_game(page: &Page) -> anyhow::Result<()> {
    let js = "document.querySelectorAll('.cell').length > 0 && \
              document.querySelectorAll('.cell--revealed, .cell--absent').length === 0";
    if wait_until(page, js, 30000).await.is_err() {
        // Do not stop the bot: just continue
        println!("Timeout waiting for a new game, continuing");
    }
    Ok(())
}

async fn is_word_rejected(page: &Page, toast: &str) -> anyhow::Result<bool> {
    let js = format!(
        "Array.from(document.querySelectorAll('.toast')).some(t => \
            t.innerText.trim().toLowerCase() === '{}')",
        toast.to_lowercase()
    );
    Ok(page.evaluate(js).await?.into_value()?)
}

// Returns true if the row was revealed, false if the word was rejected.
// Returns an error on timeout, so the caller does not ban a valid word.
async fn wait_for_row_reveal(
    page: &Page,
    previous_len: usize,
    toast: &str,
) -> anyhow::Result<bool> {
    // About 6 seconds of timeout
    for _ in 0..200 {
        if is_word_rejected(page, toast).await? {
            return Ok(false);
        }
        if read_rows(page).await?.len() > previous_len {
            return Ok(true);
        }
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;
    }
    anyhow::bail!("Timeout: no toast and no new row")
}
