use crate::config::{
    DEFAULT_DELIMITER_HIERARCHY, DelimiterHierarchySpec, DelimiterLevel, DocumentProfile,
    HEADER_NOISE_PATTERNS, LEVEL5_TRIPLE_BAR, LanguageProfile, PresetProfile,
    SCENE_BREAK_PATTERNS, STITCHABLE_DELIMITER_PAIRS, SUBSECTION_PATTERNS,
    normalize_cjk_delimiter_variants,
};
use crate::detector::ChapterDetector;
use crate::models::{Footnote, Paragraph, PdfPageData, Section};
use regex::{Captures, Regex, RegexSet};
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;
use unicode_normalization::UnicodeNormalization;

#[derive(Debug, Clone)]
struct RawLineInfo {
    text: String,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
}

pub struct TextCleaner {
    pub language_profile: LanguageProfile,
    pub preset_profile: PresetProfile,
    noise_set: RegexSet,
    scene_set: RegexSet,
    subsection_set: RegexSet,
    pub dynamic_margin_noise: HashSet<String>,
    pub para_break_dy: f64,
    ws_collapse_re: Regex,
    has_letter_re: Regex,
    has_alnum_re: Regex,
    word_num_re: Regex,
    empty_fn_num_re: Regex,
    fn_line_start_re: Regex,
    fn_sup_token_space_re: Regex,
    fn_sup_strip_re: Regex,
    wrap_terminal_punct_re: Regex,
    lower_start_re: Regex,
    dialogue_start_line_re: Regex,
    tn_start_line_re: Regex,
    dialogue_style_re: Regex,
    epistolary_start_re: Regex,
    section_truncate_re: Regex,
    translator_spam_re: Regex,
    lead_tn_split_re: Regex,
    trail_tn_split_re: Regex,
    paren_tn_split_re: Regex,
    standalone_tn_re: Regex,
    sfx_core_re: Regex,
    orphan_ast_before_punct_re: Regex,
    orphan_ast_after_word_re: Regex,
    stitch_lacks_terminal_re: Regex,
    stitch_next_blocked_re: Regex,
    stitch_lower_cont_re: Regex,
    trail_fn_sup_re: Regex,
    mismatch_quote_1_re: Regex,
    mismatch_quote_2_re: Regex,
    paren_thought_re: Regex,
    fullwidth_paren_thought_re: Regex,
    dash_prefix_norm_re: Regex,
    redundant_dash_quote_re: Regex,
    kagi_dialogue_re: Regex,
    pure_quote_dialogue_re: Regex,
    quote_attr_dialogue_re: Regex,
    curly_double_quote_re: Regex,
    straight_double_quote_re: Regex,
    curly_single_quote_re: Regex,
    straight_single_quote_re: Regex,
    spaced_pause_dash_re: Regex,
    multi_bar_norm_re: Regex,
    space_before_triple_bar_re: Regex,
    space_after_triple_bar_re: Regex,
    dialogue_triple_bar_start_re: Regex,
    inner_space_open_double_angle_re: Regex,
    inner_space_close_double_angle_re: Regex,
    inner_space_open_guillemet_re: Regex,
    inner_space_close_guillemet_re: Regex,
    word_before_double_angle_re: Regex,
    word_after_double_angle_re: Regex,
    bracket_inner_quote_re: Regex,
    unopened_title_quote_re: Regex,
    bullet_heading_re: Regex,
    only_ast_re: Regex,
    fn_token_re: Regex,
    fn_quote_norm_re: Regex,
    repeated_sfx_sup_re: Regex,
    angle_kagi_mismatch_re: Regex,
    unclosed_angle_punct_re: Regex,
    angle_bar_re: Regex,
    kagi_quote_mismatch_re: Regex,
    multi_open_guil_re: Regex,
    multi_close_guil_re: Regex,
    multi_open_angle_re: Regex,
    multi_close_angle_re: Regex,
    unclosed_angle_end_re: Regex,
    colon_prefix_re: Regex,
    bracket_tn_re: Regex,
    ast_punct_target_re: Regex,
    ast_word_target_re: Regex,
    mid_quote_space_re: Regex,
}

impl Default for TextCleaner {
    fn default() -> Self {
        Self::new()
    }
}

