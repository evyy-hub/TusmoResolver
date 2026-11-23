use rayon::prelude::*;
use std::collections::HashMap;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum State {
    Correct,
    Present,
    Absent,
}

#[derive(Debug, Clone)]
pub struct Tile {
    pub letter: char,
    pub state: State,
}

// One row = one guess
pub type Row = Vec<Tile>;

// Simulate the feedback Tusmo would give for `guess` if `secret` was the answer
pub fn compute_feedback(secret: &[char], guess: &[char]) -> Vec<State> {
    let n = secret.len();
    let mut result = vec![State::Absent; n];
    let mut remaining: HashMap<char, usize> = HashMap::new();

    // Pass 1: exact matches. Other secret letters stay available for pass 2
    for i in 0..n {
        if secret[i] == guess[i] {
            result[i] = State::Correct;
        } else {
            *remaining.entry(secret[i]).or_insert(0) += 1;
        }
    }

    // Pass 2: misplaced letters, each secret letter can be matched only once
    for i in 0..n {
        if result[i] == State::Correct {
            continue;
        }
        if let Some(count) = remaining.get_mut(&guess[i]) {
            if *count > 0 {
                *count -= 1;
                result[i] = State::Present;
            }
        }
    }
    result
}

// A candidate is compatible if it would have produced exactly the observed row
pub fn is_compatible(candidate: &str, row: &Row) -> bool {
    let secret: Vec<char> = candidate.chars().collect();
    if secret.len() != row.len() {
        return false;
    }
    let guess: Vec<char> = row.iter().map(|t| t.letter).collect();
    let observed: Vec<State> = row.iter().map(|t| t.state).collect();
    compute_feedback(&secret, &guess) == observed
}

// Keep only the words compatible with every revealed row
pub fn filter_candidates(words: &[String], history: &[Row]) -> Vec<String> {
    words
        .iter()
        .filter(|w| history.iter().all(|row| is_compatible(w, row)))
        .cloned()
        .collect()
}

// Load the word list: uppercase, no accents, right length, optional first letter
pub fn load_words(raw: &str, len: usize, first: Option<char>) -> Vec<String> {
    raw.lines()
        .map(|l| deunicode::deunicode(l.trim()).to_uppercase())
        .filter(|w| w.chars().count() == len && w.chars().all(|c| c.is_ascii_uppercase()))
        .filter(|w| first.map_or(true, |f| w.starts_with(f)))
        .collect()
}

// Pick the word that splits the remaining candidates the best.
// The guess may be a non-candidate word if it separates them better.
pub fn best_guess(candidates: &[String], all_words: &[String]) -> Option<String> {
    // Nothing to optimize with 1 or 2 candidates: just play one
    if candidates.len() <= 2 {
        return candidates.first().cloned();
    }

    let cand_chars: Vec<Vec<char>> = candidates.iter().map(|c| c.chars().collect()).collect();

    // Score = sum of squared group sizes (lower is better).
    // A candidate gets a tiny bonus because it can win immediately.
    let score = |guess: &String, is_candidate: bool| -> usize {
        let g: Vec<char> = guess.chars().collect();
        let mut groups: HashMap<Vec<State>, usize> = HashMap::new();
        for s in &cand_chars {
            *groups.entry(compute_feedback(s, &g)).or_insert(0) += 1;
        }
        let total: usize = groups.values().map(|n| n * n).sum();
        if is_candidate {
            total * 100 - 1
        } else {
            total * 100
        }
    };

    // Many candidates: only search among candidates (keeps it fast)
    // Few candidates: also try other words of the list as probes
    let pool: Vec<(&String, bool)> = if candidates.len() > 150 {
        let step = (candidates.len() / 100).max(1);
        candidates.iter().step_by(step).map(|w| (w, true)).collect()
    } else {
        let mut p: Vec<(&String, bool)> = candidates.iter().map(|w| (w, true)).collect();
        // Spread the probes over the whole list instead of taking only the first ones
        let step = (all_words.len() / 3000).max(1);
        p.extend(all_words.iter().step_by(step).map(|w| (w, false)));
        p
    };

    pool.into_par_iter()
        .min_by_key(|(w, is_c)| score(w, *is_c))
        .map(|(w, _)| w.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chars(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    #[test]
    fn feedback_basic() {
        let fb = compute_feedback(&chars("CHAT"), &chars("CHUT"));
        assert_eq!(
            fb,
            vec![
                State::Correct,
                State::Correct,
                State::Absent,
                State::Correct
            ]
        );
    }

    #[test]
    fn feedback_duplicate_letters() {
        // Secret has only one E: the second E of the guess must be absent
        let fb = compute_feedback(&chars("ABEL"), &chars("EEXX"));
        assert_eq!(fb[0], State::Present);
        assert_eq!(fb[1], State::Absent);
    }

    #[test]
    fn filter_keeps_only_compatible() {
        let row: Row = "CHUT"
            .chars()
            .zip([
                State::Correct,
                State::Correct,
                State::Absent,
                State::Correct,
            ])
            .map(|(letter, state)| Tile { letter, state })
            .collect();
        let words = vec!["CHAT".to_string(), "CHEF".to_string()];
        assert_eq!(filter_candidates(&words, &[row]), vec!["CHAT".to_string()]);
    }

    #[test]
    fn best_guess_splits_similar_words() {
        let cands: Vec<String> = ["RAYAT", "RAPAT", "RAMAT", "RALAT", "RABAT"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let probes = vec!["RAMPY".to_string()];
        assert!(best_guess(&cands, &probes).is_some());
    }
}
