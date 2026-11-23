use clap::ValueEnum;

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum Lang {
    Fr,
    En,
}

// Everything that changes with the game language
pub struct LangConfig {
    pub url: &'static str,
    // Label of the endless mode tile
    pub mode_label: &'static str,
    // Text of the toast shown when a word is refused
    pub rejected_toast: &'static str,
    pub words_file: &'static str,
    pub banned_file: &'static str,
    pub extra_file: &'static str,
}

impl Lang {
    pub fn config(self) -> LangConfig {
        match self {
            Lang::Fr => LangConfig {
                url: "https://www.tusmo.xyz",
                mode_label: "Infini",
                rejected_toast: "Mot inconnu",
                words_file: "src/words/fr.txt",
                banned_file: "src/words/banned_words_fr.txt",
                extra_file: "src/words/extra_words_fr.txt",
            },
            Lang::En => LangConfig {
                url: "https://www.tusmo.xyz/en/",
                mode_label: "Infinite",
                rejected_toast: "Unknown word",
                words_file: "src/words/en.txt",
                banned_file: "src/words/banned_words_en.txt",
                extra_file: "src/words/extra_words_en.txt",
            },
        }
    }
}
