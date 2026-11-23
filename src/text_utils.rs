use regex::Regex;
use std::sync::OnceLock;

pub const BOLD_START: char = '\u{E000}';
pub const BOLD_END: char = '\u{E001}';

pub const SPANISH_STOPWORDS: &[&str] = &[
    "a", "ademas", "además", "ahora", "al", "algo", "alguien", "alli", "allí",
    "ante", "antes", "aqui", "aquí", "aunque", "bajo", "bien", "cada", "capitulo",
    "capítulo", "casi", "como", "con", "contra", "cual", "cuando", "de", "del",
    "desde", "despues", "después", "donde", "durante", "e", "el", "ella", "ellas",
    "ello", "ellos", "en", "entonces", "entre", "epilogo", "epílogo", "era", "erais",
    "eramos", "eran", "eras", "eres", "es", "esa", "esas", "ese", "eso", "esos",
    "esta", "estaba", "estabais", "estabamos", "estaban", "estabas", "estad",
    "estada", "estadas", "estado", "estados", "estais", "estamos", "estan",
    "estando", "estar", "estara", "estaran", "estaras", "estare", "estareis",
    "estaremos", "estaria", "estariais", "estariamos", "estarian", "estarias",
    "estas", "este", "estemos", "esto", "estos", "estoy", "estuve", "estuviera",
    "estuvierais", "estuvieramos", "estuvieran", "estuvieras", "estuvieron",
    "estuviese", "estuvieseis", "estuviesemos", "estuviesen", "estuvieses",
    "estuvimos", "estuviste", "estuvisteis", "estuvo", "fue", "fuera", "fuerais",
    "fueramos", "fueran", "fueras", "fueron", "fuese", "fueseis", "fuesemos",
    "fuesen", "fueses", "fui", "fuimos", "fuiste", "fuisteis", "ha", "habeis",
    "habia", "habiais", "habiamos", "habian", "habias", "habida", "habidas",
    "habido", "habidos", "habiendo", "habla", "hablan", "hablar", "habra",
    "habran", "habras", "habre", "habreis", "habremos", "habria", "habriais",
    "habriamos", "habrian", "habrias", "hace", "haceis", "hacemos", "hacen",
    "hacer", "hacerlo", "haces", "hacia", "haciendo", "hago", "han", "has",
    "hasta", "hay", "haya", "hayais", "hayamos", "hayan", "hayas", "he", "hecho",
    "hemos", "hube", "hubiera", "hubierais", "hubieramos", "hubieran", "hubieras",
    "hubieron", "hubiese", "hubieseis", "hubiesemos", "hubiesen", "hubieses",
    "hubimos", "hubiste", "hubisteis", "hubo", "la", "las", "le", "les", "lo",
    "los", "mas", "me", "mediante", "mientras", "mio", "misma", "mismas", "mismo",
    "mismos", "mucho", "muchos", "muy", "más", "mía", "nada", "nadie", "ni", "no",
    "nos", "nosotras", "nosotros", "nuestra", "nuestras", "nuestro", "nuestros",
    "nunca", "o", "os", "otra", "otras", "otro", "otros", "para", "parte", "pero",
    "poco", "por", "porque", "prologo", "prólogo", "que", "quien", "quienes",
    "qué", "se", "sea", "seais", "seamos", "sean", "seas", "segun", "según",
    "ser", "sera", "seran", "seras", "sere", "sereis", "seremos", "seria",
    "seriais", "seriamos", "serian", "serias", "si", "siempre", "siendo", "sin",
    "sino", "sobre", "sois", "solamente", "solo", "somos", "son", "soy", "su",
    "sus", "suya", "suyas", "suyo", "suyos", "sí", "sólo", "tal", "tambien",
    "también", "tampoco", "tan", "tanta", "tantas", "tanto", "tantos", "te",
    "tendra", "tendran", "tendras", "tendre", "tendreis", "tendremos", "tendria",
    "tendriais", "tendriamos", "tendrian", "tendrias", "tened", "teneis",
    "tenemos", "tener", "tenga", "tengais", "tengamos", "tengan", "tengas",
    "tengo", "tenia", "teniais", "teniamos", "tenian", "tenias", "tenida",
    "tenidas", "tenido", "tenidos", "teniendo", "ti", "tiene", "tienen", "tienes",
    "toda", "todas", "todo", "todos", "tras", "tu", "tus", "tuve", "tuviera",
    "tuvierais", "tuvieramos", "tuvieran", "tuvieras", "tuvieron", "tuviese",
    "tuvieseis", "tuviesemos", "tuviesen", "tuvieses", "tuvimos", "tuviste",
    "tuvisteis", "tuvo", "tuya", "tuyas", "tuyo", "tuyos", "tú", "un", "una",
    "unas", "uno", "unos", "usa", "usais", "usamos", "usan", "usar", "usas",
    "uso", "usted", "ustedes", "va", "vais", "vamos", "van", "varias", "varios",
    "vaya", "veces", "ver", "vosotras", "vosotros", "vuestra", "vuestras",
    "vuestro", "vuestros", "y", "ya", "yo",
];

