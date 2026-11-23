use crate::models::{FrontendSpellCorrection, Paragraph, SpellTransformation};
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;
use symspell::{SymSpell, SymSpellBuilder, UnicodeStringStrategy, Verbosity};

const ES_FREQ_DICT: &str = include_str!("../data/dictionaries/es_frequency.txt");
const EN_FREQ_DICT: &str = include_str!("../data/dictionaries/en_frequency.txt");
const JA_ROMAJI_DICT: &str = include_str!("../data/dictionaries/ja_romaji_common.tsv");

static SPANISH_SPELL: OnceLock<SymSpell<UnicodeStringStrategy>> = OnceLock::new();
static ENGLISH_SPELL: OnceLock<SymSpell<UnicodeStringStrategy>> = OnceLock::new();
static JAPANESE_ROMAJI_SET: OnceLock<HashSet<String>> = OnceLock::new();

fn get_spanish_spell() -> &'static SymSpell<UnicodeStringStrategy> {
    SPANISH_SPELL.get_or_init(|| {
        let mut symspell = SymSpellBuilder::default()
            .max_dictionary_edit_distance(2)
            .prefix_length(7)
            .count_threshold(1)
            .build()
            .expect("Failed to build Spanish SymSpell");

        for line in ES_FREQ_DICT.lines() {
            symspell.load_dictionary_line(line, 0, 1, " ");
        }
        symspell
    })
}

fn get_english_spell() -> &'static SymSpell<UnicodeStringStrategy> {
    ENGLISH_SPELL.get_or_init(|| {
        let mut symspell = SymSpellBuilder::default()
            .max_dictionary_edit_distance(2)
            .prefix_length(7)
            .count_threshold(1)
            .build()
            .expect("Failed to build English SymSpell");

        for line in EN_FREQ_DICT.lines() {
            symspell.load_dictionary_line(line, 0, 1, " ");
        }
        symspell
    })
}

fn get_japanese_romaji_set() -> &'static HashSet<String> {
    JAPANESE_ROMAJI_SET.get_or_init(|| {
        let mut set = HashSet::new();
        for line in JA_ROMAJI_DICT.lines() {
            let term = line.split('\t').next().unwrap_or("").trim().to_lowercase();
            if !term.is_empty() {
                set.insert(term);
            }
        }
        set
    })
}

pub fn is_cjk_or_japanese(c: char) -> bool {
    matches!(
        c,
        '\u{3040}'..='\u{309F}'
            | '\u{30A0}'..='\u{30FF}'
            | '\u{4E00}'..='\u{9FFF}'
            | '\u{3400}'..='\u{4DBF}'
            | '\u{F900}'..='\u{FAFF}'
            | '\u{FF66}'..='\u{FF9F}'
    )
}

pub fn has_japanese_char(s: &str) -> bool {
    s.chars().any(is_cjk_or_japanese)
}

pub fn is_foreign_or_fantasy(s: &str) -> bool {
    s.contains('\'')
        || s.contains('’')
        || s.contains('_')
        || s.chars().any(|c| c.is_ascii_digit())
        || (s.len() <= 4 && s.chars().filter(|c| c.is_ascii_uppercase()).count() >= 2)
}

pub fn is_japanese_or_romaji(s: &str) -> bool {
    if has_japanese_char(s) {
        return true;
    }
    let lower = s.trim().to_lowercase();
    let romaji_set = get_japanese_romaji_set();
    if romaji_set.contains(&lower) {
        return true;
    }
    let honorific_suffixes = ["-san", "-kun", "-chan", "-sama", "-senpai", "-sensei", "-dono", "-sempai", "-sama"];
    for suffix in honorific_suffixes {
        if lower.ends_with(suffix) {
            return true;
        }
    }
    false
}

fn strip_diacritics(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'Á' | 'À' | 'Ä' | 'Â' => 'A',
            'É' | 'È' | 'Ë' | 'Ê' => 'E',
            'Í' | 'Ì' | 'Ï' | 'Î' => 'I',
            'Ó' | 'Ò' | 'Ö' | 'Ô' => 'O',
            'Ú' | 'Ù' | 'Ü' | 'Û' => 'U',
            _ => c,
        })
        .collect()
}

