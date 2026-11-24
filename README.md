# TusmoResolver

A Rust bot that plays the **Infinite** (endless) mode of [Tusmo](https://www.tusmo.xyz) on its own. It opens a real Chromium window, reads the grid from the page, picks the best word with a constraint solver, types it, and moves on to the next game, forever.

Available in **French** (default) and **English**.

> **Educational project.** Automating an online game may go against the site's terms of use. Do not use it in ranked or competitive modes, and use it at your own risk.

## Features

- Drives a real Chromium window through the DevTools Protocol ([`chromiumoxide`](https://crates.io/crates/chromiumoxide))
- Reads the grid straight from the DOM (letters, positions, colors), no screenshots or OCR
- Constraint solver that handles duplicate letters correctly
- Smart word choice: picks the word that splits the remaining candidates the best, even if it is not a candidate itself
- Multi-core scoring with `rayon`: the first move takes a few milliseconds even with thousands of candidates
- Detects words refused by Tusmo, bans them, and never loses an attempt because of them
- Closes popups automatically (welcome message, news, boosters...) with an in-page watcher, so they never block a game
- Plays game after game without any interaction
- French and English modes with `-l`

## Requirements

- [Rust](https://rustup.rs) (stable)
- Chrome or Chromium installed (detected automatically)

## Usage

```bash
# French (default)
cargo run --release

# English
cargo run --release -- -l EN
```

Always use `--release`: the solver is much slower in debug builds.

| Option | Description | Default |
|---|---|---|
| `-l`, `--lang <FR\|EN>` | Game language (case insensitive) | `FR` |

Stop the bot with `Ctrl+C`.

To build a standalone executable:

```bash
cargo build --release
# target/release/TusmoResolver (.exe on Windows)
```

```bash
TusmoResolver -l EN
```

## Word lists

The word lists are included in the repository, nothing to download:

```
words/
  fr.txt                 French word list (ODS9)
  en.txt                 English word list
  banned_words_fr.txt    words refused by Tusmo, updated at runtime
  banned_words_en.txt    words refused by Tusmo, updated at runtime
```

Accents and case do not matter: words are uppercased, accents are removed, and only words with the right length and first letter are kept.

You can replace them with any list, one word per line.

Tusmo also accepts words that are not in Scrabble dictionaries (brand names, some proper nouns). If the secret word is missing from the list, the bot plays filler words until the game ends. Add such words to the list to fix it.

If a `banned_words_*.txt` file does not exist, it is created the first time a word is banned.

## How it works

```
Chromium (CDP)  ->  read grid  ->  solver picks a word  ->  type it  ->  repeat
```

1. **Browser control** (`src/browser.rs`): the bot opens Tusmo, sets the language, clicks the endless mode tile, then reads and writes the page directly.
2. **Language**: the game language is switched through the page's `localStorage`. Everything else that depends on the language (URL, tile label, toast text, files) lives in `src/lang.rs`.
3. **Grid reading**: a JavaScript snippet runs inside the page and returns every cell as `(position, letter, state)`. The position comes from the `--i` CSS variable, the state from the cell classes (`correct`, `present`, `absent`). A new row starts every time `--i == 0`.
4. **Solver** (`src/resolver.rs`): pure synchronous Rust, no async, unit tested.
5. **Game loop**: type the word, wait for the row to be revealed, check for a win, wait for the next grid, start again.
6. **Popup killer**: right after the page loads, a `MutationObserver` is injected into the page. It watches every change in the DOM and clicks the dismiss button of known popups as soon as they appear, matched by button text or CSS selector.

## The solver

A candidate word is kept only if, **assuming it is the secret word**, it would have produced exactly the feedback Tusmo showed for every row played so far. The feedback is simulated by `compute_feedback`, which handles duplicate letters with two passes: exact matches first, then misplaced letters, each secret letter matched only once.

To choose the next word, `best_guess` scores each possible word by how well it splits the remaining candidates:

- every candidate is grouped by the feedback pattern the guess would produce
- the score is the sum of squared group sizes (lower is better)
- when few candidates remain, **probe words** that are not candidates are also considered, so that look-alike words (`RAYAT`, `RAPAT`, `RAMAT`, `RALAT`, `RABAT`) are told apart in one move instead of being tried one by one
- a candidate gets a tiny bonus because it can win immediately

The feedback is encoded as a base-3 number with no allocation, and the scoring runs on all CPU cores with `rayon`.

## Project structure

```
src/
  main.rs        entry point, CLI arguments, tokio runtime
  lang.rs        language configuration (URL, labels, files)
  browser.rs     Chromium control, grid reading, game loop
  resolver.rs    feedback simulation, filtering, best word selection
words/           word lists (included)
```

## Tests

```bash
cargo test
```

The tests cover the feedback computation (including duplicate letters), candidate filtering, and the choice of a splitting word.

## Dependencies

`chromiumoxide`, `tokio`, `futures`, `serde`, `anyhow`, `deunicode`, `clap`, `rayon`

## Design choices

- **Async only at the edges.** Everything that talks to Chrome is `async`. The solver is plain synchronous code, which makes it easy to test and reuse.
- **Read the DOM, not pixels.** The grid state is read from CSS classes, so it is exact and fast.
- **Simulation instead of rules.** Rather than hand-writing green/yellow/grey rules, the solver replays the game for each candidate. Duplicate letters need no special case.
- **Language as data.** Everything that changes between French and English lives in one config struct, so adding a language is one more `match` arm.

## Known limitations

- Tusmo waits about 3 to 4 seconds between two games. This delay comes from the site, the bot polls every 20 ms and cannot shorten it.
- Words accepted by Tusmo but missing from the list cost attempts until they are added.
- The CSS selectors (`.cell`, `.tile__label`, `.toast`) depend on the current Tusmo front end and may break if the site changes.
- A new kind of popup is only closed once its button text or CSS selector is added to the popup killer (`install_popup_killer`).

## Possible improvements

- Tell a real loss (the solver ran out of moves) from a missing word (the secret word is not in the list), and handle each case differently
- Read the secret word on the end-of-game screen and add it to the list automatically
- Click "Play again" on the end-of-game screen instead of waiting
- Rank words by frequency so common words are tried first
- Add a `--games <N>` option to stop after N games
- Prebuilt binaries for Windows, macOS and Linux through GitHub Actions

## Disclaimer

This project is not affiliated with Tusmo. It is a personal learning project about browser automation and constraint solving in Rust.