pub const ENGLISH_STOPWORDS: &[&str] = &[
    "a", "about", "above", "after", "again", "against", "all", "am", "an", "and",
    "any", "are", "aren't", "as", "at", "be", "because", "been", "before", "being",
    "below", "between", "both", "but", "by", "can't", "cannot", "chapter", "could",
    "couldn't", "did", "didn't", "do", "does", "doesn't", "doing", "don't", "down",
    "during", "each", "epilogue", "few", "for", "from", "further", "had", "hadn't",
    "has", "hasn't", "have", "haven't", "having", "he", "he'd", "he'll", "he's",
    "her", "here", "here's", "hers", "herself", "him", "himself", "his", "how",
    "how's", "i", "i'd", "i'll", "i'm", "i've", "if", "in", "into", "is", "isn't",
    "it", "it's", "its", "itself", "let's", "me", "more", "most", "mustn't", "my",
    "myself", "no", "nor", "not", "of", "off", "on", "once", "only", "or", "other",
    "ought", "our", "ours", "ourselves", "out", "over", "own", "part", "prologue",
    "same", "shan't", "she", "she'd", "she'll", "she's", "should", "shouldn't",
    "so", "some", "such", "than", "that", "that's", "the", "their", "theirs",
    "them", "themselves", "then", "there", "there's", "these", "they", "they'd",
    "they'll", "they're", "they've", "this", "those", "through", "to", "too",
    "under", "until", "up", "very", "was", "wasn't", "we", "we'd", "we'll",
    "we're", "we've", "were", "weren't", "what", "what's", "when", "when's",
    "where", "where's", "which", "while", "who", "who's", "whom", "why",
    "why's", "with", "won't", "would", "wouldn't", "you", "you'd", "you'll",
    "you're", "you've", "your", "yours", "yourself", "yourselves",
];

struct TextUtilRegexes {
    bold_merge_re: Regex,
    bold_empty_re: Regex,
}

static TEXT_UTIL_REGEXES: OnceLock<TextUtilRegexes> = OnceLock::new();

fn get_text_util_regexes() -> &'static TextUtilRegexes {
    TEXT_UTIL_REGEXES.get_or_init(|| TextUtilRegexes {
        bold_merge_re: Regex::new(r"\u{E001}(\s*)\u{E000}").unwrap(),
        bold_empty_re: Regex::new(r"\u{E000}\s*\u{E001}").unwrap(),
    })
}

pub fn is_stopword(word: &str, lang: &str) -> bool {
    let lower = word.to_lowercase();
    match lang.to_lowercase().as_str() {
        "en" | "english" => ENGLISH_STOPWORDS.binary_search(&lower.as_str()).is_ok(),
        _ => SPANISH_STOPWORDS.binary_search(&lower.as_str()).is_ok(),
    }
}

pub fn collapse_whitespace(text: &str) -> String {
    let trimmed = text.trim();
    if !trimmed.contains(char::is_whitespace) {
        return trimmed.to_string();
    }
    let mut out = String::with_capacity(trimmed.len());
    let mut in_space = false;
    for c in trimmed.chars() {
        if c.is_whitespace() {
            if !in_space {
                out.push(' ');
                in_space = true;
            }
        } else {
            out.push(c);
            in_space = false;
        }
    }
    out
}

pub fn merge_bold_tokens(s: &str) -> String {
    if !s.contains(BOLD_START) && !s.contains(BOLD_END) {
        return s.to_string();
    }
    let rx = get_text_util_regexes();
    let res = rx.bold_merge_re.replace_all(s, "$1");
    rx.bold_empty_re.replace_all(&res, "").to_string()
}

pub fn strip_formatting_tokens(s: &str) -> String {
    if !s.contains(BOLD_START) && !s.contains(BOLD_END) {
        return s.to_string();
    }
    s.replace(BOLD_START, "").replace(BOLD_END, "")
}

pub fn format_tokens_to_html(s: &str) -> String {
    if !s.contains(BOLD_START) && !s.contains(BOLD_END) {
        return s.to_string();
    }
    s.replace(BOLD_START, "<strong>").replace(BOLD_END, "</strong>")
}