pub fn match_casing(original: &str, replacement: &str) -> String {
    let mut chars = original.chars().filter(|c| c.is_alphabetic());
    let first_char = chars.next();
    let first_is_upper = first_char.map_or(false, |c| c.is_uppercase());
    let all_upper = original.chars().filter(|c| c.is_alphabetic()).all(|c| c.is_uppercase());

    if all_upper && original.len() > 1 {
        replacement.to_uppercase()
    } else if first_is_upper {
        let mut result = String::new();
        let mut first = true;
        for c in replacement.chars() {
            if first {
                result.extend(c.to_uppercase());
                first = false;
            } else {
                result.push(c);
            }
        }
        result
    } else {
        replacement.to_string()
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpellAuditOptions {
    pub language_mode: String,
    pub ignore_japanese: bool,
    pub ignore_foreign: bool,
    pub ignore_proper_nouns: bool,
    pub ignored_words: Vec<String>,
}

impl Default for SpellAuditOptions {
    fn default() -> Self {
        Self {
            language_mode: "hybrid".to_string(),
            ignore_japanese: true,
            ignore_foreign: true,
            ignore_proper_nouns: true,
            ignored_words: Vec::new(),
        }
    }
}

impl SpellAuditOptions {
    pub fn legacy_parity() -> Self {
        Self {
            language_mode: "es".to_string(),
            ignore_japanese: true,
            ignore_foreign: false,
            ignore_proper_nouns: false,
            ignored_words: Vec::new(),
        }
    }
}

#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpellAuditStats {
    pub ignored_japanese_count: usize,
    pub ignored_foreign_count: usize,
    pub ignored_proper_nouns_count: usize,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DynamicParagraphInput {
    pub id: String,
    pub text: String,
    #[serde(default)]
    pub section_id: String,
    #[serde(default)]
    pub section_title: String,
    #[serde(default)]
    pub page_num: usize,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DynamicAuditRequest {
    pub file_id: String,
    pub paragraphs: Vec<DynamicParagraphInput>,
    pub options: SpellAuditOptions,
}

pub struct ContextualSpellCorrector {
    pub context_window: usize,
    ordinal_re: Regex,
    space_before_punct_re: Regex,
    missing_space_comma_re: Regex,
    midword_caps_re: Regex,
    word_token_re: Regex,
}

impl Default for ContextualSpellCorrector {
    fn default() -> Self {
        Self::new(2)
    }
}

impl ContextualSpellCorrector {
    pub fn new(context_window: usize) -> Self {
        Self {
            context_window,
            ordinal_re: Regex::new(r"(?i)\b(\d+)\s*(da|do|ra|ro|ta|to|va|vo|ma|mo|na|no)\b").unwrap(),
            space_before_punct_re: Regex::new(r"([A-Za-zÀ-ÿ0-9»”』」])\s+([,.:;!?])").unwrap(),
            missing_space_comma_re: Regex::new(r"([A-Za-zÀ-ÿ0-9»”』」]),([A-Za-zÀ-ÿ])").unwrap(),
            midword_caps_re: Regex::new(r"\b([a-záéíóúñ]+)([A-ZÁÉÍÓÚÑ]+[a-záéíóúñ]*)\b").unwrap(),
            word_token_re: Regex::new(r"[A-Za-zÀ-ÿ\-]+").unwrap(),
        }
    }

    pub fn audit_section_paragraphs(
        &self,
        file_id: &str,
        paragraphs: &mut [Paragraph],
        volume_num: usize,
        page_num: usize,
        section_title: &str,
        options: &SpellAuditOptions,
        stats: &mut SpellAuditStats,
        apply_changes: bool,
        starting_index: usize,
    ) -> (Vec<FrontendSpellCorrection>, Vec<SpellTransformation>) {
        let mut transformations: Vec<SpellTransformation> = Vec::new();
        let mut frontend_corrections: Vec<FrontendSpellCorrection> = Vec::new();
        let mut next_idx = starting_index;

        let es_spell = get_spanish_spell();
        let en_spell = get_english_spell();
        let romaji_set = get_japanese_romaji_set();

        let ignored_set: HashSet<String> = options
            .ignored_words
            .iter()
            .map(|w| w.trim().to_lowercase())
            .filter(|w| !w.is_empty())
            .collect();

        let mut dynamic_proper_nouns: HashSet<String> = HashSet::new();
        if options.ignore_proper_nouns {
            let mut title_counts: HashMap<String, usize> = HashMap::new();
            for p in paragraphs.iter() {
                if p.is_part_heading || p.is_scene_break {
                    continue;
                }
                for m in self.word_token_re.find_iter(&p.text) {
                    let w = m.as_str().trim_matches('-');
                    let b = m.start();
                    let is_start = if b == 0 {
                        true
                    } else {
                        let prior = p.text[..b].trim_end();
                        prior.ends_with('.')
                            || prior.ends_with('?')
                            || prior.ends_with('!')
                            || prior.ends_with('—')
                            || prior.ends_with('«')
                            || prior.ends_with('“')
                            || prior.ends_with('《')
                            || prior.ends_with('[')
                            || prior.ends_with('『')
                            || prior.ends_with('「')
                            || prior.ends_with('¿')
                            || prior.ends_with('¡')
                    };
                    if !is_start && w.chars().next().map_or(false, |c| c.is_uppercase()) && w.len() >= 3 {
                        *title_counts.entry(w.to_lowercase()).or_insert(0) += 1;
                    }
                }
            }
            for (term, count) in title_counts {
                if count >= 2 {
                    dynamic_proper_nouns.insert(term);
                }
            }
        }

        for i in 0..paragraphs.len() {
            if paragraphs[i].is_part_heading
                || paragraphs[i].is_scene_break
                || paragraphs[i].text.trim().is_empty()
            {
                continue;
            }

            let original_para = paragraphs[i].text.clone();
            let mut working_para = original_para.clone();

            let start_before = i.saturating_sub(self.context_window);
            let ctx_before: Vec<String> = (start_before..i)
                .filter(|&j| !paragraphs[j].is_part_heading && !paragraphs[j].is_scene_break)
                .map(|j| paragraphs[j].text.clone())
                .collect();

            let end_after = (i + 1 + self.context_window).min(paragraphs.len());
            let ctx_after: Vec<String> = ((i + 1)..end_after)
                .filter(|&j| !paragraphs[j].is_part_heading && !paragraphs[j].is_scene_break)
                .map(|j| paragraphs[j].text.clone())
                .collect();

            let mut para_changes: Vec<(String, String, String, f64, Vec<String>)> = Vec::new();

            for cap in self.ordinal_re.captures_iter(&original_para) {
                let full_match = cap.get(0).unwrap().as_str();
                let num_str = cap.get(1).unwrap().as_str();
                let suffix = cap.get(2).unwrap().as_str().to_lowercase();
                let is_fem = matches!(suffix.as_str(), "da" | "ra" | "ta" | "va" | "ma" | "na");

                let replacement = if is_fem {
                    format!("{}.ª", num_str)
                } else {
                    format!("{}.º", num_str)
                };

                if full_match != replacement {
                    para_changes.push((
                        full_match.to_string(),
                        replacement.clone(),
                        "grammar".to_string(),
                        0.92,
                        vec![replacement],
                    ));
                }
            }

            for cap in self.space_before_punct_re.captures_iter(&original_para) {
                let full_match = cap.get(0).unwrap().as_str();
                let before = cap.get(1).unwrap().as_str();
                let punct = cap.get(2).unwrap().as_str();
                let replacement = format!("{}{}", before, punct);
                para_changes.push((
                    full_match.to_string(),
                    replacement.clone(),
                    "punctuation".to_string(),
                    0.96,
                    vec![replacement],
                ));
            }

            for cap in self.missing_space_comma_re.captures_iter(&original_para) {
                let full_match = cap.get(0).unwrap().as_str();
                let before = cap.get(1).unwrap().as_str();
                let after = cap.get(2).unwrap().as_str();
                let replacement = format!("{}, {}", before, after);
                para_changes.push((
                    full_match.to_string(),
                    replacement.clone(),
                    "punctuation".to_string(),
                    0.95,
                    vec![replacement],
                ));
            }

            for cap in self.midword_caps_re.captures_iter(&original_para) {
                let full_match = cap.get(0).unwrap().as_str();
                let part1 = cap.get(1).unwrap().as_str();
                let part2 = cap.get(2).unwrap().as_str();
                let replacement = format!("{}{}", part1, part2.to_lowercase());
                para_changes.push((
                    full_match.to_string(),
                    replacement.clone(),
                    "casing".to_string(),
                    0.90,
                    vec![replacement],
                ));
            }

            let word_matches: Vec<_> = self.word_token_re.find_iter(&original_para).collect();
            for w_idx in 0..word_matches.len() {
                let m = &word_matches[w_idx];
                let raw_word = m.as_str();
                let word = raw_word.trim_matches('-');
                let start_byte = m.start();

                if w_idx > 0 {
                    let prev_m = &word_matches[w_idx - 1];
                    let prev_word = prev_m.as_str().trim_matches('-');
                    let gap = &original_para[prev_m.end()..start_byte];
                    if gap.trim().is_empty() && word.eq_ignore_ascii_case(prev_word) && word.len() > 1 {
                        let lower_w = word.to_lowercase();
                        let is_sound_or_interjection = matches!(
                            lower_w.as_str(),
                            "ja" | "ha" | "he" | "je" | "ji" | "jo" | "ju" | "hum" | "oh" | "ah" | "ay" | "eh" | "no" | "clan" | "rock"
                        );
                        let is_prep_proper_noun = prev_word.chars().all(|c| c.is_lowercase())
                            && word.chars().next().map_or(false, |c| c.is_uppercase());

                        if !is_sound_or_interjection && !is_prep_proper_noun {
                            let dup_phrase = format!("{} {}", prev_word, word);
                            para_changes.push((
                                dup_phrase,
                                prev_word.to_string(),
                                "grammar".to_string(),
                                0.94,
                                vec![prev_word.to_string()],
                            ));
                        }
                    }
                }

                if word.chars().count() < 2 {
                    continue;
                }

                let chars_vec: Vec<char> = word.to_lowercase().chars().collect();
                if chars_vec.windows(3).any(|w| w[0] == w[1] && w[1] == w[2]) {
                    continue;
                }

                let before_char = original_para[..start_byte].chars().next_back();
                let after_char = original_para[m.end()..].chars().next();
                if matches!(before_char, Some('~') | Some('〜')) || matches!(after_char, Some('~') | Some('〜')) {
                    continue;
                }

                if has_japanese_char(word) {
                    if options.ignore_japanese {
                        stats.ignored_japanese_count += 1;
                        continue;
                    }
                }

                let lower_w = word.to_lowercase();
                if ignored_set.contains(&lower_w) {
                    continue;
                }

                if options.ignore_japanese {
                    if romaji_set.contains(&lower_w) {
                        stats.ignored_japanese_count += 1;
                        continue;
                    }
                    if word.contains('-') {
                        let parts: Vec<&str> = word.split('-').collect();
                        if parts.iter().any(|p| romaji_set.contains(&p.to_lowercase())) {
                            stats.ignored_japanese_count += 1;
                            continue;
                        }
                    }
                }

                if is_foreign_or_fantasy(word) {
                    if options.ignore_foreign {
                        stats.ignored_foreign_count += 1;
                        continue;
                    }
                }

                let is_sentence_start = if start_byte == 0 {
                    true
                } else {
                    let prior = original_para[..start_byte].trim_end();
                    prior.ends_with('.')
                        || prior.ends_with('?')
                        || prior.ends_with('!')
                        || prior.ends_with('—')
                        || prior.ends_with('«')
                        || prior.ends_with('“')
                        || prior.ends_with('¿')
                        || prior.ends_with('¡')
                };

                let first_char = word.chars().next().unwrap();
                let is_capitalized = first_char.is_uppercase();

                if is_capitalized && options.ignore_proper_nouns {
                    if !is_sentence_start {
                        stats.ignored_proper_nouns_count += 1;
                        continue;
                    }
                    if dynamic_proper_nouns.contains(&lower_w) {
                        stats.ignored_proper_nouns_count += 1;
                        continue;
                    }
                }

                let check_es = options.language_mode == "es" || options.language_mode == "hybrid";
                let check_en = options.language_mode == "en" || options.language_mode == "hybrid";

                let mut best_suggestion: Option<(String, usize, f64, String, Vec<String>)> = None;

                if check_es {
                    let suggestions = es_spell.lookup(&lower_w, Verbosity::Closest, 2);
                    if let Some(first) = suggestions.first() {
                        if first.distance == 0 {
                            continue;
                        }
                        let best = suggestions
                            .iter()
                            .find(|s| strip_diacritics(&lower_w) == strip_diacritics(&s.term))
                            .unwrap_or(first);

                        let repl = match_casing(word, &best.term);
                        let mut top_candidates: Vec<String> = Vec::new();
                        top_candidates.push(repl.clone());
                        for s in &suggestions {
                            let cand = match_casing(word, &s.term);
                            if !top_candidates.contains(&cand) && top_candidates.len() < 3 {
                                top_candidates.push(cand);
                            }
                        }

                        let is_accent_only = strip_diacritics(&lower_w) == strip_diacritics(&best.term);
                        let conf = if is_accent_only {
                            0.98
                        } else if best.distance == 1 {
                            0.94
                        } else {
                            0.86
                        };
                        best_suggestion = Some((repl, best.distance as usize, conf, "es".to_string(), top_candidates));
                    }
                }

                if best_suggestion.is_none() && check_en {
                    let suggestions = en_spell.lookup(&lower_w, Verbosity::Top, 2);
                    if let Some(top) = suggestions.first() {
                        if top.distance == 0 {
                            stats.ignored_foreign_count += 1;
                            continue;
                        } else if options.language_mode == "en" {
                            let repl = match_casing(word, &top.term);
                            let mut top_candidates: Vec<String> = Vec::new();
                            top_candidates.push(repl.clone());
                            for s in &suggestions {
                                let cand = match_casing(word, &s.term);
                                if !top_candidates.contains(&cand) && top_candidates.len() < 3 {
                                    top_candidates.push(cand);
                                }
                            }
                            let conf = if top.distance == 1 { 0.93 } else { 0.84 };
                            best_suggestion = Some((repl, top.distance as usize, conf, "en".to_string(), top_candidates));
                        }
                    }
                }

                if best_suggestion.is_none() && options.ignore_japanese && is_japanese_or_romaji(word) {
                    stats.ignored_japanese_count += 1;
                    continue;
                }

                if let Some((repl_word, _distance, conf, _lang, suggs)) = best_suggestion {
                    if repl_word != word {
                        let is_accent = strip_diacritics(&lower_w) == strip_diacritics(&repl_word.to_lowercase());
                        let category = if is_accent {
                            "typo".to_string()
                        } else {
                            "typo".to_string()
                        };
                        para_changes.push((word.to_string(), repl_word, category, conf, suggs));
                    }
                }
            }

            if !para_changes.is_empty() {
                for (orig_t, repl_t, _, _, _) in &para_changes {
                    if working_para.contains(orig_t) {
                        working_para = working_para.replacen(orig_t, repl_t, 1);
                    }
                }

                for (c_idx, (orig_t, repl_t, cat, conf, suggs)) in para_changes.iter().enumerate() {
                    let is_smell = cat == "style";
                    let snippet = if let Some(pos) = original_para.find(orig_t) {
                        let s = pos.saturating_sub(35);
                        let mut s_idx = s;
                        while s_idx > 0 && !original_para.is_char_boundary(s_idx) {
                            s_idx -= 1;
                        }
                        let mut e = (pos + orig_t.len() + 35).min(original_para.len());
                        while e < original_para.len() && !original_para.is_char_boundary(e) {
                            e += 1;
                        }
                        let prefix = if s_idx > 0 { "…" } else { "" };
                        let suffix = if e < original_para.len() { "…" } else { "" };
                        format!("{}{}{}", prefix, &original_para[s_idx..e], suffix)
                    } else {
                        original_para.chars().take(80).collect()
                    };

                    frontend_corrections.push(FrontendSpellCorrection {
                        id: format!("spell-{}-{}-{}-{}", file_id, volume_num, next_idx, c_idx),
                        file_id: file_id.to_string(),
                        volume_num,
                        page_num,
                        section_title: section_title.to_string(),
                        category: cat.clone(),
                        confidence: *conf,
                        is_smell,
                        original_token: orig_t.clone(),
                        replacement_token: repl_t.clone(),
                        suggestions: suggs.clone(),
                        paragraph_id: None,
                        paragraph_before: original_para.clone(),
                        paragraph_after: working_para.clone(),
                        context_snippet: snippet,
                        language: options.language_mode.clone(),
                        enabled: true,
                    });
                }

                let orig_summary = para_changes
                    .iter()
                    .map(|(o, _, _, _, _)| format!("'{}'", o))
                    .collect::<Vec<_>>()
                    .join(", ");
                let repl_summary = para_changes
                    .iter()
                    .map(|(_, r, _, _, _)| format!("'{}'", r))
                    .collect::<Vec<_>>()
                    .join(", ");
                let mut cats = Vec::new();
                for (_, _, c, _, _) in &para_changes {
                    if !cats.contains(c) {
                        cats.push(c.clone());
                    }
                }
                let categories_used = cats.join(" + ");
                let sum_conf: f64 = para_changes.iter().map(|(_, _, _, cf, _)| *cf).sum();
                let avg_conf = ((sum_conf / (para_changes.len() as f64)) * 100.0).round() / 100.0;

                transformations.push(SpellTransformation {
                    index: next_idx,
                    volume_num,
                    page_num,
                    section_title: section_title.to_string(),
                    category: categories_used,
                    confidence: avg_conf,
                    original_token: orig_summary,
                    replacement_token: repl_summary,
                    paragraph_before: original_para.clone(),
                    paragraph_after: working_para.clone(),
                    context_before: ctx_before.join(" "),
                    context_after: ctx_after.join(" "),
                });
                next_idx += 1;

                if apply_changes {
                    paragraphs[i].text = working_para;
                }
            }
        }

        (frontend_corrections, transformations)
    }

    pub fn audit_dynamic_paragraphs(
        &self,
        request: &DynamicAuditRequest,
    ) -> Vec<FrontendSpellCorrection> {
        let mut all_corrections = Vec::new();
        let mut dummy_stats = SpellAuditStats::default();

        for dp in &request.paragraphs {
            let mut paras = vec![Paragraph::new(dp.text.clone())];

            let (mut f_corrs, _) = self.audit_section_paragraphs(
                &request.file_id,
                &mut paras,
                1,
                dp.page_num.max(1),
                &dp.section_title,
                &request.options,
                &mut dummy_stats,
                false,
                1,
            );

            for (c_idx, c) in f_corrs.iter_mut().enumerate() {
                c.id = format!("sp-dyn-{}-{}", dp.id, c_idx);
                c.paragraph_id = Some(dp.id.clone());
                c.section_title = dp.section_title.clone();
            }

            all_corrections.append(&mut f_corrs);
        }

        all_corrections
    }

    pub fn analyze_and_correct_paragraphs(
        &self,
        paragraphs: &mut [Paragraph],
        volume_num: usize,
        page_num: usize,
        section_title: &str,
        apply_changes: bool,
        starting_index: usize,
    ) -> Vec<SpellTransformation> {
        let options = SpellAuditOptions::legacy_parity();
        let mut stats = SpellAuditStats::default();
        let (_, transforms) = self.audit_section_paragraphs(
            "",
            paragraphs,
            volume_num,
            page_num,
            section_title,
            &options,
            &mut stats,
            apply_changes,
            starting_index,
        );
        transforms
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symspell_accent_correction() {
        let corrector = ContextualSpellCorrector::default();
        let options = SpellAuditOptions::default();
        let mut stats = SpellAuditStats::default();

        let mut paras = vec![Paragraph::new(
            "El caballero tambien tenia una espada que habia perdido despues del combate.",
        )];

        let (corrections, _) = corrector.audit_section_paragraphs(
            "test_file",
            &mut paras,
            1,
            1,
            "Capítulo 1",
            &options,
            &mut stats,
            false,
            1,
        );

        let orig_tokens: Vec<&str> = corrections.iter().map(|c| c.original_token.as_str()).collect();
        assert!(orig_tokens.contains(&"tambien"));
        assert!(orig_tokens.contains(&"habia"));
        assert!(orig_tokens.contains(&"despues"));
    }

    #[test]
    fn test_ordinal_and_duplicate_detection() {
        let corrector = ContextualSpellCorrector::default();
        let options = SpellAuditOptions::default();
        let mut stats = SpellAuditStats::default();

        let mut paras = vec![Paragraph::new("En esta 62da batalla de de prueba.")];

        let (corrections, _) = corrector.audit_section_paragraphs(
            "test_file",
            &mut paras,
            1,
            1,
            "Capítulo 1",
            &options,
            &mut stats,
            false,
            1,
        );

        let orig_tokens: Vec<&str> = corrections.iter().map(|c| c.original_token.as_str()).collect();
        assert!(orig_tokens.contains(&"62da"));
        assert!(orig_tokens.contains(&"de de"));
    }

    #[test]
    fn test_japanese_romaji_and_phonotactics() {
        assert!(is_japanese_or_romaji("katana"));
        assert!(is_japanese_or_romaji("shinigami"));
        assert!(is_japanese_or_romaji("senpai"));
        assert!(is_japanese_or_romaji("Tanaka-kun"));
        assert!(!is_japanese_or_romaji("desenvaino"));
    }
}
