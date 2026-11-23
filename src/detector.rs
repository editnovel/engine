use crate::config::{
    CHAPTER_PATTERNS, DocumentProfile, HEADER_NOISE_PATTERNS, SCENE_BREAK_PATTERNS,
    SKIP_VISUAL_TOC_PAGES, SUBSECTION_PATTERNS, normalize_cjk_delimiter_variants,
};
use crate::models::{PdfPageData, Section};
use regex::{Captures, Regex};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct PageTextBlock {
    pub y0: f64,
    pub y1: f64,
    pub lines: Vec<String>,
    pub max_size: f64,
    pub is_bold: bool,
}

pub struct ChapterDetector {
    patterns: Vec<Regex>,
    subsection_patterns: Vec<Regex>,
    noise_patterns: Vec<Regex>,
    scene_patterns: Vec<Regex>,
    index_pattern: Regex,
    greeting_pattern: Regex,
    bullet_heading_re: Regex,
    ws_re: Regex,
    palabras_finales_re: Regex,
    periodico_mural_re: Regex,
    trailing_quote_re: Regex,
    trailing_digit_re: Regex,
    cap_re: Regex,
    fin_re: Regex,
    pro_re: Regex,
    epi_re: Regex,
    int_re: Regex,
    interm_re: Regex,
    afterword_re: Regex,
    resumen_re: Regex,
    outer_q_re: Regex,
    punct_low_re: Regex,
    word3_re: Regex,
    mid_cap_re: Regex,
    small_words_re: Regex,
    open_angle_sp_re: Regex,
    close_angle_sp_re: Regex,
    open_excl_sp_re: Regex,
    open_quest_sp_re: Regex,
    bullet_sp_re: Regex,
    close_angle_word_re: Regex,
    word_open_angle_re: Regex,
    vs_re: Regex,
    palabras_del_re: Regex,
    autor_re: Regex,
    cap_final_re: Regex,
    single_cap_re: Regex,
    afterword_or_summary_re: Regex,
    epilogo_only_re: Regex,
    toc_dots_re: Regex,
    main_type_re: Regex,
}