impl TextCleaner {
    pub fn new() -> Self {
        let narrator_verbs = r"(?:dijo|preguntó|respondió|contestó|exclamó|gritó|susurró|murmuró|pensó|añadió|continuó|interrumpió|ordenó|llamó|habló|repitió|rió|sonrió|asintió|negó|explicó|declaró|anunció|advirtió|rogó|suplicó|gruñó|bramó|jadeó)";

        let noise_pats: Vec<String> = HEADER_NOISE_PATTERNS
            .iter()
            .map(|p| format!("(?i){}", p))
            .collect();
        let sub_pats: Vec<String> = SUBSECTION_PATTERNS
            .iter()
            .map(|p| format!("(?i){}", p))
            .collect();

        Self {
            language_profile: LanguageProfile::default(),
            preset_profile: PresetProfile::default(),
            noise_set: RegexSet::new(noise_pats).unwrap(),
            scene_set: RegexSet::new(SCENE_BREAK_PATTERNS).unwrap(),
            subsection_set: RegexSet::new(sub_pats).unwrap(),
            dynamic_margin_noise: HashSet::new(),
            para_break_dy: 20.5,
            ws_collapse_re: Regex::new(r"\s+").unwrap(),
            has_letter_re: Regex::new(r"[A-Za-zÀ-ÿ]").unwrap(),
            has_alnum_re: Regex::new(r"[A-Za-zÀ-ÿ0-9]").unwrap(),
            word_num_re: Regex::new(r"\b\d+\b").unwrap(),
            empty_fn_num_re: Regex::new(r"^(\d{1,3})$").unwrap(),
            fn_line_start_re: Regex::new(r"^(\d{1,3})(?:\s+(.+)|([A-Za-zÀ-ÿ].*))").unwrap(),
            fn_sup_token_space_re: Regex::new(r"(\[\[(?:FN|SUP):[^\]]+\]\])([A-Za-zÀ-ÿ0-9])")
                .unwrap(),
            fn_sup_strip_re: Regex::new(r"\[\[(?:FN|SUP):[^\]]+\]\]").unwrap(),
            wrap_terminal_punct_re: Regex::new(r#"[.!?…:;”»\]』》」"'—―\*]$"#).unwrap(),
            lower_start_re: Regex::new(r"^[a-záéíóúñ]").unwrap(),
            dialogue_start_line_re: Regex::new(r#"^(?:—|―|--+|-\s*\S|[“"「]|\([“"])"#).unwrap(),
            tn_start_line_re: Regex::new(r"(?i)^\*+\s*(?:Notas?|N\.?T|TN)\b").unwrap(),
            dialogue_style_re: Regex::new(r"^(?:—|―|--+|-\s*\S)").unwrap(),
            epistolary_start_re: Regex::new(r"(?i)^(?:Estimad[oa]|Querid[oa]|Informe|Carta|Fecha:)\b")
                .unwrap(),
            section_truncate_re: Regex::new(
                r"(?i)^(?:Palabras\s+del\s+traductor|Atenci[óo]n|Ilustraciones\s+Adicionales\s+En\s+Alta\s+Resoluci[óo]n|Puede\s+encontrarnos\s+en\s+nuestras\s+p[áa]ginas)\b.*$",
            )
            .unwrap(),
            translator_spam_re: Regex::new(
                r"(?i)(?:^Link\s+(?:si\s+desean\s+apoyar|de\s+la\s+p[áa]gina)\s+(?:al|del)\s+traductor\b|^Si\s+desean\s+leer\s+y\s+saber\s+que\s+sucede|paypal\.me/|patreon\.com/|ko-fi\.com/|discord\.gg/|(?:\s|^)#[A-ZÁÉÍÓÚÜÑ][a-záéíóúüñ]{2,15}\s*:|\b[A-Za-z0-9]+\s+Tra[ns]lations?\b|\bp[áa]gina\s+[A-ZÁÉÍÓÚÜÑ][A-Za-zÀ-ÿ\s]{2,25}\b[Tt]raducciones\b)",
            )
            .unwrap(),
            lead_tn_split_re: Regex::new(
                r"(?i)^(\*+\s*(?:Notas?\s+del\s+tr(?:aductor)?|TN|N\.?T\.?)\s*:?\s*\[[^\]]+\])\s+([A-ZÁÉÍÓÚÑ“«\[].+)$",
            )
            .unwrap(),
            trail_tn_split_re: Regex::new(
                r"(?i)^(.+?)\s+(\*+\s*(?:Notas?\s+del\s+tr(?:aductor)?|TN|N\.?T\.?)\s*:?\s*.+)$",
            )
            .unwrap(),
            paren_tn_split_re: Regex::new(r"^(.+\*.+?)\s+\(\*([^)]+)\)\.?$").unwrap(),
            standalone_tn_re: Regex::new(
                r"(?i)^\*+\s*(?:Notas?\s+del\s+tr(?:aductor)?|TN|N\.?T\.?)\b\s*:?\s*(.+)$",
            )
            .unwrap(),
            sfx_core_re: Regex::new(r"\*[^\*\n]{1,35}\*").unwrap(),
            orphan_ast_before_punct_re: Regex::new(r"\*+([》』”\]\.,!?;:])").unwrap(),
            orphan_ast_after_word_re: Regex::new(r"([A-Za-zÀ-ÿ0-9》』”\])])(\*+)").unwrap(),
            stitch_lacks_terminal_re: Regex::new(r#"[.!?…:;\]»》”』」"'—―\*\)\♪~☆]$"#.replace(r"\♪", "♪").as_str()).unwrap(),
            stitch_next_blocked_re: Regex::new(r#"^(?:—|―|--+|-\s*\S|[“"\[«『「\*]|\([“"])"#)
                .unwrap(),
            stitch_lower_cont_re: Regex::new(r"^[a-záéíóúñ,;#]").unwrap(),
            trail_fn_sup_re: Regex::new(r"((?:\s*\[\[(?:FN|SUP):[^\]]+\]\])+)\s*$").unwrap(),
            mismatch_quote_1_re: Regex::new(r#"[“"]([^”"’'\n]{1,50})[’']"#).unwrap(),
            mismatch_quote_2_re: Regex::new(r#"[‘']([^’'”"\n]{1,50})[”"]"#).unwrap(),
            paren_thought_re: Regex::new(r#"^\(\s*[“"](.+)[”"“]\s*\)\.?$"#).unwrap(),
            fullwidth_paren_thought_re: Regex::new(r"^（\s*([^（）\n]+)\s*）\.?$").unwrap(),
            dash_prefix_norm_re: Regex::new(r"^(\u{E000})?(?:—|―|--+|-\s*)\s*").unwrap(),
            redundant_dash_quote_re: Regex::new(r#"^—[“"](.+)[”"“](\.?)$"#).unwrap(),
            kagi_dialogue_re: Regex::new(r"^「+(.+?)」+(\.?)$").unwrap(),
            pure_quote_dialogue_re: Regex::new(r#"^[“"](.+?)[”"“](\.?)$"#).unwrap(),
            quote_attr_dialogue_re: Regex::new(&format!(
                r#"^[“"]([^”"]+?)[”"],?\s+(—?\s*{}\b.+|[¡¿]?[A-ZÁÉÍÓÚÑ].*)$"#,
                narrator_verbs
            ))
            .unwrap(),
            curly_double_quote_re: Regex::new(r"“([^”\n]+)”").unwrap(),
            straight_double_quote_re: Regex::new(r#""([^"\n]+)""#).unwrap(),
            curly_single_quote_re: Regex::new(r"‘([^’\n]{1,120})’").unwrap(),
            straight_single_quote_re: Regex::new(r"'([^'\n]{1,120})'").unwrap(),
            spaced_pause_dash_re: Regex::new(
                r"([A-Za-zÀ-ÿ0-9…»》])\s+[—―–]\s+([A-Za-zÀ-ÿ0-9¡¿«《])",
            )
            .unwrap(),
            multi_bar_norm_re: Regex::new(r"(?:─+|—{2,}|―{2,}|–{2,})").unwrap(),
            space_before_triple_bar_re: Regex::new(r"\s+───").unwrap(),
            space_after_triple_bar_re: Regex::new(r"───\s+").unwrap(),
            dialogue_triple_bar_start_re: Regex::new(r"^—\s*───\s*").unwrap(),
            inner_space_open_double_angle_re: Regex::new(r"《\s+").unwrap(),
            inner_space_close_double_angle_re: Regex::new(r"\s+》").unwrap(),
            inner_space_open_guillemet_re: Regex::new(r"«\s+").unwrap(),
            inner_space_close_guillemet_re: Regex::new(r"\s+»").unwrap(),
            word_before_double_angle_re: Regex::new(r"([A-Za-zÀ-ÿ0-9])《").unwrap(),
            word_after_double_angle_re: Regex::new(r"》([A-Za-zÀ-ÿ0-9])").unwrap(),
            bracket_inner_quote_re: Regex::new(r#"^\[[«“"]([^»”"\]]+)\]$"#).unwrap(),
            unopened_title_quote_re: Regex::new(
                r#"(\b(?:serializando|publicando|titulado|llamado)\s+)((?:El|La|Los|Las)\s+[A-ZÁÉÍÓÚÜÑ][a-záéíóúüñ]+(?:\s+(?:de(?:\s+la|\s+las|\s+los)?|del|y|en)\s+[A-ZÁÉÍÓÚÜÑ][a-záéíóúüñ]+|\s+[A-ZÁÉÍÓÚÜÑ][a-záéíóúüñ]+){2,})[”»"]"#,
            )
            .unwrap(),
            bullet_heading_re: Regex::new(
                r"(?i)^[u•·▪▸►]\s+((?:Pr[oó]logo|Cap[ií]tulo|Ep[ií]logo|Interludio|Intermisi[oó]n|Palabras|Resumen|Notas?)\b)",
            )
            .unwrap(),
            only_ast_re: Regex::new(r"^\*{1,2}$").unwrap(),
            fn_token_re: Regex::new(r"\[\[FN:\d+\]\]").unwrap(),
            fn_quote_norm_re: Regex::new(r#"[“"”]([^“"”\n]+)[”"]"#).unwrap(),
            repeated_sfx_sup_re: Regex::new(
                r"\b([A-ZÁÉÍÓÚÜÑ][a-záéíóúüñ]{2,})\*\s*([A-ZÁÉÍÓÚÜÑ][a-záéíóúüñ]{2,})\[\[SUP:\*\]\]",
            )
            .unwrap(),
            angle_kagi_mismatch_re: Regex::new(r"《([^《》\n]{1,35})」$").unwrap(),
            unclosed_angle_punct_re: Regex::new(
                r"《([A-ZÁÉÍÓÚÜÑ][A-Za-zÀ-ÿ\s]{1,28}?)(\.{3}|…|\.)",
            )
            .unwrap(),
            angle_bar_re: Regex::new(r"《([^《》\n]{1,35}───)").unwrap(),
            kagi_quote_mismatch_re: Regex::new(r#"^『([^『』\n]+)[”"]$"#).unwrap(),
            multi_open_guil_re: Regex::new(r"«{2,}").unwrap(),
            multi_close_guil_re: Regex::new(r"»{2,}").unwrap(),
            multi_open_angle_re: Regex::new(r"《{2,}").unwrap(),
            multi_close_angle_re: Regex::new(r"》{2,}").unwrap(),
            unclosed_angle_end_re: Regex::new(r"《([^《》\n]{1,35})$").unwrap(),
            colon_prefix_re: Regex::new(r"^:\s*").unwrap(),
            bracket_tn_re: Regex::new(r"^\[+:?\s*(.*?)\s*\]+(.*)$").unwrap(),
            ast_punct_target_re: Regex::new(r"\*+([》』”\])])").unwrap(),
            ast_word_target_re: Regex::new(r"([A-Za-zÀ-ÿ0-9》』”\])])(\*+)").unwrap(),
            mid_quote_space_re: Regex::new(r#"[”"]\s+\S"#).unwrap(),
        }
    }

    pub fn with_profiles(
        language_profile: LanguageProfile,
        preset_profile: PresetProfile,
    ) -> Self {
        let mut cleaner = Self::new();
        cleaner.language_profile = language_profile;
        cleaner.preset_profile = preset_profile;
        cleaner
    }

    pub fn set_profiles(
        &mut self,
        language_profile: LanguageProfile,
        preset_profile: PresetProfile,
    ) {
        self.language_profile = language_profile;
        self.preset_profile = preset_profile;
    }

    pub fn extract_raw_sections(
        &mut self,
        pages: &[PdfPageData],
        detector: &ChapterDetector,
    ) -> (
        usize,
        Vec<(Section, Vec<(usize, Paragraph)>, Vec<Footnote>)>,
    ) {
        let profile = DocumentProfile::from_pages(pages, "");
        self.extract_raw_sections_with_profile(pages, detector, &profile)
    }

    pub fn extract_raw_sections_with_profile(
        &mut self,
        pages: &[PdfPageData],
        detector: &ChapterDetector,
        profile: &DocumentProfile,
    ) -> (
        usize,
        Vec<(Section, Vec<(usize, Paragraph)>, Vec<Footnote>)>,
    ) {
        let total_pages = pages.len();
        self.learn_document_margin_noise(pages);
        let sections = detector.detect_sections_with_profile(pages, profile);

        let mut raw_sections_list = Vec::with_capacity(sections.len());
        for sec in sections {
            let mut raw_pages_data = Vec::new();
            for pno in sec.start_page..=sec.end_page {
                if pno == 0 || pno > pages.len() {
                    continue;
                }
                let page = &pages[pno - 1];
                let p_min_y = if pno == sec.start_page { sec.start_y } else { 0.0 };
                let p_max_y = if pno == sec.end_page { sec.end_y } else { None };
                let (paras, page_footnotes) =
                    self.clean_page_with_profile(page, p_min_y, p_max_y, Some(profile));
                raw_pages_data.push((pno, paras, page_footnotes));
            }
            let (stitched_pairs, sec_footnotes) = self.finalize_section_paragraphs(raw_pages_data);
            raw_sections_list.push((sec, stitched_pairs, sec_footnotes));
        }

        (total_pages, raw_sections_list)
    }

    pub fn learn_document_margin_noise(&mut self, pages: &[PdfPageData]) {
        self.dynamic_margin_noise.clear();
        let mut margin_counts: HashMap<String, usize> = HashMap::new();
        let mut all_dys: Vec<f64> = Vec::new();
        let total_pages = pages.len();

        for (pno, page) in pages.iter().enumerate() {
            let h = page.height;
            let mut page_seen: HashSet<String> = HashSet::new();
            let mut page_lines_xy: Vec<(f64, f64)> = Vec::new();

            for b in &page.blocks {
                if b.block_type != 0 {
                    continue;
                }
                for l in &b.lines {
                    let y0 = l.bbox.1;
                    let mut raw_txt: String = l.spans.iter().map(|s| s.text.as_str()).collect();
                    raw_txt = self.ws_collapse_re.replace_all(raw_txt.trim(), " ").to_string();

                    if !raw_txt.is_empty() && (5..total_pages.min(40)).contains(&pno) {
                        page_lines_xy.push((l.bbox.0, y0));
                    }

                    if y0 < h * 0.09 || y0 > h * 0.88 {
                        if raw_txt.chars().count() < 4 {
                            continue;
                        }
                        if !self.has_letter_re.is_match(&raw_txt) {
                            continue;
                        }
                        if self.is_scene_break(&raw_txt) || self.is_subsection(&raw_txt) {
                            continue;
                        }
                        if raw_txt.starts_with('—')
                            || raw_txt.starts_with('―')
                            || raw_txt.starts_with('-')
                            || raw_txt.starts_with('“')
                            || raw_txt.starts_with('"')
                            || raw_txt.starts_with('[')
                            || raw_txt.starts_with('«')
                            || raw_txt.starts_with('¡')
                            || raw_txt.starts_with('¿')
                        {
                            continue;
                        }
                        if raw_txt.ends_with('.') && raw_txt.split_whitespace().count() <= 4 {
                            continue;
                        }
                        let norm_txt = self.word_num_re.replace_all(&raw_txt, "<NUM>").to_string();
                        page_seen.insert(norm_txt);
                    }
                }
            }

            if !page_lines_xy.is_empty() {
                page_lines_xy.sort_by(|a, b| {
                    let qa = (a.1 / 4.0).round() as i64;
                    let qb = (b.1 / 4.0).round() as i64;
                    qa.cmp(&qb).then_with(|| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
                });
                for idx in 1..page_lines_xy.len() {
                    let dy_val = ((page_lines_xy[idx].1 - page_lines_xy[idx - 1].1) * 10.0).round() / 10.0;
                    if (10.0..=30.0).contains(&dy_val) {
                        all_dys.push(dy_val);
                    }
                }
            }

            for norm_item in page_seen {
                *margin_counts.entry(norm_item).or_insert(0) += 1;
            }
        }

        for (norm_item, count) in margin_counts {
            if count >= 4 {
                self.dynamic_margin_noise.insert(norm_item);
            }
        }

        if !all_dys.is_empty() {
            all_dys.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let idx = ((all_dys.len() as f64) * 0.20) as usize;
            let base_dy = all_dys[idx.min(all_dys.len() - 1)];
            let cand = (base_dy * 1.22).max(base_dy + 3.2);
            self.para_break_dy = (cand * 100.0).round() / 100.0;
        } else {
            self.para_break_dy = 20.5;
        }
    }

    pub fn merge_bold_tokens(&self, s: &str) -> String {
        crate::text_utils::merge_bold_tokens(s)
    }

    pub fn is_noise_line(
        &self,
        line: &str,
        y0: f64,
        page_height: f64,
        profile: Option<&DocumentProfile>,
    ) -> bool {
        let stripped = crate::text_utils::strip_formatting_tokens(line);
        let stripped = stripped.trim();
        if stripped.is_empty() {
            return true;
        }

        let in_margin = y0 < page_height * 0.09 || y0 > page_height * 0.88;
        if in_margin {
            if stripped.chars().all(|c| c.is_ascii_digit()) {
                return true;
            }
            let ws_norm = self.ws_collapse_re.replace_all(stripped, " ");
            let norm_stripped = self.word_num_re.replace_all(&ws_norm, "<NUM>").to_string();
            if self.dynamic_margin_noise.contains(&norm_stripped) {
                return true;
            }
        }

        if self.noise_set.is_match(stripped) {
            return true;
        }

        if let Some(prof) = profile {
            if prof.is_margin_or_banner_noise(stripped) {
                return true;
            }
        }

        false
    }

    pub fn is_scene_break(&self, line: &str) -> bool {
        if !self.preset_profile.enable_scene_breaks {
            return false;
        }
        let stripped = crate::text_utils::strip_formatting_tokens(line);
        let trimmed = stripped.trim();
        self.scene_set.is_match(trimmed)
    }

    pub fn is_subsection(&self, line: &str) -> bool {
        let stripped = crate::text_utils::strip_formatting_tokens(line);
        let trimmed = stripped.trim();
        self.subsection_set.is_match(trimmed)
    }

    pub fn clean_page(
        &self,
        page: &PdfPageData,
        min_y: f64,
        max_y: Option<f64>,
    ) -> (Vec<Paragraph>, Vec<Footnote>) {
        self.clean_page_with_profile(page, min_y, max_y, None)
    }

    pub fn clean_page_with_profile(
        &self,
        page: &PdfPageData,
        min_y: f64,
        max_y: Option<f64>,
        profile: Option<&DocumentProfile>,
    ) -> (Vec<Paragraph>, Vec<Footnote>) {
        let h = page.height;
        let w = page.width;
        let mut footnotes: Vec<Footnote> = Vec::new();
        let mut page_fn_order: Vec<String> = Vec::new();
        let mut page_fn_map: HashMap<String, String> = HashMap::new();
        let mut empty_fn_nums: HashSet<String> = HashSet::new();

        for b in &page.blocks {
            if b.block_type != 0 {
                continue;
            }
            let y0 = b.bbox.1;
            if y0 > h * 0.75 {
                let mut is_fn_block = false;
                let mut current_num: Option<String> = None;
                let mut current_text: Vec<String> = Vec::new();

                for l in &b.lines {
                    let raw_line_text: String = l
                        .spans
                        .iter()
                        .map(|s| s.text.as_str())
                        .collect::<String>()
                        .nfc()
                        .collect::<String>();
                    let line_text = normalize_cjk_delimiter_variants(raw_line_text.trim());
                    if line_text.is_empty() {
                        continue;
                    }
                    let first_size = l.spans.first().map(|s| s.size).unwrap_or(11.0);
                    if let Some(caps) = self.empty_fn_num_re.captures(&line_text) {
                        if first_size < 9.5 || y0 > h * 0.80 {
                            if is_fn_block {
                                if let Some(prev_num) = current_num.take() {
                                    let joined_prev = current_text.join(" ").trim().to_string();
                                    if self.has_alnum_re.is_match(&joined_prev) {
                                        if !page_fn_map.contains_key(&prev_num) {
                                            page_fn_order.push(prev_num.clone());
                                        }
                                        page_fn_map.insert(prev_num, joined_prev);
                                    } else {
                                        empty_fn_nums.insert(prev_num);
                                    }
                                }
                            }
                            current_text.clear();
                            empty_fn_nums.insert(caps[1].to_string());
                            continue;
                        }
                    }

                    if let Some(caps) = self.fn_line_start_re.captures(&line_text) {
                        if first_size < 9.5 || y0 > h * 0.80 {
                            is_fn_block = true;
                            if let Some(prev_num) = current_num.take() {
                                let joined_prev = current_text.join(" ").trim().to_string();
                                if self.has_alnum_re.is_match(&joined_prev) {
                                    if !page_fn_map.contains_key(&prev_num) {
                                        page_fn_order.push(prev_num.clone());
                                    }
                                    page_fn_map.insert(prev_num, joined_prev);
                                } else {
                                    empty_fn_nums.insert(prev_num);
                                }
                            }
                            current_num = Some(caps[1].to_string());
                            let body = caps
                                .get(2)
                                .or_else(|| caps.get(3))
                                .map(|m| m.as_str().trim().to_string())
                                .unwrap_or_default();
                            current_text = vec![body];
                            continue;
                        }
                    }

                    if is_fn_block && current_num.is_some() {
                        current_text.push(line_text);
                    }
                }

                if is_fn_block {
                    if let Some(prev_num) = current_num.take() {
                        let joined_last = current_text.join(" ").trim().to_string();
                        if self.has_alnum_re.is_match(&joined_last) {
                            if !page_fn_map.contains_key(&prev_num) {
                                page_fn_order.push(prev_num.clone());
                            }
                            page_fn_map.insert(prev_num, joined_last);
                        } else {
                            empty_fn_nums.insert(prev_num);
                        }
                    }
                }
            }
        }

        for num in &page_fn_order {
            if let Some(text) = page_fn_map.get(num) {
                footnotes.push(Footnote {
                    id: format!("fn-{}", num),
                    number: num.clone(),
                    text: text.clone(),
                });
            }
        }

        let mut all_fn_nums: HashSet<String> = page_fn_map.keys().cloned().collect();
        all_fn_nums.extend(empty_fn_nums);

        let mut raw_lines: Vec<RawLineInfo> = Vec::new();
        for b in &page.blocks {
            if b.block_type != 0 {
                continue;
            }
            let y0 = b.bbox.1;

            if y0 > h * 0.75 && !all_fn_nums.is_empty() {
                let raw_block_text: String = b
                    .lines
                    .iter()
                    .flat_map(|l| l.spans.iter().map(|s| s.text.as_str()))
                    .collect::<String>()
                    .trim()
                    .to_string();
                let is_fn_blk = all_fn_nums.iter().any(|num| {
                    if let Some(rest) = raw_block_text.strip_prefix(num.as_str()) {
                        rest.is_empty()
                            || rest.starts_with(char::is_whitespace)
                            || rest.chars().next().is_some_and(char::is_alphabetic)
                    } else {
                        false
                    }
                });
                if is_fn_blk {
                    continue;
                }
            }

            for l in &b.lines {
                let line_y0 = l.bbox.1;
                if line_y0 < min_y {
                    continue;
                }
                if let Some(max_val) = max_y {
                    if line_y0 > max_val {
                        continue;
                    }
                }

                let mut spans_text: Vec<String> = Vec::new();
                for s in &l.spans {
                    let txt: String = s.text.nfc().collect();
                    let s_str = txt.trim();
                    let leading_ws = if txt.chars().next().is_some_and(char::is_whitespace) {
                        " "
                    } else {
                        ""
                    };
                    let trailing_ws = if txt.chars().last().is_some_and(char::is_whitespace) {
                        " "
                    } else {
                        ""
                    };
                    let has_super_flag = (s.flags & 1) != 0;
                    let has_bold_flag = (s.flags & 16) != 0;
                    let is_digits = !s_str.is_empty() && s_str.chars().all(|c| c.is_ascii_digit());
                    let is_tiny_digit = s.size < 8.5 && is_digits;
                    if is_digits && (has_super_flag || is_tiny_digit) {
                        if page_fn_map.contains_key(s_str) {
                            spans_text.push(format!("{}[[FN:{}]]{}", leading_ws, s_str, trailing_ws));
                        } else if !leading_ws.is_empty() || !trailing_ws.is_empty() {
                            spans_text.push(" ".to_string());
                        }
                    } else if has_super_flag && !s_str.is_empty() && !is_digits {
                        spans_text.push(format!("{}[[SUP:{}]]{}", leading_ws, s_str, trailing_ws));
                    } else if has_bold_flag && !s_str.is_empty() {
                        spans_text.push(format!("{}\u{E000}{}\u{E001}{}", leading_ws, s_str, trailing_ws));
                    } else {
                        spans_text.push(txt);
                    }
                }

                let mut line_str = normalize_cjk_delimiter_variants(&spans_text.join(""));
                if line_str.contains('\u{E000}') || line_str.contains('\u{E001}') {
                    line_str = self.merge_bold_tokens(&line_str);
                }
                if line_str.contains("[[") {
                    line_str = self
                        .fn_sup_token_space_re
                        .replace_all(&line_str, "$1 $2")
                        .to_string();
                }
                line_str = self
                    .ws_collapse_re
                    .replace_all(&line_str, " ")
                    .trim()
                    .to_string();
                if line_str.contains('\u{E000}') || line_str.contains('\u{E001}') {
                    line_str = self.merge_bold_tokens(&line_str);
                }
                line_str = self
                    .bullet_heading_re
                    .replace(&line_str, "$1")
                    .to_string();
                if line_str.is_empty() || self.only_ast_re.is_match(&line_str) {
                    continue;
                }
                if self.is_noise_line(&line_str, line_y0, h, profile) {
                    continue;
                }

                raw_lines.push(RawLineInfo {
                    text: line_str,
                    x0: l.bbox.0,
                    y0: line_y0,
                    x1: l.bbox.2,
                    y1: l.bbox.3,
                });
            }
        }

        raw_lines.sort_by(|a, b| {
            let qa = (a.y0 / 4.0).round() as i64;
            let qb = (b.y0 / 4.0).round() as i64;
            qa.cmp(&qb).then_with(|| a.x0.partial_cmp(&b.x0).unwrap_or(std::cmp::Ordering::Equal))
        });

        let mut merged_raw_lines: Vec<RawLineInfo> = Vec::new();
        for item in raw_lines {
            if let Some(prev) = merged_raw_lines.last_mut() {
                if (item.y0 - prev.y0).abs() <= 3.0 && item.x0 >= prev.x0 {
                    let mut comb = format!("{} {}", prev.text, item.text);
                    if comb.contains('\u{E000}') || comb.contains('\u{E001}') {
                        comb = self.merge_bold_tokens(&comb);
                    }
                    prev.text = comb;
                    prev.x1 = prev.x1.max(item.x1);
                    prev.y0 = prev.y0.min(item.y0);
                    prev.y1 = prev.y1.max(item.y1);
                    continue;
                }
            }
            merged_raw_lines.push(item);
        }
        let raw_lines = merged_raw_lines;

        let mut paragraphs: Vec<Paragraph> = Vec::new();
        let mut curr_lines: Vec<String> = Vec::new();

        for i in 0..raw_lines.len() {
            let line_info = &raw_lines[i];
            let txt = &line_info.text;

            if self.is_subsection(txt) {
                if !curr_lines.is_empty() {
                    self.flush_paragraph(&curr_lines, &mut paragraphs);
                    curr_lines.clear();
                }
                let sub_title = txt.trim_end_matches('.').to_string();
                paragraphs.push(Paragraph::part_heading(sub_title));
                continue;
            }

            if self.is_scene_break(txt) {
                if !curr_lines.is_empty() {
                    self.flush_paragraph(&curr_lines, &mut paragraphs);
                    curr_lines.clear();
                }
                let cleaned_txt = crate::text_utils::strip_formatting_tokens(txt);
                let trimmed_txt = cleaned_txt.trim();
                paragraphs.push(Paragraph::scene_break_with_text(trimmed_txt));
                continue;
            }

            if curr_lines.is_empty() {
                curr_lines.push(txt.clone());
                continue;
            }

            let prev_info = &raw_lines[i - 1];
            let prev_txt = curr_lines.last().unwrap().clone();
            let joined_prev = self.join_lines(&curr_lines);
            let dy = line_info.y0 - prev_info.y0;

            let unclosed_cjk = ((joined_prev.matches('《').count() > joined_prev.matches('》').count())
                || (joined_prev.matches('『').count() > joined_prev.matches('』').count())
                || (joined_prev.matches('[').count() > joined_prev.matches(']').count()
                    && txt.contains(']')
                    && !txt.contains('[')
                    && (self.tn_start_line_re.is_match(&joined_prev)
                        || joined_prev.starts_with('['))))
                && !(prev_txt.ends_with('”')
                    || prev_txt.ends_with('"')
                    || prev_txt.ends_with('』')
                    || prev_txt.ends_with('」'))
                && !(txt.starts_with('“')
                    || txt.starts_with('"')
                    || txt.starts_with('『')
                    || txt.starts_with('「')
                    || txt.starts_with('—')
                    || txt.starts_with('['));

            let txt_clean = txt.strip_prefix('\u{E000}').unwrap_or(txt);
            let prev_clean = prev_txt.strip_suffix('\u{E001}').unwrap_or(&prev_txt);
            let wrapped_without_punct = prev_info.x1 >= w * 0.74
                && !self.wrap_terminal_punct_re.is_match(prev_clean)
                && (self.lower_start_re.is_match(txt_clean)
                    || txt_clean.starts_with('》')
                    || txt_clean.starts_with('』')
                    || txt_clean.starts_with(']')
                    || txt_clean.starts_with('»')
                    || txt_clean.starts_with('”')
                    || txt_clean.starts_with('」'));

            let is_dialogue_start = self.dialogue_start_line_re.is_match(txt_clean)
                && !(joined_prev.matches('“').count() > joined_prev.matches('”').count()
                    || joined_prev.matches('「').count() > joined_prev.matches('」').count());
            let is_bracket_start = txt_clean.starts_with('[');
            let is_thought_start = (txt_clean.starts_with('«')
                && !(joined_prev.matches('«').count() > joined_prev.matches('»').count()))
                || (txt_clean.starts_with('『')
                    && !(joined_prev.matches('『').count() > joined_prev.matches('』').count()));
            let is_tn_start = self.tn_start_line_re.is_match(txt_clean);

            if !(unclosed_cjk || wrapped_without_punct)
                && (dy > self.para_break_dy
                    || is_dialogue_start
                    || is_bracket_start
                    || is_thought_start
                    || is_tn_start)
            {
                self.flush_paragraph(&curr_lines, &mut paragraphs);
                curr_lines = vec![txt.clone()];
            } else {
                let last_line = curr_lines.last_mut().unwrap();
                if !crate::text_utils::try_merge_hyphenated_line(last_line, txt) {
                    curr_lines.push(txt.clone());
                }
            }
        }

        if !curr_lines.is_empty() {
            self.flush_paragraph(&curr_lines, &mut paragraphs);
        }

        for fn_item in &footnotes {
            let token = format!("[[FN:{}]]", fn_item.number);
            let mut matched_idx = paragraphs.iter().position(|p| p.text.contains(&token));

            if matched_idx.is_none() {
                let fn_inline_pat = Regex::new(&format!(
                    r"\b([a-zA-ZáéíóúÁÉÍÓÚñÑ\]>》”»]+)({})\b",
                    regex::escape(&fn_item.number)
                ))
                .unwrap();
                for (idx, p) in paragraphs.iter_mut().enumerate() {
                    if p.is_part_heading || p.is_scene_break {
                        continue;
                    }
                    if fn_inline_pat.is_match(&p.text) {
                        p.text = fn_inline_pat
                            .replace(&p.text, format!("$1{}", token))
                            .to_string();
                        matched_idx = Some(idx);
                        break;
                    }
                }
            }

            if let Some(idx) = matched_idx {
                paragraphs[idx].footnotes.push(fn_item.clone());
            }
        }

        (paragraphs, footnotes)
    }

    fn flush_paragraph(&self, lines: &[String], paragraphs: &mut Vec<Paragraph>) {
        let merged = self.join_lines(lines);
        if merged.is_empty() {
            return;
        }

        if self.is_subsection(&merged) {
            paragraphs.push(Paragraph::part_heading(merged.trim_end_matches('.')));
            return;
        }

        if self.is_scene_break(&merged) {
            let cleaned_merged = crate::text_utils::strip_formatting_tokens(&merged);
            let trimmed_merged = cleaned_merged.trim();
            paragraphs.push(Paragraph::scene_break_with_text(trimmed_merged));
            return;
        }

        if merged.starts_with('“') && merged.contains("” “") {
            let sub_parts = split_adjacent_curly_quotes(&merged);
            if sub_parts.len() > 1 && sub_parts.iter().all(|sp| sp.starts_with('“')) {
                for sp in sub_parts {
                    let s_trim = sp.trim().to_string();
                    self.append_classified_paragraph(&s_trim, &[s_trim.clone()], paragraphs);
                }
                return;
            }
        }

        if merged.starts_with('[')
            && merged.matches('[').count() > merged.matches(']').count()
            && merged.contains(" [")
        {
            let sub_parts = split_before_open_bracket(&merged);
            if sub_parts.len() > 1 && sub_parts.iter().all(|sp| sp.starts_with('[')) {
                for sp in sub_parts {
                    let mut sp_clean = sp.trim().to_string();
                    if sp_clean.starts_with('[') && !sp_clean.contains(']') {
                        sp_clean.push(']');
                    }
                    self.append_classified_paragraph(&sp_clean, &[sp_clean.clone()], paragraphs);
                }
                return;
            }
        }

        self.append_classified_paragraph(&merged, lines, paragraphs);
    }

    fn append_classified_paragraph(
        &self,
        merged: &str,
        lines: &[String],
        paragraphs: &mut Vec<Paragraph>,
    ) {
        let style = self.classify_paragraph_style(merged, Some(lines));
        if style == "verse_or_chant" {
            paragraphs.push(Paragraph::with_style(lines.join("\n"), "verse_or_chant"));
        } else {
            paragraphs.push(Paragraph::with_style(merged, style));
        }
    }

    pub fn classify_paragraph_style(&self, text: &str, lines: Option<&[String]>) -> String {
        let clean_t = if text.contains("[[") {
            std::borrow::Cow::Owned(self.fn_sup_strip_re.replace_all(text, "").trim().to_string())
        } else {
            std::borrow::Cow::Borrowed(text.trim())
        };
        let clean_t = if clean_t.contains('\u{E000}') || clean_t.contains('\u{E001}') {
            std::borrow::Cow::Owned(crate::text_utils::strip_formatting_tokens(&clean_t).trim().to_string())
        } else {
            clean_t
        };

        if self.dialogue_style_re.is_match(&clean_t) {
            return "dialogue".to_string();
        }
        if !self.language_profile.use_em_dash_dialogue
            && (clean_t.starts_with('“') || clean_t.starts_with('"'))
        {
            return "dialogue".to_string();
        }
        if self.preset_profile.enable_lit_rpg_brackets
            && clean_t.starts_with('[')
            && clean_t.ends_with(']')
        {
            return "bracket_speech".to_string();
        }
        if clean_t.starts_with('«') && clean_t.ends_with('»') {
            return "thought".to_string();
        }
        if let Some(lns) = lines {
            let max_len = lns.iter().map(|l| l.chars().count()).max().unwrap_or(0);
            let cond_len = lns.len() >= 3
                || (lns.len() == 2
                    && max_len < 35
                    && lns
                        .last()
                        .is_some_and(|last| self.wrap_terminal_punct_re.is_match(last)));
            let all_short = lns.iter().all(|l| l.chars().count() < 45);
            let starts_special = clean_t.starts_with('—')
                || clean_t.starts_with('«')
                || clean_t.starts_with('[')
                || clean_t.starts_with('“')
                || clean_t.starts_with('『')
                || clean_t.starts_with('「')
                || clean_t.starts_with('*');
            if cond_len && all_short && !starts_special {
                return "verse_or_chant".to_string();
            }
        }
        if self.epistolary_start_re.is_match(&clean_t) {
            return "epistolary_doc".to_string();
        }
        "narrative".to_string()
    }

    fn join_lines(&self, lines: &[String]) -> String {
        if lines.is_empty() {
            return String::new();
        }
        if lines.len() == 1 {
            return self.merge_bold_tokens(lines[0].trim());
        }
        let total_len: usize = lines.iter().map(|l| l.len() + 1).sum();
        let mut result: Vec<String> = Vec::with_capacity(lines.len());
        for line in lines {
            if result.is_empty() {
                result.push(line.clone());
            } else {
                let last = result.last_mut().unwrap();
                if !crate::text_utils::try_merge_hyphenated_line(last, line) {
                    if last.ends_with('—')
                        || last.ends_with('―')
                        || last.ends_with('《')
                        || last.ends_with('『')
                    {
                        result.push(line.clone());
                    } else {
                        result.push(format!(" {}", line));
                    }
                }
            }
        }
        let mut joined = String::with_capacity(total_len);
        for s in &result {
            joined.push_str(s);
        }
        self.merge_bold_tokens(joined.trim())
    }

    pub fn finalize_section_paragraphs(
        &self,
        pages_data: Vec<(usize, Vec<Paragraph>, Vec<Footnote>)>,
    ) -> (Vec<(usize, Paragraph)>, Vec<Footnote>) {
        let mut flat: Vec<(usize, Paragraph)> = Vec::new();
        let mut section_truncated = false;

        for (pno, paras, _) in pages_data {
            if section_truncated {
                break;
            }
            for p in paras {
                let t_strip = p.text.trim();
                if t_strip.is_empty() {
                    continue;
                }
                if self.section_truncate_re.is_match(t_strip) {
                    section_truncated = true;
                    break;
                }
                if self.translator_spam_re.is_match(t_strip) {
                    continue;
                }

                if !p.is_part_heading && !p.is_scene_break && t_strip.contains('*') {
                    if let Some(caps) = self.lead_tn_split_re.captures(t_strip) {
                        let tn_part = caps[1].trim().to_string();
                        let narr_part = caps[2].trim().to_string();
                        let narr_style = self.classify_paragraph_style(&narr_part, None);
                        flat.push((pno, Paragraph::with_style(tn_part, "narrative")));
                        flat.push((
                            pno,
                            Paragraph {
                                text: narr_part,
                                is_scene_break: false,
                                is_part_heading: false,
                                style: narr_style,
                                footnotes: p.footnotes.clone(),
                            },
                        ));
                        continue;
                    }

                    if let Some(caps) = self.trail_tn_split_re.captures(t_strip) {
                        let main_part = caps[1].trim().to_string();
                        let tn_part = caps[2].trim().to_string();
                        let main_style = self.classify_paragraph_style(&main_part, None);
                        flat.push((
                            pno,
                            Paragraph {
                                text: main_part,
                                is_scene_break: false,
                                is_part_heading: false,
                                style: main_style,
                                footnotes: p.footnotes.clone(),
                            },
                        ));
                        flat.push((pno, Paragraph::with_style(tn_part, "narrative")));
                        continue;
                    }

                    if let Some(caps) = self.paren_tn_split_re.captures(t_strip) {
                        let main_part = caps[1].trim().to_string();
                        let tn_part = format!("*Nota del tr: [{}]", caps[2].trim());
                        let main_style = self.classify_paragraph_style(&main_part, None);
                        flat.push((
                            pno,
                            Paragraph {
                                text: main_part,
                                is_scene_break: false,
                                is_part_heading: false,
                                style: main_style,
                                footnotes: p.footnotes.clone(),
                            },
                        ));
                        flat.push((pno, Paragraph::with_style(tn_part, "narrative")));
                        continue;
                    }
                }

                flat.push((pno, p));
            }
        }

        let mut merged_tn_flat: Vec<(usize, Paragraph)> = Vec::new();
        let mut idx_f = 0;
        while idx_f < flat.len() {
            let (pno_f, mut p_f) = flat[idx_f].clone();
            let t_f = p_f.text.trim().to_string();
            if t_f.starts_with('*')
                && self.standalone_tn_re.is_match(&t_f)
                && !p_f.is_part_heading
                && !p_f.is_scene_break
                && t_f.matches('[').count() > t_f.matches(']').count()
            {
                let mut comb_txt = t_f;
                if idx_f + 1 < flat.len() {
                    let next_p = &flat[idx_f + 1].1;
                    let next_t = next_p.text.trim();
                    if !next_p.is_part_heading
                        && !next_p.is_scene_break
                        && next_t.contains(']')
                        && !next_t.contains('[')
                    {
                        comb_txt = self.join_two_texts(&comb_txt, next_t);
                        idx_f += 1;
                    }
                }
                p_f.text = comb_txt;
            }
            merged_tn_flat.push((pno_f, p_f));
            idx_f += 1;
        }
        let flat = merged_tn_flat;

        let mut converted_flat: Vec<(usize, Paragraph)> = Vec::new();
        let mut synthetic_counter: usize = 500;

        for (pno, p) in flat {
            let t_strip = p.text.trim();
            let m_tn = if t_strip.starts_with('*') {
                self.standalone_tn_re.captures(t_strip)
            } else {
                None
            };
            if let Some(caps) = m_tn {
                if !p.is_part_heading && !p.is_scene_break {
                    let mut raw_body = caps[1].trim().to_string();
                    raw_body = self.colon_prefix_re.replace(&raw_body, "").to_string();
                    if let Some(br_caps) = self.bracket_tn_re.captures(&raw_body) {
                        let inner = br_caps[1].trim();
                        let tail = br_caps[2].trim();
                        raw_body = if !tail.is_empty() {
                            format!("{} {}", inner, tail).trim().to_string()
                        } else {
                            inner.to_string()
                        };
                    } else if raw_body.starts_with('[') && !raw_body.contains(']') {
                        raw_body = raw_body.trim_start_matches('[').trim().to_string();
                    }
                    raw_body = self.colon_prefix_re.replace(&raw_body, "").trim().to_string();
                    if raw_body.is_empty() {
                        continue;
                    }
                    synthetic_counter += 1;
                    let fn_num_str = synthetic_counter.to_string();
                    let fn_obj = Footnote {
                        id: format!("fn-{}", fn_num_str),
                        number: fn_num_str.clone(),
                        text: format!("N.T.: {}", raw_body),
                    };

                    let mut target_idx: Option<usize> = None;
                    let min_back = converted_flat.len().saturating_sub(4);
                    for back_idx in (min_back..converted_flat.len()).rev() {
                        let cand_p = &converted_flat[back_idx].1;
                        if cand_p.is_part_heading || cand_p.is_scene_break {
                            continue;
                        }
                        if has_word_asterisk(&cand_p.text, &self.ast_word_target_re)
                            || cand_p.text.contains("[[SUP:*]]")
                            || self.ast_punct_target_re.is_match(&cand_p.text)
                        {
                            target_idx = Some(back_idx);
                            break;
                        }
                    }

                    if target_idx.is_none() {
                        for back_idx in (0..converted_flat.len()).rev() {
                            let cand_p = &converted_flat[back_idx].1;
                            if !cand_p.is_part_heading && !cand_p.is_scene_break {
                                target_idx = Some(back_idx);
                                break;
                            }
                        }
                    }

                    if let Some(t_idx) = target_idx {
                        let target_para = &mut converted_flat[t_idx].1;
                        if target_para.text.contains("[[SUP:*]]") {
                            target_para.text = target_para.text.replacen(
                                "[[SUP:*]]",
                                &format!("[[FN:{}]]", fn_num_str),
                                1,
                            );
                        } else if self.ast_punct_target_re.is_match(&target_para.text) {
                            target_para.text = self
                                .ast_punct_target_re
                                .replace(&target_para.text, format!("$1[[FN:{}]]", fn_num_str))
                                .to_string();
                        } else if has_word_asterisk(&target_para.text, &self.ast_word_target_re) {
                            target_para.text = replace_first_word_asterisk(
                                &target_para.text,
                                &self.ast_word_target_re,
                                &fn_num_str,
                            );
                        } else {
                            target_para.text = format!("{}[[FN:{}]]", target_para.text, fn_num_str);
                        }
                        target_para.footnotes.push(fn_obj);
                    }
                    continue;
                }
            }

            converted_flat.push((pno, p));
        }

        for (_, p) in &mut converted_flat {
            if p.is_part_heading || p.is_scene_break {
                continue;
            }
            if p.text.contains("[[SUP:*]]") {
                p.text = self
                    .repeated_sfx_sup_re
                    .replace_all(&p.text, "*$1* *$2*")
                    .to_string();
                p.text = p.text.replace("[[SUP:*]]", "");
            }
            if p.text.contains('*') {
                p.text = self.strip_orphan_asterisks(&p.text);
            }
            p.text = self.repair_pre_stitch_delimiters(&p.text);
        }

        let mut stitched = self.stitch_paragraphs(converted_flat);

        let mut seq_footnotes: Vec<Footnote> = Vec::new();
        let mut seq_num: usize = 0;

        for (_, p) in &mut stitched {
            if p.footnotes.is_empty() {
                continue;
            }
            let mut new_p_fns: Vec<Footnote> = Vec::new();
            for fn_item in &p.footnotes {
                if fn_item.text.trim().is_empty() {
                    p.text = p.text.replace(&format!("[[FN:{}]]", fn_item.number), "");
                    continue;
                }
                seq_num += 1;
                let old_token = format!("[[FN:{}]]", fn_item.number);
                let new_num_str = seq_num.to_string();
                let new_token = format!("[[FN:{}]]", new_num_str);
                if p.text.contains(&old_token) {
                    p.text = p.text.replacen(&old_token, &new_token, 1);
                } else {
                    p.text.push_str(&new_token);
                }
                let norm_fn_txt = self
                    .fn_quote_norm_re
                    .replace_all(fn_item.text.trim(), "«$1»")
                    .to_string();
                let new_fn = Footnote {
                    id: format!("fn-{}", new_num_str),
                    number: new_num_str,
                    text: norm_fn_txt,
                };
                new_p_fns.push(new_fn.clone());
                seq_footnotes.push(new_fn);
            }
            p.footnotes = new_p_fns;
            let valid_tokens: HashSet<String> = p
                .footnotes
                .iter()
                .map(|f| format!("[[FN:{}]]", f.number))
                .collect();
            p.text = self
                .fn_token_re
                .replace_all(&p.text, |caps: &Captures| {
                    let m_str = &caps[0];
                    if valid_tokens.contains(m_str) {
                        m_str.to_string()
                    } else {
                        String::new()
                    }
                })
                .to_string();
        }

        for (_, p) in &mut stitched {
            if !p.is_part_heading && !p.is_scene_break {
                p.style = self.classify_paragraph_style(&p.text, None);
            }
        }

        (stitched, seq_footnotes)
    }

    fn strip_orphan_asterisks(&self, text: &str) -> String {
        let mut sound_placeholders: Vec<String> = Vec::new();
        let mut protected = String::with_capacity(text.len());
        let mut last_end = 0;

        for m in self.sfx_core_re.find_iter(text) {
            let before = &text[..m.start()];
            let after = &text[m.end()..];
            let prev_ok = before.chars().last().map_or(true, |c| {
                c.is_whitespace() || matches!(c, '—' | '―' | '-' | '[' | '«' | '“' | '¡' | '¿' | '(')
            });
            let next_ok = after.chars().next().map_or(true, |c| {
                c.is_whitespace()
                    || matches!(c, '.' | ',' | '!' | '?' | ';' | ':' | '…' | ']' | '»' | '”' | ')')
            });
            protected.push_str(&text[last_end..m.start()]);
            if prev_ok && next_ok {
                let idx = sound_placeholders.len();
                sound_placeholders.push(m.as_str().to_string());
                protected.push_str(&format!("__SFX_{}__", idx));
            } else {
                protected.push_str(m.as_str());
            }
            last_end = m.end();
        }
        protected.push_str(&text[last_end..]);

        let cleaned_punct = self
            .orphan_ast_before_punct_re
            .replace_all(&protected, "$1")
            .to_string();

        let mut cleaned = String::with_capacity(cleaned_punct.len());
        let mut last = 0;
        for caps in self.orphan_ast_after_word_re.captures_iter(&cleaned_punct) {
            let m = caps.get(0).unwrap();
            let after = &cleaned_punct[m.end()..];
            let next_is_letter = after.chars().next().is_some_and(char::is_alphabetic);
            cleaned.push_str(&cleaned_punct[last..m.start()]);
            if !next_is_letter {
                cleaned.push_str(&caps[1]);
            } else {
                cleaned.push_str(m.as_str());
            }
            last = m.end();
        }
        cleaned.push_str(&cleaned_punct[last..]);

        for (idx, orig) in sound_placeholders.into_iter().enumerate() {
            cleaned = cleaned.replace(&format!("__SFX_{}__", idx), &orig);
        }
        cleaned
    }

    fn stitch_paragraphs(&self, items: Vec<(usize, Paragraph)>) -> Vec<(usize, Paragraph)> {
        let mut out = items;
        let mut changed = true;

        while changed {
            changed = false;
            let mut new_out: Vec<(usize, Paragraph)> = Vec::with_capacity(out.len());
            let mut i = 0;
            let n = out.len();
            let clean_cache: Vec<String> = out
                .iter()
                .map(|(_, p)| {
                    let mut s = if p.text.contains("[[") {
                        self.fn_sup_strip_re.replace_all(&p.text, "").trim().to_string()
                    } else {
                        p.text.trim().to_string()
                    };
                    if s.contains('\u{E000}') || s.contains('\u{E001}') {
                        s = crate::text_utils::strip_formatting_tokens(&s).trim().to_string();
                    }
                    s
                })
                .collect();

            while i < n {
                let (pno_i, mut p_i) = out[i].clone();
                if p_i.is_part_heading || p_i.is_scene_break || p_i.style == "verse_or_chant" {
                    new_out.push((pno_i, p_i));
                    i += 1;
                    continue;
                }

                let t_i = &clean_cache[i];
                let mut merged_multi = false;

                for &(open_ch, close_ch) in STITCHABLE_DELIMITER_PAIRS {
                    if t_i.contains(open_ch)
                        && t_i.matches(open_ch).count() > t_i.matches(close_ch).count()
                    {
                        let mut close_idx: Option<usize> = None;
                        for k in (i + 1)..n.min(i + 6) {
                            let p_k = &out[k].1;
                            if p_k.is_part_heading || p_k.is_scene_break {
                                break;
                            }
                            let t_k = &clean_cache[k];
                            if t_k.starts_with(open_ch) {
                                break;
                            }
                            if open_ch != '['
                                && k > i + 1
                                && (t_k.starts_with('—')
                                    || t_k.starts_with('―')
                                    || t_k.starts_with('“')
                                    || t_k.starts_with('"')
                                    || t_k.starts_with('「')
                                    || t_k.starts_with('『')
                                    || t_k.starts_with('«'))
                            {
                                break;
                            }
                            if t_k.contains(close_ch)
                                && t_k.matches(close_ch).count() > t_k.matches(open_ch).count()
                            {
                                close_idx = Some(k);
                                break;
                            }
                        }

                        if let Some(c_idx) = close_idx {
                            let mut combined_text = p_i.text.clone();
                            let mut combined_fns = p_i.footnotes.clone();
                            for k in (i + 1)..=c_idx {
                                combined_text = self.join_two_texts(&combined_text, &out[k].1.text);
                                combined_fns.extend(out[k].1.footnotes.iter().cloned());
                            }
                            p_i.text = combined_text;
                            p_i.footnotes = combined_fns;
                            p_i.style = self.classify_paragraph_style(&p_i.text, None);
                            new_out.push((pno_i, p_i.clone()));
                            i = c_idx + 1;
                            merged_multi = true;
                            changed = true;
                            break;
                        }
                    }
                }

                if merged_multi {
                    continue;
                }

                if i + 1 < n {
                    let (pno_next, ref p_next) = out[i + 1];
                    if !p_next.is_part_heading
                        && !p_next.is_scene_break
                        && p_next.style != "verse_or_chant"
                    {
                        let t_next = &clean_cache[i + 1];
                        let is_comma_quote_cont = (t_i.ends_with(",\"")
                            || t_i.ends_with(", ”")
                            || t_i.ends_with(",”"))
                            && self.stitch_lower_cont_re.is_match(t_next);
                        let lacks_terminal = !t_i.is_empty()
                            && (!self.stitch_lacks_terminal_re.is_match(t_i) || is_comma_quote_cont);
                        let next_is_continuation =
                            !t_next.is_empty() && !self.stitch_next_blocked_re.is_match(t_next);
                        let is_cross_page_quote = pno_next > pno_i
                            && t_i.starts_with('“')
                            && t_i.matches('“').count() > t_i.matches('”').count()
                            && !t_next.starts_with('“')
                            && t_next.ends_with('”');

                        if is_cross_page_quote
                            || (lacks_terminal
                                && next_is_continuation
                                && (pno_next > pno_i || self.stitch_lower_cont_re.is_match(t_next)))
                        {
                            p_i.text = self.join_two_texts(&p_i.text, &p_next.text);
                            p_i.footnotes.extend(p_next.footnotes.iter().cloned());
                            p_i.style = self.classify_paragraph_style(&p_i.text, None);
                            new_out.push((pno_i, p_i));
                            i += 2;
                            changed = true;
                            continue;
                        }
                    }
                }

                new_out.push((pno_i, p_i));
                i += 1;
            }

            out = new_out;
        }

        out
    }

    fn join_two_texts(&self, first: &str, second: &str) -> String {
        crate::text_utils::join_hyphenated_texts(first, second)
    }

    pub const fn delimiter_hierarchy(&self) -> &'static [DelimiterHierarchySpec; 5] {
        &DEFAULT_DELIMITER_HIERARCHY
    }

    pub fn canonical_delimiters_for_level(
        &self,
        level: DelimiterLevel,
    ) -> (&'static str, &'static str) {
        match level {
            DelimiterLevel::Level1SpokenDialogue => {
                if self.language_profile.use_em_dash_dialogue {
                    ("—", "")
                } else {
                    ("“", "”")
                }
            }
            DelimiterLevel::Level2MediatedThoughtCite => ("«", "»"),
            DelimiterLevel::Level3TechniqueSkillSystem => ("《", "》"),
            DelimiterLevel::Level4InterfaceStatusLocation => ("[", "]"),
            DelimiterLevel::Level5RhythmPauseSilence => (LEVEL5_TRIPLE_BAR, "…"),
        }
    }

    fn repair_pre_stitch_delimiters(&self, text: &str) -> String {
        let mut s = normalize_cjk_delimiter_variants(text);
        if s.starts_with('[')
            && s.ends_with('」')
            && !s.contains(']')
            && !s.contains('「')
        {
            let mut chars: Vec<char> = s.chars().collect();
            chars.pop();
            chars.push(']');
            s = chars.into_iter().collect();
        }
        if s.contains('《') && s.matches('《').count() > s.matches('》').count() {
            s = self
                .angle_kagi_mismatch_re
                .replace_all(&s, "《$1》」")
                .to_string();
            if s.matches('《').count() > s.matches('》').count() {
                s = self
                    .unclosed_angle_punct_re
                    .replace_all(&s, "《$1》$2")
                    .to_string();
            }
            if s.matches('《').count() > s.matches('》').count() {
                s = close_angle_after_bar(&s, &self.angle_bar_re);
            }
        }
        if s.starts_with('『') {
            s = self
                .kagi_quote_mismatch_re
                .replace_all(&s, "『$1』")
                .to_string();
        }
        if s.ends_with('」') && !s.contains('「') && !s.starts_with('[') {
            s.pop();
        }
        s
    }

    pub fn standardize_section_typography(&self, paragraphs: &mut [Paragraph]) {
        for p in paragraphs.iter_mut() {
            for fn_item in &mut p.footnotes {
                let norm_fn = normalize_cjk_delimiter_variants(fn_item.text.trim());
                fn_item.text = self
                    .fn_quote_norm_re
                    .replace_all(&norm_fn, "«$1»")
                    .to_string();
            }
            if p.is_part_heading || p.is_scene_break || p.text.trim().is_empty() {
                continue;
            }
            p.text = self.standardize_paragraph_text(&p.text, &p.style);
            p.style = self.classify_paragraph_style(&p.text, None);
        }
    }

    pub fn standardize_paragraph_text(&self, text: &str, current_style: &str) -> String {
        let mut s = normalize_cjk_delimiter_variants(text.trim());
        if s.is_empty() {
            return s;
        }

        let mut trail_tokens = String::new();
        if s.contains("[[") {
            if let Some(m) = self.trail_fn_sup_re.find(&s) {
                trail_tokens = m.as_str().trim().to_string();
                s = s[..m.start()].trim().to_string();
            }
        }

        if !self.language_profile.use_em_dash_dialogue {
            s = self.standardize_english_paragraph_text(s, current_style);
        } else {
            s = self.unify_level1_spoken_dialogue(s);
            s = self.unify_level2_mediated_thought_and_citations(s);
            s = self.unify_level3_techniques_and_skills(s);
            s = self.unify_level4_interface_brackets(s);
            s = self.unify_level5_rhythm_and_pauses(s, current_style);
            s = self.balance_residual_delimiters(s);
        }

        if !trail_tokens.is_empty() {
            s.push_str(&trail_tokens);
        }

        s
    }

    fn standardize_english_paragraph_text(&self, mut s: String, current_style: &str) -> String {
        if s.starts_with('「') {
            if let Some(caps) = self.kagi_dialogue_re.captures(&s) {
                s = format!("“{}{}”", caps[1].trim(), &caps[2]);
            } else {
                let inner = s.trim_start_matches('「').trim_end_matches('」').trim();
                s = format!("“{}”", inner);
            }
        } else if self.dialogue_style_re.is_match(&s) {
            let stripped = self.dash_prefix_norm_re.replace(&s, "$1").trim().to_string();
            s = format!("“{}”", stripped);
        }

        if s.contains('"') {
            s = self
                .straight_double_quote_re
                .replace_all(&s, "“$1”")
                .to_string();
            if s.starts_with('"') {
                s = format!("“{}", &s[1..]);
            }
            if s.ends_with('"') {
                s.pop();
                s.push('”');
            }
        }

        if s.contains('『') || s.contains('』') {
            s = s.replace('『', "«").replace('』', "»");
        }
        if s.starts_with('（') {
            if let Some(caps) = self.fullwidth_paren_thought_re.captures(&s) {
                s = format!("«{}»", caps[1].trim());
            }
        }

        if self.preset_profile.enable_lit_rpg_brackets {
            s = self.unify_level3_techniques_and_skills(s);
            s = self.unify_level4_interface_brackets(s);
        }
        s = self.unify_level5_rhythm_and_pauses(s, current_style);

        if s.starts_with('“') && s.matches('“').count() == 1 && s.matches('”').count() == 0 {
            s.push('”');
        }
        if self.preset_profile.enable_lit_rpg_brackets {
            if s.starts_with('[') && s.matches('[').count() == 1 && s.matches(']').count() == 0 {
                s.push(']');
            }
            if s.matches('《').count() > s.matches('》').count() {
                s = self
                    .unclosed_angle_end_re
                    .replace_all(&s, "《$1》")
                    .to_string();
            }
        }
        self.merge_bold_tokens(&s)
    }

    fn unify_level1_spoken_dialogue(&self, mut s: String) -> String {
        let has_any_quote = s.chars().any(|ch| {
            matches!(
                ch,
                '"' | '“' | '”' | '«' | '»' | '『' | '』' | '‘' | '’' | '\'' | '（' | '）'
            )
        });
        if has_any_quote {
            s = replace_with_right_boundary(&s, &self.mismatch_quote_1_re);
            s = replace_with_left_boundary(&s, &self.mismatch_quote_2_re);

            if s.starts_with('(') {
                if let Some(caps) = self.paren_thought_re.captures(&s) {
                    s = format!("«{}»", caps[1].trim());
                }
            } else if s.starts_with('（') {
                if let Some(caps) = self.fullwidth_paren_thought_re.captures(&s) {
                    s = format!("«{}»", caps[1].trim());
                }
            }
        }

        if self.dialogue_style_re.is_match(&s) {
            s = self.dash_prefix_norm_re.replace(&s, "${1}—").to_string();
            if let Some(caps) = self.redundant_dash_quote_re.captures(&s) {
                if !caps[1].contains('“') && !caps[1].contains('"') {
                    s = format!("—{}{}", caps[1].trim(), &caps[2]);
                }
            } else if (s.ends_with('”') || s.ends_with('"'))
                && s.matches('“').count() == 0
                && s.matches('"').count() <= 1
            {
                s.pop();
                s = s.trim().to_string();
            }
        } else if s.starts_with('「') {
            if s.matches('「').count() == s.matches('」').count() {
                if let Some(caps) = self.kagi_dialogue_re.captures(&s) {
                    let inner_k = caps[1].trim();
                    if !inner_k.contains('」') && !inner_k.contains('「') {
                        s = format!("—{}", inner_k);
                    }
                }
            } else if s.matches('「').count() == 1 && s.matches('」').count() == 0 {
                let rest = s.trim_start_matches('「').trim();
                s = format!("—{}", rest);
            }
        } else if s.starts_with('“') || s.starts_with('"') {
            let mut matched_pure = false;
            if let Some(caps) = self.pure_quote_dialogue_re.captures(&s) {
                if !self.mid_quote_space_re.is_match(&caps[1]) {
                    s = format!("—{}", caps[1].trim());
                    matched_pure = true;
                }
            }
            if !matched_pure {
                if let Some(caps) = self.quote_attr_dialogue_re.captures(&s) {
                    let speech_part = caps[1].trim().trim_end_matches(',').trim();
                    let narr_part = caps[2].trim().trim_start_matches('—').trim();
                    s = format!("—{} —{}", speech_part, narr_part);
                } else if (s.matches('“').count() + s.matches('"').count()) == 1
                    && s.matches('”').count() == 0
                {
                    let mut chs = s.chars();
                    chs.next();
                    s = format!("—{}", chs.as_str().trim());
                }
            }
        } else if s.starts_with('‘') && !s.contains('’') && s.chars().count() > 25 {
            let mut chs = s.chars();
            chs.next();
            s = format!("«{}»", chs.as_str().trim());
        }

        s
    }

    fn unify_level2_mediated_thought_and_citations(&self, mut s: String) -> String {
        if s.contains('『') || s.contains('』') || s.contains('「') || s.contains('」') {
            s = s
                .replace('『', "«")
                .replace('』', "»")
                .replace('「', "«")
                .replace('」', "»");
        }

        if s.contains('“') {
            s = self.curly_double_quote_re.replace_all(&s, "«$1»").to_string();
        }
        if s.contains('"') {
            s = self
                .straight_double_quote_re
                .replace_all(&s, "«$1»")
                .to_string();
        }
        if s.contains('‘') {
            s = replace_with_both_boundaries(&s, &self.curly_single_quote_re);
        }
        if s.contains('\'') {
            s = replace_with_both_boundaries(&s, &self.straight_single_quote_re);
        }

        if s.contains('«') || s.contains('»') {
            s = self
                .inner_space_open_guillemet_re
                .replace_all(&s, "«")
                .to_string();
            s = self
                .inner_space_close_guillemet_re
                .replace_all(&s, "»")
                .to_string();
        }

        if s.contains('”') || s.contains('»') || s.contains('"') {
            s = self
                .unopened_title_quote_re
                .replace_all(&s, "$1«$2»")
                .to_string();
        }

        s
    }

    fn unify_level3_techniques_and_skills(&self, mut s: String) -> String {
        if s.contains('《') || s.contains('》') {
            s = self
                .inner_space_open_double_angle_re
                .replace_all(&s, "《")
                .to_string();
            s = self
                .inner_space_close_double_angle_re
                .replace_all(&s, "》")
                .to_string();
            s = self
                .word_before_double_angle_re
                .replace_all(&s, "$1 《")
                .to_string();
            s = self
                .word_after_double_angle_re
                .replace_all(&s, "》 $1")
                .to_string();
        }
        s
    }

    fn unify_level4_interface_brackets(&self, mut s: String) -> String {
        if s.starts_with('[') {
            s = self
                .bracket_inner_quote_re
                .replace_all(&s, "[$1]")
                .to_string();
        }
        s
    }

    fn unify_level5_rhythm_and_pauses(&self, mut s: String, current_style: &str) -> String {
        if !s.starts_with('—')
            && current_style != "dialogue"
            && (s.contains('—') || s.contains('―') || s.contains('–'))
        {
            s = self
                .spaced_pause_dash_re
                .replace_all(&s, "$1───$2")
                .to_string();
        }

        if s.contains('─') || s.contains("——") || s.contains("――") || s.contains("––") {
            s = self.multi_bar_norm_re.replace_all(&s, LEVEL5_TRIPLE_BAR).to_string();
        }
        if s.contains(LEVEL5_TRIPLE_BAR) {
            s = self
                .space_before_triple_bar_re
                .replace_all(&s, LEVEL5_TRIPLE_BAR)
                .to_string();
            s = self
                .space_after_triple_bar_re
                .replace_all(&s, LEVEL5_TRIPLE_BAR)
                .to_string();
            if s.starts_with('—') {
                s = self
                    .dialogue_triple_bar_start_re
                    .replace(&s, "—───")
                    .to_string();
            }
        }
        s
    }

    fn balance_residual_delimiters(&self, mut s: String) -> String {
        if s.matches('«').count() != s.matches('»').count() {
            s = self.multi_open_guil_re.replace_all(&s, "«").to_string();
            s = self.multi_close_guil_re.replace_all(&s, "»").to_string();
        }

        if s.matches('《').count() != s.matches('》').count() {
            s = self.multi_open_angle_re.replace_all(&s, "《").to_string();
            s = self.multi_close_angle_re.replace_all(&s, "》").to_string();
            if s.matches('《').count() > s.matches('》').count() {
                s = close_angle_after_bar(&s, &self.angle_bar_re);
            }
            if s.matches('《').count() > s.matches('》').count() {
                s = self
                    .unclosed_angle_end_re
                    .replace_all(&s, "《$1》")
                    .to_string();
            }
        }

        if s.starts_with('[') && s.matches('[').count() == 1 && s.matches(']').count() == 0 {
            s.push(']');
        } else if s.ends_with(']') && s.matches(']').count() == 1 && s.matches('[').count() == 0 {
            s.insert(0, '[');
        }

        if s.starts_with('«') && s.matches('«').count() == 1 && s.matches('»').count() == 0 {
            s.push('»');
        } else if s.ends_with('»') && s.matches('»').count() == 1 && s.matches('«').count() == 0 {
            s.insert(0, '«');
        }

        if s.contains('“') || s.contains('”') {
            s = s.replace(['“', '”'], "");
        }

        s = self.merge_bold_tokens(&s);
        s
    }
}

static SPLIT_CURLY_QUOTES_RE: OnceLock<Regex> = OnceLock::new();
static SPLIT_OPEN_BRACKET_RE: OnceLock<Regex> = OnceLock::new();

fn split_adjacent_curly_quotes(text: &str) -> Vec<String> {
    let re = SPLIT_CURLY_QUOTES_RE.get_or_init(|| Regex::new(r"”\s+“").unwrap());
    let mut parts = Vec::new();
    let mut last = 0;
    for m in re.find_iter(text) {
        let end_quote = m.start() + '”'.len_utf8();
        let next_quote = m.end() - '“'.len_utf8();
        parts.push(text[last..end_quote].to_string());
        last = next_quote;
    }
    parts.push(text[last..].to_string());
    parts
}

fn split_before_open_bracket(text: &str) -> Vec<String> {
    let re = SPLIT_OPEN_BRACKET_RE.get_or_init(|| Regex::new(r"\s+\[").unwrap());
    let mut parts = Vec::new();
    let mut last = 0;
    for m in re.find_iter(text) {
        let next_bracket = m.end() - 1;
        parts.push(text[last..m.start()].to_string());
        last = next_bracket;
    }
    parts.push(text[last..].to_string());
    parts
}

fn has_word_asterisk(text: &str, re: &Regex) -> bool {
    for caps in re.captures_iter(text) {
        let m = caps.get(0).unwrap();
        let before = &text[..m.start()];
        let after = &text[m.end()..];
        let prev_is_ast = before.ends_with('*');
        let next_is_letter_or_ast = after
            .chars()
            .next()
            .is_some_and(|c| c.is_alphabetic() || c == '*');
        if !prev_is_ast && !next_is_letter_or_ast {
            return true;
        }
    }
    false
}

fn replace_first_word_asterisk(text: &str, re: &Regex, fn_num_str: &str) -> String {
    for caps in re.captures_iter(text) {
        let m = caps.get(0).unwrap();
        let before = &text[..m.start()];
        let after = &text[m.end()..];
        let prev_is_ast = before.ends_with('*');
        let next_is_letter_or_ast = after
            .chars()
            .next()
            .is_some_and(|c| c.is_alphabetic() || c == '*');
        if !prev_is_ast && !next_is_letter_or_ast {
            return format!("{}{}[[FN:{}]]{}", before, &caps[1], fn_num_str, after);
        }
    }
    text.to_string()
}

fn close_angle_after_bar(text: &str, re: &Regex) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for caps in re.captures_iter(text) {
        let m = caps.get(0).unwrap();
        let after = &text[m.end()..];
        out.push_str(&text[last..m.start()]);
        if !after.starts_with('》') {
            out.push_str(&format!("《{}》", &caps[1]));
        } else {
            out.push_str(m.as_str());
        }
        last = m.end();
    }
    out.push_str(&text[last..]);
    out
}

fn replace_with_right_boundary(text: &str, re: &Regex) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for caps in re.captures_iter(text) {
        let m = caps.get(0).unwrap();
        let after = &text[m.end()..];
        let right_ok = after.chars().next().map_or(true, |c| {
            c.is_whitespace() || matches!(c, '.' | ',' | '!' | '?' | ';' | ':' | '…' | ']' | '»' | ')')
        });
        out.push_str(&text[last..m.start()]);
        if right_ok {
            out.push_str(&format!("«{}»", &caps[1]));
        } else {
            out.push_str(m.as_str());
        }
        last = m.end();
    }
    out.push_str(&text[last..]);
    out
}

fn replace_with_left_boundary(text: &str, re: &Regex) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for caps in re.captures_iter(text) {
        let m = caps.get(0).unwrap();
        let before = &text[..m.start()];
        let left_ok = before.chars().last().map_or(true, |c| {
            c.is_whitespace() || matches!(c, '—' | '―' | '-' | '[' | '«' | '(')
        });
        out.push_str(&text[last..m.start()]);
        if left_ok {
            out.push_str(&format!("«{}»", &caps[1]));
        } else {
            out.push_str(m.as_str());
        }
        last = m.end();
    }
    out.push_str(&text[last..]);
    out
}

fn replace_with_both_boundaries(text: &str, re: &Regex) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for caps in re.captures_iter(text) {
        let m = caps.get(0).unwrap();
        let before = &text[..m.start()];
        let after = &text[m.end()..];
        let left_ok = before.chars().last().map_or(true, |c| {
            c.is_whitespace() || matches!(c, '—' | '―' | '-' | '[' | '«' | '(')
        });
        let right_ok = after.chars().next().map_or(true, |c| {
            c.is_whitespace() || matches!(c, '.' | ',' | '!' | '?' | ';' | ':' | '…' | ']' | '»' | ')')
        });
        out.push_str(&text[last..m.start()]);
        if left_ok && right_ok {
            out.push_str(&format!("«{}»", &caps[1]));
        } else {
            out.push_str(m.as_str());
        }
        last = m.end();
    }
    out.push_str(&text[last..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_five_level_delimiter_unification() {
        let cleaner = TextCleaner::new();
        assert_eq!(cleaner.delimiter_hierarchy().len(), 5);
        assert_eq!(
            cleaner.canonical_delimiters_for_level(DelimiterLevel::Level1SpokenDialogue),
            ("—", "")
        );
        assert_eq!(
            cleaner.canonical_delimiters_for_level(DelimiterLevel::Level2MediatedThoughtCite),
            ("«", "»")
        );
        assert_eq!(
            cleaner.canonical_delimiters_for_level(DelimiterLevel::Level3TechniqueSkillSystem),
            ("《", "》")
        );
        assert_eq!(
            cleaner.canonical_delimiters_for_level(DelimiterLevel::Level4InterfaceStatusLocation),
            ("[", "]")
        );
        assert_eq!(
            cleaner.canonical_delimiters_for_level(DelimiterLevel::Level5RhythmPauseSilence),
            ("───", "…")
        );

        let l1 = cleaner.standardize_paragraph_text("「¡Adelante, no te detengas!」", "narrative");
        assert_eq!(l1, "—¡Adelante, no te detengas!");
        assert_eq!(cleaner.classify_paragraph_style(&l1, None), "dialogue");

        let l2_kagi = cleaner.standardize_paragraph_text("『¿Puedes escucharme ahora?』", "narrative");
        assert_eq!(l2_kagi, "«¿Puedes escucharme ahora?»");
        assert_eq!(cleaner.classify_paragraph_style(&l2_kagi, None), "thought");

        let l2_paren = cleaner.standardize_paragraph_text("（Tengo que encontrar una salida）", "narrative");
        assert_eq!(l2_paren, "«Tengo que encontrar una salida»");
        assert_eq!(cleaner.classify_paragraph_style(&l2_paren, None), "thought");

        let l3 = cleaner.standardize_paragraph_text("Invocó≪ Excalibur ≫y〈 Avalón 〉al instante.", "narrative");
        assert_eq!(l3, "Invocó 《Excalibur》 y 《Avalón》 al instante.");

        let l4 = cleaner.standardize_paragraph_text("【¡Comienza el encuentro!】", "narrative");
        assert_eq!(l4, "[¡Comienza el encuentro!]");
        assert_eq!(cleaner.classify_paragraph_style(&l4, None), "bracket_speech");

        let l4_unclosed_q = cleaner.standardize_paragraph_text("［“¡Comienza el encuentro!］", "narrative");
        assert_eq!(l4_unclosed_q, "[¡Comienza el encuentro!]");

        let l5 = cleaner.standardize_paragraph_text("El acero brilló ―― y el silencio cayó.", "narrative");
        assert_eq!(l5, "El acero brilló───y el silencio cayó.");
    }
}
