use crate::models::PdfPageData;
use regex::{Captures, Regex};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::OnceLock;

pub const LANGUAGE: &str = "es";
pub const ENABLE_DROP_CAPS: bool = true;
pub const FOOTNOTE_STYLE: &str = "end_of_chapter";
pub const FULL_PAGE_ILLUSTRATIONS: bool = true;
pub const SKIP_VISUAL_TOC_PAGES: bool = true;

fn default_parse_language() -> String {
    "auto".to_string()
}

fn default_preset_id() -> String {
    "visual-novel".to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParseOptions {
    #[serde(default = "default_parse_language", alias = "lang")]
    pub language: String,
    #[serde(
        default = "default_preset_id",
        alias = "preset_id",
        alias = "preset",
        alias = "profile"
    )]
    pub preset_id: String,
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self {
            language: default_parse_language(),
            preset_id: default_preset_id(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetProfile {
    pub id: String,
    pub enable_drop_caps: bool,
    pub enable_lit_rpg_brackets: bool,
    pub enable_scene_breaks: bool,
    pub footnote_style: String,
    pub full_page_illustrations: bool,
}

impl Default for PresetProfile {
    fn default() -> Self {
        Self::from_id("visual-novel")
    }
}

impl PresetProfile {
    pub fn from_id(raw_id: &str) -> Self {
        let norm = raw_id.trim().to_lowercase();
        match norm.as_str() {
            "webnovel" | "en-litrpg" | "litrpg" => Self {
                id: "webnovel".to_string(),
                enable_drop_caps: false,
                enable_lit_rpg_brackets: true,
                enable_scene_breaks: true,
                footnote_style: "callout_card".to_string(),
                full_page_illustrations: true,
            },
            "generic" | "en-fiction" | "fiction" => Self {
                id: "generic".to_string(),
                enable_drop_caps: true,
                enable_lit_rpg_brackets: false,
                enable_scene_breaks: true,
                footnote_style: "end_of_chapter".to_string(),
                full_page_illustrations: false,
            },
            "readme" => Self {
                id: "readme".to_string(),
                enable_drop_caps: false,
                enable_lit_rpg_brackets: false,
                enable_scene_breaks: true,
                footnote_style: "end_of_chapter".to_string(),
                full_page_illustrations: false,
            },
            "paper" => Self {
                id: "paper".to_string(),
                enable_drop_caps: false,
                enable_lit_rpg_brackets: false,
                enable_scene_breaks: false,
                footnote_style: "end_of_chapter".to_string(),
                full_page_illustrations: false,
            },
            _ => Self {
                id: "visual-novel".to_string(),
                enable_drop_caps: ENABLE_DROP_CAPS,
                enable_lit_rpg_brackets: true,
                enable_scene_breaks: true,
                footnote_style: FOOTNOTE_STYLE.to_string(),
                full_page_illustrations: FULL_PAGE_ILLUSTRATIONS,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageProfile {
    pub code: String,
    pub dialogue_open: String,
    pub dialogue_close: String,
    pub thought_open: String,
    pub thought_close: String,
    pub technique_open: String,
    pub technique_close: String,
    pub system_open: String,
    pub system_close: String,
    pub use_em_dash_dialogue: bool,
}

impl Default for LanguageProfile {
    fn default() -> Self {
        Self::for_language(LANGUAGE)
    }
}

impl LanguageProfile {
    pub fn for_language(lang: &str) -> Self {
        let code = if lang.trim().to_lowercase().starts_with("en") {
            "en"
        } else {
            "es"
        };
        if code == "en" {
            Self {
                code: "en".to_string(),
                dialogue_open: "“".to_string(),
                dialogue_close: "”".to_string(),
                thought_open: "«".to_string(),
                thought_close: "»".to_string(),
                technique_open: "《".to_string(),
                technique_close: "》".to_string(),
                system_open: "[".to_string(),
                system_close: "]".to_string(),
                use_em_dash_dialogue: false,
            }
        } else {
            Self {
                code: "es".to_string(),
                dialogue_open: "—".to_string(),
                dialogue_close: String::new(),
                thought_open: "«".to_string(),
                thought_close: "»".to_string(),
                technique_open: "《".to_string(),
                technique_close: "》".to_string(),
                system_open: "[".to_string(),
                system_close: "]".to_string(),
                use_em_dash_dialogue: true,
            }
        }
    }
}

pub fn detect_language_from_pages(pages: &[PdfPageData]) -> &'static str {
    let mut sample = String::new();
    for page in pages.iter().take(20) {
        for b in &page.blocks {
            if b.block_type != 0 {
                continue;
            }
            for l in &b.lines {
                for s in &l.spans {
                    sample.push_str(&s.text);
                    sample.push(' ');
                }
            }
        }
        if sample.len() >= 12000 {
            break;
        }
    }
    crate::text_utils::detect_language_from_text(&sample)
}

pub fn resolve_language(requested: &str, text_sample: &str, preset_id: &str) -> &'static str {
    let req = requested.trim().to_lowercase();
    if req == "en" || req == "english" || req.starts_with("en-") {
        return "en";
    }
    if req == "es" || req == "spanish" || req.starts_with("es-") {
        return "es";
    }
    let (es_score, en_score) = crate::text_utils::count_language_stopwords(text_sample);
    if en_score > es_score {
        "en"
    } else if es_score > en_score {
        "es"
    } else if preset_id.trim().to_lowercase().starts_with("en-") {
        "en"
    } else {
        "es"
    }
}

pub const CHAPTER_PATTERNS: &[&str] = &[
    r"^Cap[íi]tulo\s+\d+",
    r"^Cap[íi]tulo\s+[IVXLCDM]+\b",
    r"^Cap[íi]tulo\s+Final(?:\s+[A-Z])?\b",
    r"^Chapter\s+\d+",
    r"^Pr[óo]logo\b",
    r"^Prologue\b",
    r"^Ep[íi]logo\b",
    r"^Epilogue\b",
    r"^Interludio\b",
    r"^Intermisi[óo]n\b",
    r"^Palabras\s+(?:del\s+autor|finales|de\s+cierre)\b",
    r"^Posfacio\b",
    r"^Afterword\b",
    r"^Resumen\s+(?:de\s+lo\s+ocurrido\s+)?hasta\s+ahora\b",
];

pub const SUBSECTION_PATTERNS: &[&str] = &[
    r"^Parte\s+\d+\.?$",
    r"^Parte\s+[IVXLCDM]+\.?$",
    r"^Part\s+\d+\.?$",
    r"^Part\s+[IVXLCDM]+\.?$",
];

pub const PART_PATTERNS: &[&str] = &[
    r"^Parte\s+\d+",
    r"^Parte\s+[IVXLCDM]+",
    r"^Part\s+\d+",
    r"^Part\s+[IVXLCDM]+",
];

pub const SCENE_BREAK_PATTERNS: &[&str] = &[
    r"^\s*[◊◇♦◆✦✧❖]\s*$",
    r"^\s*[\*＊✦✧❖♦◆◇◊•·◦○●□■△▲▽▼☆★※†‡§~∼─━—–―\-_=+]{2,}(?:\s+[\*＊✦✧❖♦◆◇◊•·◦○●□■△▲▽▼☆★※†‡§~∼─━—–―\-_=+]+)*\s*$",
    r"^\s*(?:[\*＊✦✧❖♦◆◇◊•·◦○●□■△▲▽▼☆★※†‡§~∼─━—–―\-_=+]\s+){2,}[\*＊✦✧❖♦◆◇◊•·◦○●□■△▲▽▼☆★※†‡§~∼─━—–―\-_=+]\s*$",
    r"^\s*(?:x\s+){2,}x\s*$",
    r"^\s*x{3,}\s*$",
];

pub const HEADER_NOISE_PATTERNS: &[&str] = &[
    r#"^\s*(?:(?:Traducci[oó]n|Traductor(?:a)?|Correc?ci[oó]n|Corrector(?:a)?|Edici[oó]n|Editor(?:a)?|Staff|Clean(?:er)?|Redraw(?:er)?|Typeset(?:ter|ting)?|QC)\b[^.!?]*:.*|TRADUCCI[ÓO]N\s+[A-ZÁÉÍÓÚÑ\s–\-]+)$"#,
    r#"^.*(?:https?://|www\.|\b(?:wordpress|blogspot|facebook|discord|telegram)\b).*$"#,
    r"(?i)^P[áa]gina\s*\|\s*\d+$",
    r"(?i)^P[áa]gina\s+\d+(?:\s+de\s+\d+)?$",
    r"^\s*[-–—]?\s*\d{1,4}\s*[-–—]?\s*$",
    r"(?i)^.*\b(?:Cap[íi]tulo|Pr[óo]logo|Interludio|Intermisi[óo]n|Ep[íi]logo|Palabras\s+del\s+Autor|Afterword|Posfacio)(?:\s+[^|]+)?\s*\|\s*\d+.*$",
    r"^\s*\d{1,4}\s*\|\s*[^.!?]{2,65}\s*$",
    r"^\s*[^.!?]{2,65}\s*\|\s*\d{1,4}\s*$",
    r"(?i)^\s*(?:\d+\s+)?Traducci[oó]n:\s*\S+\s+Correc?ci[oó]n:\s*\S+\s*$",
    r"(?i)^\s*(?:\d+\s+)?Traductor:\s*\S+\s+Corrector:\s*\S+\s*$",
    r"(?i)^VOLUMEN\s+\d+\s*[-–—]\s*LIGHT\s+NOVEL(?:\s+PREMIUM)?$",
    r"(?i)^LIGHT\s+NOVEL(?:\s+PREMIUM)?$",
    r"(?i)^[A-Za-z0-9\s]{2,45}\b(?:Fansub|Scanlation|Project)\s*$",
];

pub const DIALOGUE_DASHES: &[char] = &['—', '–', '―', '-', '−'];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DelimiterLevel {
    Level1SpokenDialogue,
    Level2MediatedThoughtCite,
    Level3TechniqueSkillSystem,
    Level4InterfaceStatusLocation,
    Level5RhythmPauseSilence,
}

#[derive(Debug, Clone, Copy)]
pub struct DelimiterHierarchySpec {
    pub level: DelimiterLevel,
    pub canonical_open: &'static str,
    pub canonical_close: &'static str,
    pub target_style: Option<&'static str>,
    pub source_openers: &'static [char],
    pub source_closers: &'static [char],
}

pub const LEVEL1_DIALOGUE_DASHES: &[char] = DIALOGUE_DASHES;
pub const LEVEL1_SPOKEN_OPENERS: &[char] = &['—', '–', '―', '-', '−', '「', '“', '"'];
pub const LEVEL1_SPOKEN_CLOSERS: &[char] = &['」', '”', '"'];

pub const LEVEL2_THOUGHT_OPENERS: &[char] = &['«', '『', '「', '“', '"', '‘', '\'', '〝', '（'];
pub const LEVEL2_THOUGHT_CLOSERS: &[char] = &['»', '』', '」', '”', '"', '’', '\'', '〟', '）'];
pub const LEVEL2_THOUGHT_CITE_PAIRS: &[(char, char)] = &[
    ('«', '»'),
    ('『', '』'),
    ('「', '」'),
    ('“', '”'),
    ('"', '"'),
    ('‘', '’'),
    ('\'', '\''),
    ('〝', '〟'),
    ('（', '）'),
];

pub const LEVEL3_TECHNIQUE_OPENERS: &[char] = &['《', '≪', '〈'];
pub const LEVEL3_TECHNIQUE_CLOSERS: &[char] = &['》', '≫', '〉'];
pub const LEVEL3_TECHNIQUE_SKILL_PAIRS: &[(char, char)] = &[
    ('《', '》'),
    ('≪', '≫'),
    ('〈', '〉'),
];

pub const LEVEL4_SYSTEM_OPENERS: &[char] = &['[', '【', '［', '〔'];
pub const LEVEL4_SYSTEM_CLOSERS: &[char] = &[']', '】', '］', '〕'];
pub const LEVEL4_SYSTEM_UI_PAIRS: &[(char, char)] = &[
    ('[', ']'),
    ('【', '】'),
    ('［', '］'),
    ('〔', '〕'),
];

pub const LEVEL5_PAUSE_CHARS: &[char] = &['─', '—', '―', '–', '…', '.'];
pub const LEVEL5_TRIPLE_BAR: &str = "───";
pub const LEVEL5_ELLIPSIS: &str = "…";

pub const STITCHABLE_DELIMITER_PAIRS: &[(char, char)] = &[
    ('[', ']'),
    ('«', '»'),
    ('《', '》'),
    ('『', '』'),
    ('「', '」'),
];

pub const DEFAULT_DELIMITER_HIERARCHY: [DelimiterHierarchySpec; 5] = [
    DelimiterHierarchySpec {
        level: DelimiterLevel::Level1SpokenDialogue,
        canonical_open: "—",
        canonical_close: "",
        target_style: Some("dialogue"),
        source_openers: LEVEL1_SPOKEN_OPENERS,
        source_closers: LEVEL1_SPOKEN_CLOSERS,
    },
    DelimiterHierarchySpec {
        level: DelimiterLevel::Level2MediatedThoughtCite,
        canonical_open: "«",
        canonical_close: "»",
        target_style: Some("thought"),
        source_openers: LEVEL2_THOUGHT_OPENERS,
        source_closers: LEVEL2_THOUGHT_CLOSERS,
    },
    DelimiterHierarchySpec {
        level: DelimiterLevel::Level3TechniqueSkillSystem,
        canonical_open: "《",
        canonical_close: "》",
        target_style: None,
        source_openers: LEVEL3_TECHNIQUE_OPENERS,
        source_closers: LEVEL3_TECHNIQUE_CLOSERS,
    },
    DelimiterHierarchySpec {
        level: DelimiterLevel::Level4InterfaceStatusLocation,
        canonical_open: "[",
        canonical_close: "]",
        target_style: Some("bracket_speech"),
        source_openers: LEVEL4_SYSTEM_OPENERS,
        source_closers: LEVEL4_SYSTEM_CLOSERS,
    },
    DelimiterHierarchySpec {
        level: DelimiterLevel::Level5RhythmPauseSilence,
        canonical_open: LEVEL5_TRIPLE_BAR,
        canonical_close: LEVEL5_ELLIPSIS,
        target_style: None,
        source_openers: LEVEL5_PAUSE_CHARS,
        source_closers: LEVEL5_PAUSE_CHARS,
    },
];

pub fn normalize_cjk_delimiter_variants(text: &str) -> String {
    if !text.chars().any(|c| {
        matches!(
            c,
            '≪' | '≫' | '〈' | '〉' | '【' | '】' | '［' | '］' | '〔' | '〕' | '〝' | '〟'
        )
    }) {
        return text.to_string();
    }
    text.chars()
        .map(|c| match c {
            '≪' | '〈' => '《',
            '≫' | '〉' => '》',
            '【' | '［' | '〔' => '[',
            '】' | '］' | '〕' => ']',
            '〝' => '『',
            '〟' => '』',
            other => other,
        })
        .collect()
}

#[derive(Debug, Clone)]
pub struct DocumentProfile {
    pub inferred_series_title: String,
    pub volume_number: usize,
    pub base_font_size: f64,
    pub para_break_dy: f64,
    pub recurring_noise_signatures: HashSet<String>,
    pub learned_proper_nouns: Vec<(Regex, String)>,
    pub learned_titled_phrases: Vec<(Regex, String)>,
    pub language: String,
}

struct ProfileRegexes {
    vol_split_re: Regex,
    vol_num_re: Regex,
    ws_re: Regex,
    digits_re: Regex,
    protected_heading_re: Regex,
    mid_sentence_word_re: Regex,
    mid_sentence_phrase_re: Regex,
    quoted_phrase_re: Regex,
}

static PROFILE_REGEXES: OnceLock<ProfileRegexes> = OnceLock::new();

fn get_profile_regexes() -> &'static ProfileRegexes {
    PROFILE_REGEXES.get_or_init(|| ProfileRegexes {
        vol_split_re: Regex::new(r"(?i)\s*[-–—]?\s*(?:volumen|vol\.?|tomo)\s*(\d+).*$").unwrap(),
        vol_num_re: Regex::new(r"(?i)(?:volumen|vol\.?|tomo)\s*(\d+)").unwrap(),
        ws_re: Regex::new(r"\s+").unwrap(),
        digits_re: Regex::new(r"\d+").unwrap(),
        protected_heading_re: Regex::new(
            r"(?i)^(?:cap[íi]tulo|chapter|pr[óo]logo|prologue|ep[íi]logo|epilogue|interludio|intermisi[óo]n|parte|part|palabras|posfacio|afterword|resumen|notas?|[íi]ndice|contenido|sumario|tabla\s+de|sinopsis|fin)\b",
        )
        .unwrap(),
        mid_sentence_word_re: Regex::new(
            r"(?:[a-záéíóúüñ]{2,}|[,;])\s+([A-ZÁÉÍÓÚÜÑ][a-záéíóúüñ]{2,})\b",
        )
        .unwrap(),
        mid_sentence_phrase_re: Regex::new(
            r"(?:[a-záéíóúüñ]{2,}\s+|«|《|“)((?:(?:El|La|Los|Las)\s+)?[A-ZÁÉÍÓÚÜÑ][a-záéíóúüñ]{2,}(?:\s+(?:de(?:\s+la|\s+las|\s+los)?|del|sin|y|en|al)\s+[A-ZÁÉÍÓÚÜÑ][a-záéíóúüñ]{2,}|\s+[A-ZÁÉÍÓÚÜÑ][a-záéíóúüñ]{2,})+)",
        )
        .unwrap(),
        quoted_phrase_re: Regex::new(
            r"[«《“]((?:El|La|Los|Las)\s+[A-ZÁÉÍÓÚÜÑ][a-záéíóúüñ]{2,})[»》”]",
        )
        .unwrap(),
    })
}

pub fn normalize_noise_signature(text: &str) -> String {
    let rx = get_profile_regexes();
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let collapsed = crate::text_utils::collapse_whitespace(trimmed);
    let num_norm = rx.digits_re.replace_all(&collapsed, "<N>");
    num_norm
        .trim_matches(|c: char| c.is_whitespace() || matches!(c, '|' | '-' | '–' | '—' | '·' | '•'))
        .trim()
        .to_lowercase()
}

pub fn infer_series_and_volume(filename: &str) -> (String, usize) {
    let rx = get_profile_regexes();
    let stem = Path::new(filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(filename)
        .replace('_', " ");

    let vol_num = rx
        .vol_num_re
        .captures(&stem)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse::<usize>().ok())
        .unwrap_or(1);

    let raw_series = rx.vol_split_re.replace(&stem, "").trim().to_string();
    let series = if raw_series.is_empty() {
        "Volumen".to_string()
    } else {
        raw_series
    };

    (series, vol_num)
}

impl DocumentProfile {
    pub fn from_pages(pages: &[PdfPageData], filename: &str) -> Self {
        Self::from_pages_with_language(pages, filename, LANGUAGE)
    }

    pub fn from_pages_with_language(
        pages: &[PdfPageData],
        filename: &str,
        language: &str,
    ) -> Self {
        let rx = get_profile_regexes();
        let (inferred_series_title, volume_number) = infer_series_and_volume(filename);

        let mut size_counts: HashMap<i32, usize> = HashMap::new();
        let sample_len = pages.len().min(30);
        for page in &pages[..sample_len] {
            for b in &page.blocks {
                if b.block_type != 0 {
                    continue;
                }
                for l in &b.lines {
                    for s in &l.spans {
                        let t = s.text.trim();
                        if t.chars().count() > 3 {
                            let key = (s.size * 10.0).round() as i32;
                            *size_counts.entry(key).or_insert(0) += t.chars().count();
                        }
                    }
                }
            }
        }
        let base_font_size = size_counts
            .into_iter()
            .max_by_key(|&(_, count)| count)
            .map(|(k, _)| (k as f64) / 10.0)
            .unwrap_or(11.0);
        let para_break_dy = (base_font_size * 1.45).max(15.0);

        let mut sig_page_sets: HashMap<String, HashSet<usize>> = HashMap::new();
        let mut edge_sig_page_sets: HashMap<String, HashSet<usize>> = HashMap::new();
        let mut top_header_sig_page_sets: HashMap<String, HashSet<usize>> = HashMap::new();

        let mut proper_word_counts: HashMap<String, HashMap<String, usize>> = HashMap::new();
        let mut lower_word_counts: HashMap<String, usize> = HashMap::new();
        let mut phrase_counts: HashMap<String, (String, usize)> = HashMap::new();

        for (pno, page) in pages.iter().enumerate() {
            let page_h = page.height.max(100.0);
            let mut page_lines: Vec<(String, f64, f64)> = Vec::new();

            for b in &page.blocks {
                if b.block_type != 0 {
                    continue;
                }
                for l in &b.lines {
                    let mut raw = String::new();
                    for s in &l.spans {
                        raw.push_str(&s.text);
                    }
                    let clean = normalize_cjk_delimiter_variants(
                        &rx.ws_re.replace_all(raw.trim(), " "),
                    );
                    if !clean.is_empty() {
                        page_lines.push((clean, l.bbox.1, l.bbox.3));
                    }
                }
            }

            let page_text: String = page_lines
                .iter()
                .map(|(t, _, _)| t.as_str())
                .collect::<Vec<_>>()
                .join(" ");

            for cap in rx.mid_sentence_phrase_re.captures_iter(&page_text) {
                let phrase = cap[1].trim();
                let word_cnt = phrase.split_whitespace().count();
                if (2..=15).contains(&word_cnt) {
                    let low = phrase.to_lowercase();
                    let entry = phrase_counts
                        .entry(low)
                        .or_insert_with(|| (phrase.to_string(), 0));
                    entry.1 += 1;
                    if let Some(stripped_el) = phrase
                        .strip_prefix("El ")
                        .or_else(|| phrase.strip_prefix("el "))
                        .or_else(|| phrase.strip_prefix("La "))
                        .or_else(|| phrase.strip_prefix("la "))
                    {
                        let stripped_el = stripped_el.trim();
                        if stripped_el.split_whitespace().count() >= 3 {
                            let low_sub = stripped_el.to_lowercase();
                            let sub_entry = phrase_counts
                                .entry(low_sub)
                                .or_insert_with(|| (stripped_el.to_string(), 0));
                            sub_entry.1 += 1;
                        }
                    }
                    if let Some((_, after_del)) = phrase.split_once(" del ") {
                        let after_del = after_del.trim();
                        if after_del.split_whitespace().count() >= 3 {
                            let low_sub = after_del.to_lowercase();
                            let sub_entry = phrase_counts
                                .entry(low_sub)
                                .or_insert_with(|| (after_del.to_string(), 0));
                            sub_entry.1 += 1;
                        }
                    }
                }
            }

            for cap in rx.quoted_phrase_re.captures_iter(&page_text) {
                let phrase = cap[1].trim();
                let low = phrase.to_lowercase();
                let entry = phrase_counts
                    .entry(low)
                    .or_insert_with(|| (phrase.to_string(), 0));
                entry.1 += 2;
            }

            for (line_text, y0, y1) in page_lines.iter() {
                let char_len = line_text.chars().count();
                let is_dialogue_or_quote = line_text.starts_with('—')
                    || line_text.starts_with('―')
                    || line_text.starts_with('–')
                    || line_text.starts_with('-')
                    || line_text.starts_with('«')
                    || line_text.starts_with('“')
                    || line_text.starts_with('"')
                    || line_text.starts_with('『')
                    || line_text.starts_with('「')
                    || line_text.starts_with('*')
                    || line_text.starts_with('¡')
                    || line_text.starts_with('¿');
                let ends_like_sentence = line_text.ends_with('.')
                    || line_text.ends_with('!')
                    || line_text.ends_with('?')
                    || line_text.ends_with('…')
                    || line_text.ends_with(':')
                    || line_text.ends_with('»')
                    || line_text.ends_with('”')
                    || line_text.ends_with('"')
                    || line_text.ends_with('」')
                    || line_text.ends_with('』');
                let has_narrative_punct = line_text.chars().any(|c| {
                    matches!(
                        c,
                        '─' | '—' | '―' | '.' | ',' | ';' | ':' | '!' | '?' | '…' | '«' | '»' | '"' | '“' | '”' | '《' | '》' | '『' | '』' | '「' | '」'
                    )
                });

                let is_all_caps_banner = line_text.split_whitespace().count() >= 2
                    && line_text.chars().any(|c| c.is_alphabetic())
                    && line_text
                        .chars()
                        .filter(|c| c.is_alphabetic())
                        .all(|c| c.is_uppercase())
                    && !has_narrative_punct;
                let in_margin = *y0 < page_h * 0.15 || *y1 > page_h * 0.85;
                if is_all_caps_banner
                    && char_len >= 6
                    && char_len <= 65
                    && !is_dialogue_or_quote
                    && !ends_like_sentence
                    && !rx.protected_heading_re.is_match(line_text)
                {
                    let sig = normalize_noise_signature(line_text);
                    if sig.chars().count() >= 6
                        && sig.chars().any(|c| c.is_alphabetic())
                        && !rx.protected_heading_re.is_match(&sig)
                    {
                        sig_page_sets.entry(sig.clone()).or_default().insert(pno);
                        if in_margin || pno < 6 || pno + 6 >= pages.len() {
                            edge_sig_page_sets.entry(sig).or_default().insert(pno);
                        }
                    }
                }

                if *y0 < page_h * 0.09
                    && line_text.split_whitespace().count() >= 3
                    && !has_narrative_punct
                    && !is_dialogue_or_quote
                    && !ends_like_sentence
                    && char_len >= 8
                    && char_len <= 70
                    && !rx.protected_heading_re.is_match(line_text)
                {
                    let sig = normalize_noise_signature(line_text);
                    if sig.chars().count() >= 8
                        && sig.chars().any(|c| c.is_alphabetic())
                        && !rx.protected_heading_re.is_match(&sig)
                    {
                        top_header_sig_page_sets
                            .entry(sig)
                            .or_default()
                            .insert(pno);
                    }
                }

                for word in line_text.split(|c: char| !c.is_alphabetic()) {
                    let w_chars: Vec<char> = word.chars().collect();
                    if w_chars.len() >= 3 && w_chars.iter().all(|c| c.is_lowercase()) {
                        *lower_word_counts.entry(word.to_string()).or_insert(0) += 1;
                    }
                }

                for cap in rx.mid_sentence_word_re.captures_iter(line_text) {
                    let w = &cap[1];
                    let low = w.to_lowercase();
                    *proper_word_counts
                        .entry(low)
                        .or_default()
                        .entry(w.to_string())
                        .or_insert(0) += 1;
                }
            }
        }

        let mut recurring_noise_signatures = HashSet::new();
        let series_sig = normalize_noise_signature(&inferred_series_title);
        if series_sig.chars().count() >= 4 {
            recurring_noise_signatures.insert(series_sig.clone());
            recurring_noise_signatures.insert(format!("{} volumen <n>", series_sig));
            recurring_noise_signatures.insert(format!("{} - volumen <n>", series_sig));
        }

        for (sig, p_set) in sig_page_sets {
            if p_set.len() >= 2 && edge_sig_page_sets.contains_key(&sig) {
                recurring_noise_signatures.insert(sig);
            }
        }
        for (sig, p_set) in top_header_sig_page_sets {
            if p_set.len() >= 10 {
                recurring_noise_signatures.insert(sig);
            }
        }

        let mut learned_proper_nouns = Vec::new();
        for (low, forms) in proper_word_counts {
            if crate::text_utils::is_stopword(&low, language) {
                continue;
            }
            let total_cap: usize = forms.values().sum();
            let total_low = lower_word_counts.get(&low).copied().unwrap_or(0);
            if total_cap >= 2 && total_cap * 2 >= total_low {
                if let Some((best_form, _)) = forms.into_iter().max_by_key(|&(_, c)| c) {
                    if let Ok(re) = Regex::new(&format!(r"(?i)\b{}\b", regex::escape(&low))) {
                        learned_proper_nouns.push((re, best_form));
                    }
                }
            }
        }

        let mut phrase_vec: Vec<(String, String, usize)> = phrase_counts
            .into_iter()
            .filter(|(low, (_, count))| {
                let word_cnt = low.split_whitespace().count();
                let is_generic = low.contains("ronda")
                    || low.contains("sala ")
                    || low.contains("imperio ")
                    || low.starts_with("la primera")
                    || low.starts_with("la segunda")
                    || low.starts_with("la tercera");
                let min_req = if word_cnt >= 4 { 1 } else { 2 };
                *count >= min_req
                    && !is_generic
                    && !rx.protected_heading_re.is_match(low)
                    && low.chars().count() >= 6
            })
            .map(|(low, (form, count))| (low, form, count))
            .collect();
        phrase_vec.sort_by(|a, b| b.0.len().cmp(&a.0.len()));

        let mut learned_titled_phrases = Vec::new();
        for (low, form, _) in phrase_vec {
            let pat_str = low
                .split_whitespace()
                .map(regex::escape)
                .collect::<Vec<_>>()
                .join(r"\s+");
            if let Ok(re) = Regex::new(&format!(r"(?i)\b{}\b", pat_str)) {
                learned_titled_phrases.push((re, form));
            }
        }

        Self {
            inferred_series_title,
            volume_number,
            base_font_size,
            para_break_dy,
            recurring_noise_signatures,
            learned_proper_nouns,
            learned_titled_phrases,
            language: language.to_string(),
        }
    }

    pub fn is_margin_or_banner_noise(&self, text: &str) -> bool {
        let sig = normalize_noise_signature(text);
        if sig.is_empty() {
            return false;
        }
        self.recurring_noise_signatures.contains(&sig)
    }

    pub fn apply_learned_capitalization(&self, text: &str, is_all_caps_source: bool) -> String {
        if self.learned_proper_nouns.is_empty() && self.learned_titled_phrases.is_empty() {
            return text.to_string();
        }
        let mut s = text.to_string();
        if is_all_caps_source {
            for (re, form) in &self.learned_proper_nouns {
                if re.is_match(&s) {
                    s = re
                        .replace_all(&s, |_: &Captures| form.as_str())
                        .to_string();
                }
            }
        }
        for (re, form) in &self.learned_titled_phrases {
            if is_all_caps_source || form.split_whitespace().count() >= 5 {
                if re.is_match(&s) {
                    s = re
                        .replace_all(&s, |_: &Captures| form.as_str())
                        .to_string();
                }
            }
        }
        s
    }
}