impl Default for ChapterDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl ChapterDetector {
    pub fn new() -> Self {
        let patterns = CHAPTER_PATTERNS
            .iter()
            .filter_map(|p| Regex::new(&format!("(?i){}", p)).ok())
            .collect();
        let subsection_patterns = SUBSECTION_PATTERNS
            .iter()
            .filter_map(|p| Regex::new(&format!("(?i){}", p)).ok())
            .collect();
        let noise_patterns = HEADER_NOISE_PATTERNS
            .iter()
            .filter_map(|p| Regex::new(&format!("(?i){}", p)).ok())
            .collect();
        let scene_patterns = SCENE_BREAK_PATTERNS
            .iter()
            .filter_map(|p| Regex::new(p).ok())
            .collect();

        Self {
            patterns,
            subsection_patterns,
            noise_patterns,
            scene_patterns,
            index_pattern: Regex::new(
                r"(?i)^\s*(?:[IÍ]ndice|Contenido|Sumario|Tabla\s+de\s+contenido)",
            )
            .unwrap(),
            greeting_pattern: Regex::new(
                r"(?i)^(?:Gracias\s+por|¡?Gracias\s+lectores|Hola\b|Saludos\b|Cu[áa]nto\s+tiempo|Aqu[íi]\s+[A-ZÁÉÍÓÚÜÑ][a-záéíóúüñ]+|Soy\s+(?:el\s+autor|[A-ZÁÉÍÓÚÜÑ][a-záéíóúüñ]+(?:\s+[A-ZÁÉÍÓÚÜÑ][a-záéíóúüñ]+)?)|Para\s+aquellos\s+que\s+me\s+conocen|Bueno\b|\*?\s*Nota\s+del)",
            )
            .unwrap(),
            bullet_heading_re: Regex::new(
                r"(?i)^[u•·▪▸►]\s+((?:Pr[oó]logo|Cap[ií]tulo|Ep[ií]logo|Interludio|Intermisi[oó]n|Palabras|Resumen|Notas?)\b)",
            )
            .unwrap(),
            ws_re: Regex::new(r"\s+").unwrap(),
            palabras_finales_re: Regex::new(r"(?i)^Palabras\s+Finales$").unwrap(),
            periodico_mural_re: Regex::new(r"(?i)^Peri[óo]dico\s+Mural\b").unwrap(),
            trailing_quote_re: Regex::new(r#"[”"]\.?$"#).unwrap(),
            trailing_digit_re: Regex::new(r"([A-Za-zÀ-ÿ])\d+\b").unwrap(),
            cap_re: Regex::new(r"(?i)^(?:Cap[íi]tulo|Chapter)\s+(\d+|[IVXLCDM]+)$").unwrap(),
            fin_re: Regex::new(r"(?i)^Cap[íi]tulo\s+Final(?:\s+([A-Z]))?$").unwrap(),
            pro_re: Regex::new(r"(?i)^(?:Pr[óo]logo|Prologue)(?:\s+([IVXLCDM]+|\d+))?$").unwrap(),
            epi_re: Regex::new(r"(?i)^(?:Ep[íi]logo|Epilogue)(?:\s+([IVXLCDM]+|\d+))?$").unwrap(),
            int_re: Regex::new(r"(?i)^Interludio(?:\s+(\d+|[IVXLCDM]+))?$").unwrap(),
            interm_re: Regex::new(r"(?i)^Intermisi[óo]n(?:\s+(\d+|[IVXLCDM]+))?$").unwrap(),
            afterword_re: Regex::new(
                r"(?i)^(?:Palabras\s+(?:del\s+autor|finales|de\s+cierre)|Posfacio|Afterword|Ep[íi]logo\s+del\s+autor)$",
            )
            .unwrap(),
            resumen_re: Regex::new(r"(?i)^Resumen\s+(?:de\s+lo\s+ocurrido\s+)?hasta\s+ahora$")
                .unwrap(),
            outer_q_re: Regex::new(r#"^[“"«]([^”"»]+)[”"»]\.?$"#).unwrap(),
            punct_low_re: Regex::new(r"([“«—–]\s*)([a-záéíóúñ])").unwrap(),
            word3_re: Regex::new(r"\b[A-Za-zÀ-ÿ]{3,}\b").unwrap(),
            mid_cap_re: Regex::new(r"[a-záéíóúñ][A-ZÁÉÍÓÚÑ]").unwrap(),
            small_words_re: Regex::new(
                r"(\s)(De|Del|La|Las|Los|El|Y|E|O|U|Con|En|A|Al|Un|Una|Por|Para|Sin|Sobre|Contra|Entre|Hacia|Hasta)\b",
            )
            .unwrap(),
            open_angle_sp_re: Regex::new(r"《\s+").unwrap(),
            close_angle_sp_re: Regex::new(r"\s+》").unwrap(),
            open_excl_sp_re: Regex::new(r"¡\s+").unwrap(),
            open_quest_sp_re: Regex::new(r"¿\s+").unwrap(),
            bullet_sp_re: Regex::new(r"\s*•\s*").unwrap(),
            close_angle_word_re: Regex::new(r"》([A-Za-zÀ-ÿ])").unwrap(),
            word_open_angle_re: Regex::new(r"([A-Za-zÀ-ÿ])《").unwrap(),
            vs_re: Regex::new(r"(?i)(\s|》|”)\bvs\.?(\s|《|“)").unwrap(),
            palabras_del_re: Regex::new(r"(?i)^Palabras\s+del\s*:?$").unwrap(),
            autor_re: Regex::new(r"(?i)^Autor\s*:?$").unwrap(),
            cap_final_re: Regex::new(r"(?i)^Cap[íi]tulo\s+Final\s*:?$").unwrap(),
            single_cap_re: Regex::new(r"^[A-Z]\s*:?$").unwrap(),
            afterword_or_summary_re: Regex::new(
                r"(?i)^(?:Palabras\s+(?:del\s+autor|finales|de\s+cierre)|Posfacio|Afterword|Ep[íi]logo\s+del\s+autor|Resumen\s+(?:de\s+lo\s+ocurrido\s+)?hasta\s+ahora)$",
            )
            .unwrap(),
            epilogo_only_re: Regex::new(r"(?i)^Ep[íi]logo$").unwrap(),
            toc_dots_re: Regex::new(r"\.{3,}\s*\d+").unwrap(),
            main_type_re: Regex::new(r"(?i)^(?:Ep[íi]logo|Pr[óo]logo|Cap[íi]tulo\b)").unwrap(),
        }
    }

    pub fn estimate_base_font_size(&self, pages: &[PdfPageData]) -> f64 {
        let mut size_counts: HashMap<i32, usize> = HashMap::new();
        let sample_len = pages.len().min(25);
        for page in &pages[..sample_len] {
            for b in &page.blocks {
                if b.block_type != 0 {
                    continue;
                }
                for line in &b.lines {
                    for span in &line.spans {
                        let txt = span.text.trim();
                        if txt.chars().count() > 3 {
                            let key = (span.size * 10.0).round() as i32;
                            *size_counts.entry(key).or_insert(0) += txt.chars().count();
                        }
                    }
                }
            }
        }
        if size_counts.is_empty() {
            return 11.0;
        }
        let best_key = size_counts
            .into_iter()
            .max_by_key(|&(_, count)| count)
            .map(|(k, _)| k)
            .unwrap_or(110);
        (best_key as f64) / 10.0
    }

    pub fn extract_page_blocks(
        &self,
        page: &PdfPageData,
        base_size: f64,
        profile: Option<&DocumentProfile>,
    ) -> Vec<PageTextBlock> {
        let page_height = page.height;
        let mut structured = Vec::new();

        for b in &page.blocks {
            if b.block_type != 0 {
                continue;
            }
            let y0 = b.bbox.1;
            if y0 < 50.0 || y0 > page_height - 42.0 {
                continue;
            }

            let mut raw_block_lines: Vec<(String, f64, f64, f64, bool)> = Vec::new();

            for line in &b.lines {
                let mut line_parts: Vec<String> = Vec::new();
                let mut line_max_sz = 0.0_f64;
                let mut line_bold = false;
                let line_y0 = line.bbox.1;
                let line_y1 = line.bbox.3;

                for s in &line.spans {
                    let raw_s = &s.text;
                    let stripped_s = raw_s.trim();
                    if stripped_s.is_empty() {
                        if let Some(last) = line_parts.last() {
                            if !last.ends_with(' ') {
                                line_parts.push(" ".to_string());
                            }
                        }
                        continue;
                    }
                    let sz = (s.size * 10.0).round() / 10.0;
                    let flags = s.flags;
                    let is_digits = !stripped_s.is_empty()
                        && stripped_s.chars().all(|c| c.is_ascii_digit());
                    let is_super_digit = (flags & 1) != 0 && is_digits;
                    let is_tiny_digit = (sz < base_size - 1.5) && is_digits;
                    if is_super_digit || is_tiny_digit {
                        continue;
                    }
                    if sz > line_max_sz {
                        line_max_sz = sz;
                    }
                    if (flags & 16) != 0 {
                        line_bold = true;
                    }
                    line_parts.push(raw_s.clone());
                }

                let joined = line_parts.join("");
                let clean_ws = self.ws_re.replace_all(&joined, " ").trim().to_string();
                let clean_line = self
                    .bullet_heading_re
                    .replace(&clean_ws, "$1")
                    .trim()
                    .to_string();

                if !clean_line.is_empty() {
                    if self.noise_patterns.iter().any(|np| np.is_match(&clean_line)) {
                        continue;
                    }
                    if let Some(prof) = profile {
                        if prof.is_margin_or_banner_noise(&clean_line) {
                            continue;
                        }
                    }
                    let eff_sz = if line_max_sz > 0.0 {
                        line_max_sz
                    } else {
                        base_size
                    };
                    if let Some(last) = raw_block_lines.last_mut() {
                        if (line_y0 - last.1).abs() <= 3.0 {
                            last.0 = format!("{} {}", last.0, clean_line);
                            last.1 = last.1.min(line_y0);
                            last.2 = last.2.max(line_y1);
                            last.3 = last.3.max(eff_sz);
                            last.4 = last.4 || line_bold;
                            continue;
                        }
                    }
                    raw_block_lines.push((clean_line, line_y0, line_y1, eff_sz, line_bold));
                }
            }

            if !raw_block_lines.is_empty() {
                let mut curr_lines_out = vec![raw_block_lines[0].0.clone()];
                let mut curr_y0 = raw_block_lines[0].1;
                let mut curr_y1 = raw_block_lines[0].2;
                let mut curr_max_sz = raw_block_lines[0].3;
                let mut curr_bold = raw_block_lines[0].4;

                for (ln_txt, ln_y0, ln_y1, ln_sz, ln_bold) in raw_block_lines.into_iter().skip(1) {
                    let prev_is_heading_sz = curr_max_sz >= base_size + 1.5;
                    let curr_is_heading_sz = ln_sz >= base_size + 1.5;
                    if prev_is_heading_sz != curr_is_heading_sz {
                        structured.push(PageTextBlock {
                            y0: curr_y0,
                            y1: curr_y1,
                            lines: curr_lines_out,
                            max_size: curr_max_sz,
                            is_bold: curr_bold,
                        });
                        curr_lines_out = vec![ln_txt];
                        curr_y0 = ln_y0;
                        curr_y1 = ln_y1;
                        curr_max_sz = ln_sz;
                        curr_bold = ln_bold;
                    } else {
                        curr_lines_out.push(ln_txt);
                        curr_y1 = curr_y1.max(ln_y1);
                        curr_max_sz = curr_max_sz.max(ln_sz);
                        curr_bold = curr_bold || ln_bold;
                    }
                }

                structured.push(PageTextBlock {
                    y0: curr_y0,
                    y1: curr_y1,
                    lines: curr_lines_out,
                    max_size: curr_max_sz,
                    is_bold: curr_bold,
                });
            }
        }

        structured
    }

    fn is_valid_subtitle_line(
        &self,
        line: &str,
        allow_quoted: bool,
        profile: Option<&DocumentProfile>,
    ) -> bool {
        let norm_line = normalize_cjk_delimiter_variants(line);
        let cleaned = norm_line.trim();
        let char_len = cleaned.chars().count();
        if cleaned.is_empty() || char_len > 85 {
            return false;
        }
        if self.patterns.iter().any(|p| p.is_match(cleaned))
            && !self.palabras_finales_re.is_match(cleaned)
        {
            return false;
        }
        if self.subsection_patterns.iter().any(|sp| sp.is_match(cleaned)) {
            return false;
        }
        if self.scene_patterns.iter().any(|sc| sc.is_match(cleaned)) {
            return false;
        }
        if self.noise_patterns.iter().any(|np| np.is_match(cleaned)) {
            return false;
        }
        if let Some(prof) = profile {
            if prof.is_margin_or_banner_noise(cleaned) {
                return false;
            }
        }
        if self.greeting_pattern.is_match(cleaned) {
            return false;
        }
        if self.periodico_mural_re.is_match(cleaned) {
            return false;
        }
        if cleaned.starts_with('—')
            || cleaned.starts_with('―')
            || cleaned.starts_with("- ")
            || cleaned.starts_with('[')
            || cleaned.starts_with('『')
        {
            return false;
        }
        if cleaned.starts_with('“') || cleaned.starts_with('"') {
            if !allow_quoted {
                return false;
            }
            if !self.trailing_quote_re.is_match(cleaned) {
                return false;
            }
        }
        true
    }

    fn is_subtitle_block(
        &self,
        block: &PageTextBlock,
        base_size: f64,
        profile: Option<&DocumentProfile>,
    ) -> bool {
        if !(block.max_size >= base_size + 1.5 || block.is_bold) {
            return false;
        }
        if block.lines.len() > 3 {
            return false;
        }
        let allow_q = block.max_size >= base_size + 1.5;
        block
            .lines
            .iter()
            .all(|l| self.is_valid_subtitle_line(l, allow_q, profile))
    }

    pub fn normalize_main_heading(&self, main_title: &str) -> String {
        let mut t = main_title.trim().trim_end_matches(':').trim().to_string();
        t = self.trailing_digit_re.replace_all(&t, "$1").to_string();
        t = self.ws_re.replace_all(&t, " ").to_string();

        if let Some(caps) = self.cap_re.captures(&t) {
            return format!("Capítulo {}", &caps[1]);
        }
        if let Some(caps) = self.fin_re.captures(&t) {
            let suffix = caps
                .get(1)
                .map(|m| format!(" {}", m.as_str().to_uppercase()))
                .unwrap_or_default();
            return format!("Capítulo Final{}", suffix);
        }
        if let Some(caps) = self.pro_re.captures(&t) {
            let suffix = caps
                .get(1)
                .map(|m| format!(" {}", m.as_str().to_uppercase()))
                .unwrap_or_default();
            return format!("Prólogo{}", suffix);
        }
        if let Some(caps) = self.epi_re.captures(&t) {
            let suffix = caps
                .get(1)
                .map(|m| format!(" {}", m.as_str().to_uppercase()))
                .unwrap_or_default();
            return format!("Epílogo{}", suffix);
        }
        if let Some(caps) = self.int_re.captures(&t) {
            let suffix = caps
                .get(1)
                .map(|m| format!(" {}", m.as_str()))
                .unwrap_or_default();
            return format!("Interludio{}", suffix);
        }
        if let Some(caps) = self.interm_re.captures(&t) {
            let suffix = caps
                .get(1)
                .map(|m| format!(" {}", m.as_str()))
                .unwrap_or_default();
            return format!("Intermisión{}", suffix);
        }
        if self.afterword_re.is_match(&t) {
            return "Palabras del Autor".to_string();
        }
        if self.resumen_re.is_match(&t) {
            return "Resumen hasta ahora".to_string();
        }
        t
    }

    pub fn normalize_subtitle_text(
        &self,
        sub: &str,
        profile: Option<&DocumentProfile>,
    ) -> String {
        let mut s = normalize_cjk_delimiter_variants(sub.trim());
        if s.is_empty() {
            return String::new();
        }

        if let Some(caps) = self.outer_q_re.captures(&s) {
            s = caps[1].trim().to_string();
        }

        let upper_count = s.chars().filter(|c| c.is_alphabetic() && c.is_uppercase()).count();
        let lower_count = s.chars().filter(|c| c.is_alphabetic() && c.is_lowercase()).count();
        let is_all_caps = upper_count > lower_count * 2 && upper_count >= 4;

        if is_all_caps {
            let low = s.to_lowercase();
            let mut chars = low.chars();
            if let Some(first_c) = chars.next() {
                s = format!("{}{}", first_c.to_uppercase(), chars.as_str());
            }
            if let Some(prof) = profile {
                s = prof.apply_learned_capitalization(&s, true);
            }
            s = self
                .punct_low_re
                .replace_all(&s, |caps: &Captures| {
                    format!("{}{}", &caps[1], caps[2].to_uppercase())
                })
                .to_string();
        }

        s = self
            .word3_re
            .replace_all(&s, |caps: &Captures| {
                let word = &caps[0];
                if word.chars().filter(|c| c.is_alphabetic()).all(|c| c.is_uppercase()) {
                    return word.to_string();
                }
                if self.mid_cap_re.is_match(word) {
                    let mut chs = word.chars();
                    if let Some(fc) = chs.next() {
                        return format!("{}{}", fc, chs.as_str().to_lowercase());
                    }
                }
                word.to_string()
            })
            .to_string();

        if let Some(prof) = profile {
            s = prof.apply_learned_capitalization(&s, is_all_caps);
        }

        s = self
            .small_words_re
            .replace_all(&s, |caps: &Captures| {
                format!("{}{}", &caps[1], caps[2].to_lowercase())
            })
            .to_string();

        s = self.open_angle_sp_re.replace_all(&s, "《").to_string();
        s = self.close_angle_sp_re.replace_all(&s, "》").to_string();
        s = self.open_excl_sp_re.replace_all(&s, "¡").to_string();
        s = self.open_quest_sp_re.replace_all(&s, "¿").to_string();
        s = self.bullet_sp_re.replace_all(&s, " • ").to_string();
        s = self.close_angle_word_re.replace_all(&s, "》 $1").to_string();
        s = self.word_open_angle_re.replace_all(&s, "$1 《").to_string();
        s = self.vs_re.replace_all(&s, "${1}vs${2}").to_string();

        if s.ends_with('.') && !s.ends_with("...") {
            s.pop();
            s = s.trim().to_string();
        }

        self.ws_re
            .replace_all(&s, " ")
            .trim()
            .trim_end_matches(':')
            .to_string()
    }

    fn sanitize_title(
        &self,
        main_title: &str,
        subtitle_parts: &[String],
        profile: Option<&DocumentProfile>,
    ) -> String {
        let clean_main = self.normalize_main_heading(main_title);
        let joined_sub_raw: Vec<String> = subtitle_parts
            .iter()
            .filter_map(|p| {
                let s = p.trim().trim_start_matches(':').trim();
                if s.is_empty() {
                    None
                } else {
                    Some(s.to_string())
                }
            })
            .collect();
        let mut joined_sub = joined_sub_raw.join(" ");
        joined_sub = self.trailing_digit_re.replace_all(&joined_sub, "$1").to_string();
        joined_sub = self.normalize_subtitle_text(&joined_sub, profile);

        if !joined_sub.is_empty() {
            format!("{}: {}", clean_main, joined_sub)
        } else {
            clean_main
        }
    }

    fn collect_following_subtitle(
        &self,
        blocks: &[PageTextBlock],
        start_block_idx: usize,
        initial_lines: &[String],
        base_size: f64,
        profile: Option<&DocumentProfile>,
    ) -> (Vec<String>, usize) {
        let mut subtitle_parts = Vec::new();
        for line in initial_lines {
            if self.is_valid_subtitle_line(line, true, profile) {
                subtitle_parts.push(line.clone());
            } else {
                return (subtitle_parts, 0);
            }
        }

        let mut consumed_extra = 0;
        let mut probe_idx = start_block_idx;
        while probe_idx < blocks.len() {
            let cand_block = &blocks[probe_idx];
            if !self.is_subtitle_block(cand_block, base_size, profile) {
                break;
            }
            let current_len: usize = subtitle_parts.iter().map(|p| p.chars().count()).sum();
            let cand_text = cand_block.lines.join(" ");
            if current_len + cand_text.chars().count() > 110 {
                break;
            }
            subtitle_parts.extend(cand_block.lines.iter().cloned());
            consumed_extra += 1;
            probe_idx += 1;
        }

        (subtitle_parts, consumed_extra)
    }

    pub fn extract_heading_from_blocks(
        &self,
        blocks: &[PageTextBlock],
        idx: usize,
        base_size: f64,
        profile: Option<&DocumentProfile>,
    ) -> Option<(String, usize)> {
        let b = blocks.get(idx)?;
        if b.lines.is_empty() {
            return None;
        }

        let first_line = b.lines[0].trim();

        if self.palabras_del_re.is_match(first_line) {
            let mut consumed = 1;
            let mut rem_lines: Vec<String> = b.lines[1..].to_vec();
            if !rem_lines.is_empty() && self.autor_re.is_match(&rem_lines[0]) {
                rem_lines.remove(0);
            } else if idx + 1 < blocks.len()
                && !blocks[idx + 1].lines.is_empty()
                && self.autor_re.is_match(&blocks[idx + 1].lines[0])
            {
                consumed = 2;
                rem_lines = blocks[idx + 1].lines[1..].to_vec();
            } else {
                return None;
            }

            let (sub_parts, extra) = self.collect_following_subtitle(
                blocks,
                idx + consumed,
                &rem_lines,
                base_size,
                profile,
            );
            return Some((
                self.sanitize_title("Palabras del Autor", &sub_parts, profile),
                consumed + extra,
            ));
        }

        if self.cap_final_re.is_match(first_line) {
            let mut sub_id = String::new();
            let mut consumed = 1;
            let mut rem_lines: Vec<String> = b.lines[1..].to_vec();
            if !rem_lines.is_empty() && self.single_cap_re.is_match(&rem_lines[0]) {
                sub_id = rem_lines[0].trim_end_matches(':').trim().to_string();
                rem_lines.remove(0);
            } else if idx + 1 < blocks.len()
                && !blocks[idx + 1].lines.is_empty()
                && self.single_cap_re.is_match(&blocks[idx + 1].lines[0])
            {
                sub_id = blocks[idx + 1].lines[0]
                    .trim_end_matches(':')
                    .trim()
                    .to_string();
                consumed = 2;
                rem_lines = blocks[idx + 1].lines[1..].to_vec();
            }

            let main_label = format!("Capítulo Final {}", sub_id).trim().to_string();
            let (sub_parts, extra) = self.collect_following_subtitle(
                blocks,
                idx + consumed,
                &rem_lines,
                base_size,
                profile,
            );
            return Some((
                self.sanitize_title(&main_label, &sub_parts, profile),
                consumed + extra,
            ));
        }

        for pat in &self.patterns {
            if pat.is_match(first_line) {
                let is_heading_style = b.max_size >= base_size + 0.8
                    || b.is_bold
                    || (first_line.chars().count() < 40 && !first_line.ends_with('.'));
                if !is_heading_style {
                    return None;
                }

                let mut main_part = first_line.to_string();
                let mut inline_subs = Vec::new();
                if let Some((left, right)) = first_line.split_once(':') {
                    main_part = left.trim().to_string();
                    if !right.trim().is_empty() {
                        inline_subs.push(right.trim().to_string());
                    }
                }

                let is_afterword_or_summary = self.afterword_or_summary_re.is_match(&main_part);
                if is_afterword_or_summary {
                    return Some((self.sanitize_title(&main_part, &[], profile), 1));
                }

                let mut candidate_initial_lines = inline_subs;
                if b.max_size >= base_size + 1.5 || b.is_bold {
                    candidate_initial_lines.extend(b.lines[1..].iter().cloned());
                }

                let (sub_parts, extra) = self.collect_following_subtitle(
                    blocks,
                    idx + 1,
                    &candidate_initial_lines,
                    base_size,
                    profile,
                );

                if sub_parts.is_empty() && self.epilogo_only_re.is_match(main_part.trim()) {
                    let mut following_lines: Vec<String> = b.lines[1..].to_vec();
                    if following_lines.is_empty() && idx + 1 < blocks.len() {
                        following_lines = blocks[idx + 1].lines.clone();
                    }
                    if let Some(first_fol) = following_lines.first() {
                        if self.greeting_pattern.is_match(first_fol.trim()) {
                            main_part = "Palabras del Autor".to_string();
                        }
                    }
                }

                return Some((
                    self.sanitize_title(&main_part, &sub_parts, profile),
                    1 + extra,
                ));
            }
        }

        None
    }

    pub fn detect_sections(&self, pages: &[PdfPageData]) -> Vec<Section> {
        let profile = DocumentProfile::from_pages(pages, "");
        self.detect_sections_with_profile(pages, &profile)
    }

    pub fn detect_sections_with_profile(
        &self,
        pages: &[PdfPageData],
        profile: &DocumentProfile,
    ) -> Vec<Section> {
        let total_pages = pages.len();
        let base_size = profile.base_font_size;
        let mut detected_points: Vec<(usize, String)> = Vec::new();
        let mut blocks_cache: HashMap<usize, Vec<PageTextBlock>> = HashMap::new();

        for pno in 0..total_pages {
            let page = &pages[pno];
            if page.blocks.is_empty() {
                continue;
            }

            if pno < 10 {
                let raw_text = get_raw_page_text(page);
                if raw_text.is_empty() {
                    continue;
                }
                if self.index_pattern.is_match(&raw_text) {
                    continue;
                }
                if SKIP_VISUAL_TOC_PAGES && self.toc_dots_re.find_iter(&raw_text).count() >= 3 {
                    continue;
                }
            }

            let blocks = self.extract_page_blocks(page, base_size, Some(profile));
            blocks_cache.insert(pno, blocks.clone());
            if blocks.is_empty() {
                continue;
            }

            let mut i = 0;
            while i < blocks.len() {
                if let Some((mut title, consumed)) =
                    self.extract_heading_from_blocks(&blocks, i, base_size, Some(profile))
                {
                    if !title.contains(':')
                        && blocks.len() == 1
                        && self.main_type_re.is_match(&title)
                    {
                        for ahead_offset in [1, 2] {
                            if pno + ahead_offset >= total_pages {
                                break;
                            }
                            let ahead_pno = pno + ahead_offset;
                            let ahead_blocks = self.extract_page_blocks(
                                &pages[ahead_pno],
                                base_size,
                                Some(profile),
                            );
                            if ahead_blocks.is_empty() {
                                continue;
                            }
                            if self
                                .periodico_mural_re
                                .is_match(ahead_blocks[0].lines[0].trim())
                            {
                                continue;
                            }
                            if self.is_subtitle_block(&ahead_blocks[0], base_size, Some(profile)) {
                                let (sub_parts, _) = self.collect_following_subtitle(
                                    &ahead_blocks,
                                    0,
                                    &[],
                                    base_size,
                                    Some(profile),
                                );
                                if !sub_parts.is_empty() {
                                    title = self.sanitize_title(&title, &sub_parts, Some(profile));
                                }
                            }
                            break;
                        }
                    }

                    if let Some(last) = detected_points.last() {
                        if last.0 == pno + 1 {
                            i += consumed;
                            continue;
                        }
                        let prev_prefix =
                            last.1.split(':').next().unwrap_or("").trim().to_lowercase();
                        let curr_prefix =
                            title.split(':').next().unwrap_or("").trim().to_lowercase();
                        if prev_prefix == curr_prefix && (pno + 1 - last.0) <= 2 {
                            i += consumed;
                            continue;
                        }
                    }

                    detected_points.push((pno + 1, title));
                    let _ = consumed;
                    break;
                } else {
                    i += 1;
                }
            }
        }

        if detected_points.is_empty() {
            detected_points.push((1, "Capítulo Completo".to_string()));
        }

        let mut sections = Vec::with_capacity(detected_points.len());
        for i in 0..detected_points.len() {
            let (start_p, ref title) = detected_points[i];
            let mut end_p = if i + 1 < detected_points.len() {
                detected_points[i + 1].0.saturating_sub(1)
            } else {
                total_pages
            };
            if end_p < start_p {
                end_p = start_p;
            }
            sections.push(Section::new(title.clone(), None, start_p, end_p));
        }

        for k in 0..sections.len() {
            let p_idx = sections[k].start_page.saturating_sub(1);
            if p_idx >= total_pages {
                continue;
            }
            let blocks = blocks_cache.get(&p_idx).cloned().unwrap_or_else(|| {
                self.extract_page_blocks(&pages[p_idx], base_size, Some(profile))
            });
            for (idx, b) in blocks.iter().enumerate() {
                if self
                    .extract_heading_from_blocks(&blocks, idx, base_size, Some(profile))
                    .is_some()
                {
                    if idx > 0 {
                        let split_y = ((b.y0 - 1.0) * 10.0).round() / 10.0;
                        sections[k].start_y = split_y;
                        if k > 0 && sections[k - 1].end_page == sections[k].start_page - 1 {
                            sections[k - 1].end_page = sections[k].start_page;
                            sections[k - 1].end_y = Some(split_y);
                        }
                    }
                    break;
                }
            }
        }

        sections
    }
}

pub fn get_raw_page_text(page: &PdfPageData) -> String {
    let mut lines = Vec::new();
    for b in &page.blocks {
        if b.block_type != 0 {
            continue;
        }
        for l in &b.lines {
            let mut s_txt = String::new();
            for s in &l.spans {
                s_txt.push_str(&s.text);
            }
            lines.push(s_txt);
        }
    }
    lines.join("\n").trim().to_string()
}