pub fn is_line_hyphen(c: char) -> bool {
    matches!(c, '-' | '\u{00AD}' | '\u{2010}' | '\u{2011}' | '\u{2013}' | '\u{2212}')
}

pub fn try_merge_hyphenated_line(last_line: &mut String, next_line: &str) -> bool {
    let trimmed_next = next_line.trim_start();
    let starts_bold = trimmed_next.starts_with(BOLD_START);
    let next_content = if starts_bold {
        &trimmed_next[BOLD_START.len_utf8()..]
    } else {
        trimmed_next
    };
    let starts_alpha = next_content.chars().next().is_some_and(char::is_alphabetic);
    if !starts_alpha {
        return false;
    }

    let trimmed_len = last_line.trim_end().len();
    if trimmed_len == 0 {
        return false;
    }
    let trimmed_last = &last_line[..trimmed_len];

    let ends_bold = trimmed_last.ends_with(BOLD_END);
    let check_str = if ends_bold {
        &trimmed_last[..trimmed_last.len() - BOLD_END.len_utf8()]
    } else {
        trimmed_last
    };

    let check_trimmed = check_str.trim_end_matches(is_line_hyphen);
    if check_trimmed.len() == check_str.len() {
        return false;
    }
    let prev_alpha = check_trimmed.chars().next_back().is_some_and(char::is_alphabetic);

    if prev_alpha {
        last_line.truncate(trimmed_len);
        if ends_bold {
            last_line.truncate(last_line.len() - BOLD_END.len_utf8());
            while last_line.chars().next_back().is_some_and(is_line_hyphen) {
                last_line.pop();
            }
            if starts_bold {
                last_line.push_str(&trimmed_next[BOLD_START.len_utf8()..]);
            } else {
                last_line.push(BOLD_END);
                last_line.push_str(trimmed_next);
            }
        } else {
            while last_line.chars().next_back().is_some_and(is_line_hyphen) {
                last_line.pop();
            }
            last_line.push_str(trimmed_next);
        }
        true
    } else {
        false
    }
}

pub fn join_hyphenated_texts(first: &str, second: &str) -> String {
    let mut f_str = first.trim_end().to_string();
    let s_str = second.trim_start();
    if try_merge_hyphenated_line(&mut f_str, s_str) {
        return merge_bold_tokens(&f_str);
    }
    let res = if f_str.ends_with('《')
        || f_str.ends_with('『')
        || f_str.ends_with('«')
        || f_str.ends_with('[')
        || f_str.ends_with('“')
        || f_str.ends_with('—')
        || f_str.ends_with('―')
    {
        format!("{}{}", f_str, s_str)
    } else {
        format!("{} {}", f_str, s_str)
    };
    merge_bold_tokens(&res)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stopwords_order_and_detection() {
        for w in SPANISH_STOPWORDS.windows(2) {
            assert!(w[0] < w[1], "Unsorted Spanish: {} >= {}", w[0], w[1]);
        }
        for w in ENGLISH_STOPWORDS.windows(2) {
            assert!(w[0] < w[1], "Unsorted English: {} >= {}", w[0], w[1]);
        }

        assert!(is_stopword("cuando", "es"));
        assert!(is_stopword("Capítulo", "es"));
        assert!(is_stopword("when", "en"));
        assert!(is_stopword("Chapter", "en"));
        assert!(!is_stopword("Ikki", "es"));
        assert!(!is_stopword("Kurogane", "en"));
    }

    #[test]
    fn test_collapse_whitespace() {
        assert_eq!(collapse_whitespace("  hello   world  "), "hello world");
        assert_eq!(collapse_whitespace("single"), "single");
    }

    #[test]
    fn test_formatting_tokens() {
        let text = format!("{}bold text{}", BOLD_START, BOLD_END);
        assert_eq!(strip_formatting_tokens(&text), "bold text");
        assert_eq!(format_tokens_to_html(&text), "<strong>bold text</strong>");
    }

    #[test]
    fn test_join_hyphenated_texts() {
        assert_eq!(join_hyphenated_texts("transfor-", "mación"), "transformación");
        assert_eq!(join_hyphenated_texts("pintu-\u{00AD}", "ra"), "pintura");
        assert_eq!(join_hyphenated_texts("pintu-  ", "  ra"), "pintura");
        assert_eq!(join_hyphenated_texts("pintu\u{00AD}", "ra"), "pintura");
        assert_eq!(join_hyphenated_texts("Hola", "Mundo"), "Hola Mundo");
        assert_eq!(join_hyphenated_texts("—", "Diálogo"), "—Diálogo");
    }
}
