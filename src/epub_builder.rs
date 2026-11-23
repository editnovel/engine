use crate::cleaner::TextCleaner;
use crate::config::{
    detect_language_from_pages, infer_series_and_volume, resolve_language, DocumentProfile,
    LanguageProfile, ParseOptions, PresetProfile, LANGUAGE,
};
use crate::detector::ChapterDetector;
use crate::image_classifier::HybridImageClassifier;
use crate::models::{
    ContentItem, ContentItemPayload, ExtractedImage, Paragraph, PdfPageData, Section, VolumeInfo,
};
use crate::pdf_extractor::ParsedPdfDocument;
use crate::spell_corrector::ContextualSpellCorrector;
use crate::templates::{
    classify_section_type, render_colophon, render_custom_section_xhtml, render_full_image,
    render_nav, render_section, render_title_page, EPUB_CSS,
};
use lol_html::{element, html_content::ContentType, rewrite_str, RewriteStrSettings};
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::io::{Cursor, Write};
use std::sync::OnceLock;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

#[derive(Debug, Clone)]
pub struct ManifestItem {
    pub id: String,
    pub href: String,
    pub media_type: String,
    pub properties: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TocNode {
    pub title: String,
    pub href: String,
    pub children: Vec<TocNode>,
}

impl TocNode {
    pub fn leaf(title: impl Into<String>, href: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            href: href.into(),
            children: Vec::new(),
        }
    }

    pub fn branch(
        title: impl Into<String>,
        href: impl Into<String>,
        children: Vec<TocNode>,
    ) -> Self {
        Self {
            title: title.into(),
            href: href.into(),
            children,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Landmark {
    pub epub_type: String,
    pub href: String,
    pub label: String,
}

#[derive(Debug, Clone)]
pub struct RenderedSectionParts {
    pub safe_title: String,
    pub main_heading: String,
    pub sub_heading_block: String,
    pub body_joined: String,
}

struct StaticRegexes {
    synopsis_title_re: Regex,
    synopsis_prefix_re: Regex,
    synopsis_alone_re: Regex,
}

static STATIC_REGEXES: OnceLock<StaticRegexes> = OnceLock::new();

fn get_static_regexes() -> &'static StaticRegexes {
    STATIC_REGEXES.get_or_init(|| StaticRegexes {
        synopsis_title_re: Regex::new(
            r"(?i)^(?:sinopsis|synopsis|argumento|blurb|resumen\s+de\s+la\s+obra)(?:\s*[:：\-–—].*)?$",
        )
        .unwrap(),
        synopsis_prefix_re: Regex::new(
            r"(?i)^(?:sinopsis|synopsis|argumento)\s*[:：\-–—]\s*(.+)$",
        )
        .unwrap(),
        synopsis_alone_re: Regex::new(r"(?i)^(?:sinopsis|synopsis|argumento)$").unwrap(),
    })
}

pub fn image_mime_type(ext: &str) -> &'static str {
    match ext.to_lowercase().as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        _ => "application/octet-stream",
    }
}

pub fn deterministic_uuid(name: &str) -> String {
    let mut h1: u64 = 14695981039346656037;
    let mut h2: u64 = 1099511628211;
    for (i, b) in name.as_bytes().iter().enumerate() {
        if i % 2 == 0 {
            h1 ^= *b as u64;
            h1 = h1.wrapping_mul(1099511628211);
        } else {
            h2 ^= *b as u64;
            h2 = h2.wrapping_mul(14695981039346656037);
        }
    }
    let b1 = h1.to_be_bytes();
    let b2 = h2.to_be_bytes();
    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&b1);
    bytes[8..].copy_from_slice(&b2);
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;

    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0], bytes[1], bytes[2], bytes[3],
        bytes[4], bytes[5],
        bytes[6], bytes[7],
        bytes[8], bytes[9],
        bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15]
    )
}

pub struct EpubBuilder {
    pub cleaner: TextCleaner,
    pub detector: ChapterDetector,
    pub spell_corrector: ContextualSpellCorrector,
    pub image_classifier: HybridImageClassifier,
    pub language: String,
    pub preset: PresetProfile,
    multi_colon_re: Regex,
    colon_space_re: Regex,
    part_one_re: Regex,
    norm_h_re: Regex,
    init_heading_re: Regex,
    non_narrative_sec_re: Regex,
    fn_token_re: Regex,
    sup_token_re: Regex,
    strip_tokens_re: Regex,
    pub custom_publisher: Option<String>,
    pub custom_description: Option<String>,
    pub custom_series_title: Option<String>,
    pub custom_author: Option<String>,
    pub custom_illustrator: Option<String>,
    pub custom_isbn: Option<String>,
    pub custom_published_year: Option<String>,
    pub custom_copyright_notice: Option<String>,
    pub custom_translator: Option<String>,
}

impl Default for EpubBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl EpubBuilder {
    pub fn new() -> Self {
        Self {
            cleaner: TextCleaner::new(),
            detector: ChapterDetector::new(),
            spell_corrector: ContextualSpellCorrector::new(2),
            image_classifier: HybridImageClassifier::new(),
            language: LANGUAGE.to_string(),
            preset: PresetProfile::default(),
            multi_colon_re: Regex::new(r":+:").unwrap(),
            colon_space_re: Regex::new(r"\s*:\s*").unwrap(),
            part_one_re: Regex::new(r"(?i)^(?:Parte|Part)\s+(?:1|I)\.?$").unwrap(),
            norm_h_re: Regex::new(r"[\W_]+").unwrap(),
            init_heading_re: Regex::new(
                r"(?i)^(?:cap[íi]tulo\s+(?:\d+|final(?:\s+[a-z])?)|chapter\s+\d+|pr[óo]logo|prologue|ep[íi]logo(?:\s+del\s+autor)?|epilogue|interludio|interlude|intermisi[óo]n|palabras\s+(?:del\s+autor|finales|de\s+cierre)|posfacio|afterword|autor|resumen\s+(?:de\s+lo\s+ocurrido\s+)?hasta\s+ahora)$",
            )
            .unwrap(),
            non_narrative_sec_re: Regex::new(
                r"(?i)^(?:Palabras\s+del\s+Autor|Afterword|Posfacio|Ep[íi]logo\s+del\s+autor|Resumen\b|Notas?\s+del\b)",
            )
            .unwrap(),
            fn_token_re: Regex::new(r"\[\[FN:(\d+)\]\]").unwrap(),
            sup_token_re: Regex::new(r"\[\[SUP:([^\]]+)\]\]").unwrap(),
            strip_tokens_re: Regex::new(r"\[\[(?:FN|SUP):[^\]]+\]\]|[\u{E000}\u{E001}]").unwrap(),
            custom_publisher: None,
            custom_description: None,
            custom_series_title: None,
            custom_author: None,
            custom_illustrator: None,
            custom_isbn: None,
            custom_published_year: None,
            custom_copyright_notice: None,
            custom_translator: None,
        }
    }

    pub fn with_options(options: &ParseOptions) -> Self {
        let mut builder = Self::new();
        let resolved_lang = resolve_language(&options.language, "", &options.preset_id);
        let preset = PresetProfile::from_id(&options.preset_id);
        let lang_profile = LanguageProfile::for_language(resolved_lang);
        builder.language = resolved_lang.to_string();
        builder.preset = preset.clone();
        builder.cleaner.set_profiles(lang_profile, preset);
        builder
    }

    pub fn configure_for_pages(&mut self, pages: &[PdfPageData], options: &ParseOptions) {
        let req = options.language.trim().to_lowercase();
        let resolved_lang = if req == "en" || req == "english" || req.starts_with("en-") {
            "en"
        } else if req == "es" || req == "spanish" || req.starts_with("es-") {
            "es"
        } else {
            let detected = detect_language_from_pages(pages);
            if detected == "en" || options.preset_id.trim().to_lowercase().starts_with("en-") {
                detected
            } else {
                "es"
            }
        };
        let preset = PresetProfile::from_id(&options.preset_id);
        let lang_profile = LanguageProfile::for_language(resolved_lang);
        self.language = resolved_lang.to_string();
        self.preset = preset.clone();
        self.cleaner.set_profiles(lang_profile, preset);
    }

    pub fn process_pdf_bytes(
        &mut self,
        pdf_bytes: &[u8],
        filename: &str,
        extract_images: bool,
    ) -> Result<VolumeInfo, String> {
        let doc = ParsedPdfDocument::from_bytes(pdf_bytes)?;
        Ok(self.process_parsed_pdf(&doc, filename, extract_images))
    }

    pub fn process_parsed_pdf(
        &mut self,
        doc: &ParsedPdfDocument,
        filename: &str,
        extract_images: bool,
    ) -> VolumeInfo {
        let base_name = std::path::Path::new(filename)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(filename);

        let extracted_imgs = if extract_images {
            let (cover_list, inserts, p_imgs, verdicts) = self
                .image_classifier
                .classify_and_filter_document(doc, base_name);
            let cov = cover_list
                .first()
                .cloned()
                .or_else(|| inserts.first().cloned());
            Some((cov, inserts, p_imgs, verdicts))
        } else {
            None
        };

        let (extracted_publisher, extracted_description) = doc.extract_pdf_metadata();
        self.process_pdf_pages(
            &doc.pages,
            filename,
            extracted_imgs,
            extracted_publisher,
            extracted_description,
        )
    }

    pub fn process_pdf_pages(
        &mut self,
        pages: &[PdfPageData],
        filename: &str,
        extracted_imgs: Option<(
            Option<ExtractedImage>,
            Vec<ExtractedImage>,
            HashMap<usize, Vec<ExtractedImage>>,
            Vec<ExtractedImage>,
        )>,
        extracted_publisher: Option<String>,
        mut extracted_description: Option<String>,
    ) -> VolumeInfo {
        let base_name = std::path::Path::new(filename)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(filename);

        let profile = DocumentProfile::from_pages_with_language(pages, base_name, &self.language);
        let vol_num = profile.volume_number;
        let series_title = self
            .custom_series_title
            .as_deref()
            .unwrap_or(&profile.inferred_series_title);
        let vol_title = format!("{} - Volumen {:02}", series_title, vol_num);

        let (cover, color_inserts, page_images, all_verdicts) =
            extracted_imgs.unwrap_or_else(|| (None, Vec::new(), HashMap::new(), Vec::new()));

        let (total_pages, raw_sections_list) = self
            .cleaner
            .extract_raw_sections_with_profile(pages, &self.detector, &profile);
        let mut sections: Vec<Section> = Vec::with_capacity(raw_sections_list.len());
        let mut all_spell_transforms = Vec::new();

        for (mut sec, stitched_pairs, sec_footnotes) in raw_sections_list {
            sec.title = self.multi_colon_re.replace_all(&sec.title, ":").to_string();
            sec.title = self
                .colon_space_re
                .replace_all(&sec.title, ": ")
                .trim()
                .to_string();

            sec.footnotes = sec_footnotes;

            let pnos: Vec<usize> = stitched_pairs.iter().map(|(pno, _)| *pno).collect();
            let mut sec_paras: Vec<Paragraph> =
                stitched_pairs.into_iter().map(|(_, p)| p).collect();

            let start_idx = all_spell_transforms.len() + 1;
            let apply_es_auto_fixes = self.language == "es";
            let transforms = self.spell_corrector.analyze_and_correct_paragraphs(
                &mut sec_paras,
                vol_num,
                sec.start_page,
                &sec.title,
                apply_es_auto_fixes,
                start_idx,
            );
            all_spell_transforms.extend(transforms);

            self.cleaner.standardize_section_typography(&mut sec_paras);

            let mut by_page: HashMap<usize, Vec<Paragraph>> = HashMap::new();
            for (&pno_item, para_item) in pnos.iter().zip(sec_paras.iter()) {
                by_page.entry(pno_item).or_default().push(para_item.clone());
            }
            sec.paragraphs = sec_paras;

            for pno in sec.start_page..=sec.end_page {
                if let Some(page_paras) = by_page.get(&pno) {
                    for p in page_paras {
                        if p.is_scene_break {
                            sec.content_items.push(ContentItem::scene_break_with_text(&p.text, pno));
                        } else if p.is_part_heading {
                            if self.part_one_re.is_match(p.text.trim()) {
                                continue;
                            }
                            sec.content_items.push(ContentItem::scene_break_with_text(&p.text, pno));
                        } else {
                            let valid_fns = p
                                .footnotes
                                .iter()
                                .filter(|f| !f.text.trim().is_empty())
                                .cloned()
                                .collect();
                            sec.content_items.push(ContentItem::text(
                                p.text.clone(),
                                p.style.clone(),
                                valid_fns,
                                pno,
                            ));
                        }
                    }
                }

                if (pno < sec.end_page || sec.end_y.is_none())
                    && page_images.contains_key(&pno)
                {
                    if let Some(imgs) = page_images.get(&pno) {
                        for img in imgs {
                            let is_cov = cover
                                .as_ref()
                                .is_some_and(|c| c.filename == img.filename && c.page_num == img.page_num);
                            let is_col = color_inserts
                                .iter()
                                .any(|c| c.filename == img.filename && c.page_num == img.page_num);
                            if !is_cov && !is_col {
                                let item_t = if self.preset.full_page_illustrations {
                                    "full_illustration"
                                } else {
                                    "image"
                                };
                                sec.content_items
                                    .push(ContentItem::image(item_t, img.clone(), pno));
                            }
                        }
                    }
                }
            }

            self.filter_initial_heading_duplicates(&mut sec);
            self.cleanup_section_scene_breaks(&mut sec);
            sections.push(sec);
        }

        if extracted_description.is_none() {
            extracted_description = detect_synopsis_from_sections(&sections);
        }

        VolumeInfo {
            pdf_path: filename.to_string(),
            volume_number: vol_num,
            title: vol_title,
            total_pages,
            cover_image: cover,
            color_images: color_inserts,
            sections,
            all_verdicts,
            spell_transformations: all_spell_transforms,
            publisher: extracted_publisher,
            description: extracted_description,
        }
    }

    fn norm_h(&self, val: &str) -> String {
        self.norm_h_re
            .replace_all(&val.to_lowercase(), " ")
            .trim()
            .to_string()
    }

    pub fn filter_initial_heading_duplicates(&self, sec: &mut Section) {
        let title_lower = sec.title.to_lowercase();
        let parts: Vec<String> = title_lower
            .split(':')
            .map(|p| self.norm_h(p))
            .filter(|p| !p.is_empty())
            .collect();

        while !sec.content_items.is_empty() {
            let first = &sec.content_items[0];
            if first.item_type != "text" {
                break;
            }
            let ContentItemPayload::Paragraph(ref p) = first.payload else {
                break;
            };
            let raw_txt = p
                .text
                .trim()
                .to_lowercase()
                .trim_end_matches(':')
                .trim()
                .to_string();
            let txt = self.norm_h(&raw_txt);
            let matches_part = parts.iter().any(|part| {
                part == &txt
                    || txt.starts_with(part.as_str())
                    || part.starts_with(txt.as_str())
                    || part.ends_with(txt.as_str())
            });
            if txt.is_empty()
                || txt.chars().count() < 3
                || matches_part
                || self.init_heading_re.is_match(&raw_txt)
            {
                sec.content_items.remove(0);
            } else {
                break;
            }
        }
    }

    pub fn cleanup_section_scene_breaks(&self, sec: &mut Section) {
        let items = std::mem::take(&mut sec.content_items);
        let mut cleaned: Vec<ContentItem> = Vec::with_capacity(items.len());
        let mut has_text_since_boundary = false;

        for (idx, item) in items.iter().enumerate() {
            if item.item_type == "part_heading" {
                while cleaned.last().is_some_and(|c| c.item_type == "scene_break") {
                    cleaned.pop();
                }
                cleaned.push(item.clone());
                has_text_since_boundary = false;
            } else if item.item_type == "scene_break" {
                if !has_text_since_boundary {
                    continue;
                }
                let mut has_text_ahead = false;
                for ahead in &items[idx + 1..] {
                    if ahead.item_type == "part_heading" {
                        break;
                    }
                    if ahead.item_type == "text" {
                        if let ContentItemPayload::Paragraph(ref p) = ahead.payload {
                            if !p.text.trim().is_empty() {
                                has_text_ahead = true;
                                break;
                            }
                        }
                    }
                }
                if has_text_ahead {
                    cleaned.push(item.clone());
                    has_text_since_boundary = false;
                }
            } else if item.item_type == "text" {
                let non_empty = match item.payload {
                    ContentItemPayload::Paragraph(ref p) => !p.text.trim().is_empty(),
                    _ => false,
                };
                cleaned.push(item.clone());
                if non_empty {
                    has_text_since_boundary = true;
                }
            } else {
                cleaned.push(item.clone());
            }
        }

        while cleaned.last().is_some_and(|c| c.item_type == "scene_break") {
            cleaned.pop();
        }

        sec.content_items = cleaned;
    }

    pub fn build_epub_bytes(&self, volume: &VolumeInfo) -> Result<Vec<u8>, String> {
        let mut manifest_items: Vec<ManifestItem> = Vec::new();
        let mut spine_items: Vec<String> = Vec::new();
        let mut toc_tree: Vec<TocNode> = Vec::new();
        let mut image_files: Vec<(&str, &[u8])> = Vec::new();
        let mut html_files: Vec<(String, String)> = Vec::new();
        let mut seen_images: HashSet<String> = HashSet::new();

        manifest_items.push(ManifestItem {
            id: "css".to_string(),
            href: "Styles/stylesheet.css".to_string(),
            media_type: "text/css".to_string(),
            properties: None,
        });

        if let Some(ref cov) = volume.cover_image {
            let mime = image_mime_type(&cov.ext);
            let manifest_id = format!("img_{}", cov.filename.replace('.', "_"));
            manifest_items.push(ManifestItem {
                id: manifest_id,
                href: format!("Images/{}", cov.filename),
                media_type: mime.to_string(),
                properties: Some("cover-image".to_string()),
            });
            image_files.push((&cov.filename, &cov.data));
            seen_images.insert(cov.filename.clone());
        }

        for col_img in &volume.color_images {
            if seen_images.insert(col_img.filename.clone()) {
                let mime = image_mime_type(&col_img.ext);
                manifest_items.push(ManifestItem {
                    id: format!("img_{}", col_img.filename.replace('.', "_")),
                    href: format!("Images/{}", col_img.filename),
                    media_type: mime.to_string(),
                    properties: None,
                });
                image_files.push((&col_img.filename, &col_img.data));
            }
        }

        for sec in &volume.sections {
            for item in &sec.content_items {
                if item.item_type == "image" || item.item_type == "full_illustration" {
                    if let ContentItemPayload::Image(ref img) = item.payload {
                        if seen_images.insert(img.filename.clone()) {
                            let mime = image_mime_type(&img.ext);
                            manifest_items.push(ManifestItem {
                                id: format!("img_{}", img.filename.replace('.', "_")),
                                href: format!("Images/{}", img.filename),
                                media_type: mime.to_string(),
                                properties: None,
                            });
                            image_files.push((&img.filename, &img.data));
                        }
                    }
                }
            }
        }

        if let Some(ref cov) = volume.cover_image {
            let cov_html = self.render_full_image_xhtml_custom(
                "Portada",
                &cov.filename,
                cov.width,
                cov.height,
            );
            html_files.push(("OEBPS/Text/cover.xhtml".to_string(), cov_html));
            manifest_items.push(ManifestItem {
                id: "cover_page".to_string(),
                href: "Text/cover.xhtml".to_string(),
                media_type: "application/xhtml+xml".to_string(),
                properties: Some("svg".to_string()),
            });
            spine_items.push("cover_page".to_string());
            toc_tree.push(TocNode::leaf("Portada", "Text/cover.xhtml"));
        }

        let title_html = render_title_page(
            &self.language,
            &volume.volume_number.to_string(),
            &html_escape(&volume.title),
        );
        html_files.push(("OEBPS/Text/titlepage.xhtml".to_string(), title_html));
        manifest_items.push(ManifestItem {
            id: "title_page".to_string(),
            href: "Text/titlepage.xhtml".to_string(),
            media_type: "application/xhtml+xml".to_string(),
            properties: None,
        });
        spine_items.push("title_page".to_string());
        toc_tree.push(TocNode::leaf("Página de título", "Text/titlepage.xhtml"));

        let non_cover_colors: Vec<&ExtractedImage> = volume
            .color_images
            .iter()
            .filter(|col_img| {
                volume
                    .cover_image
                    .as_ref()
                    .map_or(true, |c| c.filename != col_img.filename)
            })
            .collect();

        for (idx, col_img) in non_cover_colors.iter().enumerate() {
            let item_id = format!("color_{:02}", idx + 1);
            let col_html = self.render_full_image_xhtml_custom(
                &format!("Ilustración a Color {}", idx + 1),
                &col_img.filename,
                col_img.width,
                col_img.height,
            );
            html_files.push((format!("OEBPS/Text/{}.xhtml", item_id), col_html));
            manifest_items.push(ManifestItem {
                id: item_id.clone(),
                href: format!("Text/{}.xhtml", item_id),
                media_type: "application/xhtml+xml".to_string(),
                properties: Some("svg".to_string()),
            });
            spine_items.push(item_id);
        }

        if !non_cover_colors.is_empty() {
            let color_children: Vec<TocNode> = non_cover_colors
                .iter()
                .enumerate()
                .map(|(idx, _)| {
                    TocNode::leaf(
                        format!("Ilustración {}", idx + 1),
                        format!("Text/color_{:02}.xhtml", idx + 1),
                    )
                })
                .collect();
            let first_col_href = "Text/color_01.xhtml".to_string();
            let children = if color_children.len() > 1 {
                color_children
            } else {
                Vec::new()
            };
            toc_tree.push(TocNode::branch("Ilustraciones a Color", first_col_href, children));
        }

        for (s_idx, sec) in volume.sections.iter().enumerate() {
            let sec_id = format!("sec_{:02}", s_idx + 1);
            let sec_html = self.render_section_xhtml_custom(sec, "");
            html_files.push((format!("OEBPS/Text/{}.xhtml", sec_id), sec_html));
            manifest_items.push(ManifestItem {
                id: sec_id.clone(),
                href: format!("Text/{}.xhtml", sec_id),
                media_type: "application/xhtml+xml".to_string(),
                properties: None,
            });
            spine_items.push(sec_id.clone());
            let sec_href = format!("Text/{}.xhtml", sec_id);
            let children_toc: Vec<TocNode> = sec
                .subsections
                .iter()
                .map(|(sub_t, sub_a)| TocNode::leaf(sub_t.clone(), format!("{}#{}", sec_href, sub_a)))
                .collect();
            toc_tree.push(TocNode::branch(sec.title.clone(), sec_href, children_toc));
        }

        let author = self.custom_author.as_deref().unwrap_or("");
        let illustrator = self.custom_illustrator.as_deref().unwrap_or("");
        let translator = self.custom_translator.as_deref().unwrap_or("");
        let publisher_str = self
            .custom_publisher
            .as_deref()
            .or(volume.publisher.as_deref())
            .unwrap_or("");
        let pub_year = self.custom_published_year.as_deref().unwrap_or("");
        let isbn = self.custom_isbn.as_deref().unwrap_or("");
        let cr_notice = self.custom_copyright_notice.as_deref().unwrap_or("");

        let has_colophon = !author.is_empty()
            || !publisher_str.is_empty()
            || !cr_notice.is_empty()
            || !isbn.is_empty()
            || !translator.is_empty();

        if has_colophon {
            let col_id = "colophon".to_string();
            let ill = if illustrator.is_empty() {
                "N/A".to_string()
            } else {
                html_escape(illustrator)
            };
            let col_html = render_colophon(
                &self.language,
                &html_escape(&volume.title),
                &html_escape(author),
                &ill,
                &html_escape(translator),
                &html_escape(publisher_str),
                &html_escape(pub_year),
                &html_escape(isbn),
                &html_escape(cr_notice),
            );
            html_files.push(("OEBPS/Text/colophon.xhtml".to_string(), col_html));
            manifest_items.push(ManifestItem {
                id: col_id.clone(),
                href: "Text/colophon.xhtml".to_string(),
                media_type: "application/xhtml+xml".to_string(),
                properties: None,
            });
            spine_items.push(col_id);
            toc_tree.push(TocNode::leaf("Página de derechos", "Text/colophon.xhtml"));
        }

        let mut landmarks: Vec<Landmark> = Vec::new();
        if volume.cover_image.is_some() {
            landmarks.push(Landmark {
                epub_type: "cover".to_string(),
                href: "Text/cover.xhtml".to_string(),
                label: "Portada".to_string(),
            });
        }
        landmarks.push(Landmark {
            epub_type: "titlepage".to_string(),
            href: "Text/titlepage.xhtml".to_string(),
            label: "Página de título".to_string(),
        });
        let first_body_href = if !non_cover_colors.is_empty() {
            "Text/color_01.xhtml".to_string()
        } else if !volume.sections.is_empty() {
            "Text/sec_01.xhtml".to_string()
        } else {
            String::new()
        };
        if !first_body_href.is_empty() {
            landmarks.push(Landmark {
                epub_type: "bodymatter".to_string(),
                href: first_body_href,
                label: "Comienzo de la lectura".to_string(),
            });
        }
        landmarks.push(Landmark {
            epub_type: "toc".to_string(),
            href: "nav.xhtml#toc".to_string(),
            label: "Tabla de Contenidos".to_string(),
        });
        if has_colophon {
            landmarks.push(Landmark {
                epub_type: "copyright-page".to_string(),
                href: "Text/colophon.xhtml".to_string(),
                label: "Página de derechos".to_string(),
            });
        }

        let publisher = self
            .custom_publisher
            .as_deref()
            .or(volume.publisher.as_deref());
        let description = self
            .custom_description
            .as_deref()
            .or(volume.description.as_deref());

        self.write_complete_epub(
            &volume.title,
            if author.is_empty() { None } else { Some(author) },
            publisher,
            description,
            manifest_items,
            spine_items,
            &toc_tree,
            &landmarks,
            &image_files,
            &html_files,
        )
    }

    pub fn build_omnibus_bytes(&self, volumes: &[VolumeInfo]) -> Result<Vec<u8>, String> {
        let series_title = self.custom_series_title.clone().unwrap_or_else(|| {
            volumes
                .first()
                .map(|v| infer_series_and_volume(&v.pdf_path).0)
                .unwrap_or_else(|| "Colección".to_string())
        });
        let global_title = format!("{} (Colección Completa)", series_title);
        let mut sorted_vols = volumes.to_vec();
        sorted_vols.sort_by_key(|v| v.volume_number);

        let mut manifest_items: Vec<ManifestItem> = Vec::new();
        let mut spine_items: Vec<String> = Vec::new();
        let mut image_files: Vec<(&str, &[u8])> = Vec::new();
        let mut html_files: Vec<(String, String)> = Vec::new();
        let mut named_images: Vec<(String, &[u8])> = Vec::new();

        manifest_items.push(ManifestItem {
            id: "css".to_string(),
            href: "Styles/stylesheet.css".to_string(),
            media_type: "text/css".to_string(),
            properties: None,
        });

        let global_cover = sorted_vols.iter().find_map(|v| v.cover_image.clone());
        let mut global_cover_name = String::new();
        if let Some(ref cov) = global_cover {
            let cov_manifest_id = "cov_global_img".to_string();
            let first_vol_num = sorted_vols.first().map(|v| v.volume_number).unwrap_or(1);
            global_cover_name = format!("v{:02}_{}", first_vol_num, cov.filename);
            let mime = image_mime_type(&cov.ext);
            manifest_items.push(ManifestItem {
                id: cov_manifest_id,
                href: format!("Images/{}", global_cover_name),
                media_type: mime.to_string(),
                properties: Some("cover-image".to_string()),
            });

            let cov_html = self.render_full_image_xhtml_custom(
                "Portada Principal",
                &global_cover_name,
                cov.width,
                cov.height,
            );
            html_files.push(("OEBPS/Text/global_cover.xhtml".to_string(), cov_html));
            manifest_items.push(ManifestItem {
                id: "global_cover_page".to_string(),
                href: "Text/global_cover.xhtml".to_string(),
                media_type: "application/xhtml+xml".to_string(),
                properties: Some("svg".to_string()),
            });
            spine_items.push("global_cover_page".to_string());
        }

        let mut hierarchical_toc: Vec<TocNode> = Vec::new();
        let mut registered_images: HashSet<String> = HashSet::new();

        for v in &sorted_vols {
            let v_num_str = format!("{:02}", v.volume_number);
            let v_prefix = format!("v{}", v_num_str);

            let title_page_id = format!("{}_title_page", v_prefix);
            let title_page_href = format!("Text/{}_title.xhtml", v_prefix);
            let title_html = render_title_page(&self.language, &v_num_str, &series_title);
            html_files.push((format!("OEBPS/{}", title_page_href), title_html));
            manifest_items.push(ManifestItem {
                id: title_page_id.clone(),
                href: title_page_href.clone(),
                media_type: "application/xhtml+xml".to_string(),
                properties: None,
            });
            spine_items.push(title_page_id);

            let mut vol_children_toc: Vec<TocNode> = Vec::new();

            for (c_idx, col_img) in v.color_images.iter().enumerate() {
                let img_name = format!("{}_{}", v_prefix, col_img.filename);
                let mime = image_mime_type(&col_img.ext);
                let img_manifest_id = format!("img_{}", img_name.replace('.', "_"));
                manifest_items.push(ManifestItem {
                    id: img_manifest_id,
                    href: format!("Images/{}", img_name),
                    media_type: mime.to_string(),
                    properties: None,
                });
                registered_images.insert(img_name.clone());
                named_images.push((img_name.clone(), &col_img.data));

                let col_page_id = format!("{}_color_{:02}", v_prefix, c_idx + 1);
                let col_page_href = format!("Text/{}.xhtml", col_page_id);
                let col_html = self.render_full_image_xhtml_custom(
                    &format!("Volumen {} - Ilustración {}", v_num_str, c_idx + 1),
                    &img_name,
                    col_img.width,
                    col_img.height,
                );
                html_files.push((format!("OEBPS/{}", col_page_href), col_html));
                manifest_items.push(ManifestItem {
                    id: col_page_id.clone(),
                    href: col_page_href,
                    media_type: "application/xhtml+xml".to_string(),
                    properties: Some("svg".to_string()),
                });
                spine_items.push(col_page_id);
            }

            if !v.color_images.is_empty() {
                vol_children_toc.push(TocNode::leaf(
                    "Ilustraciones a Color",
                    format!("Text/{}_color_01.xhtml", v_prefix),
                ));
            }

            for (s_idx, sec) in v.sections.iter().enumerate() {
                let sec_page_id = format!("{}_sec_{:02}", v_prefix, s_idx + 1);
                let sec_page_href = format!("Text/{}.xhtml", sec_page_id);

                for itm in &sec.content_items {
                    if itm.item_type == "image" || itm.item_type == "full_illustration" {
                        if let ContentItemPayload::Image(ref in_img) = itm.payload {
                            let in_img_name = format!("{}_{}", v_prefix, in_img.filename);
                            let in_mime = image_mime_type(&in_img.ext);
                            let in_manifest_id = format!("img_{}", in_img_name.replace('.', "_"));
                            let href_str = format!("Images/{}", in_img_name);
                            if registered_images.insert(in_img_name.clone()) {
                                manifest_items.push(ManifestItem {
                                    id: in_manifest_id,
                                    href: href_str,
                                    media_type: in_mime.to_string(),
                                    properties: None,
                                });
                                named_images.push((in_img_name, &in_img.data));
                            }
                        }
                    }
                }

                let sec_html = self.render_section_xhtml_custom(sec, &v_prefix);
                html_files.push((format!("OEBPS/{}", sec_page_href), sec_html));
                manifest_items.push(ManifestItem {
                    id: sec_page_id.clone(),
                    href: sec_page_href.clone(),
                    media_type: "application/xhtml+xml".to_string(),
                    properties: None,
                });
                spine_items.push(sec_page_id);

                let sec_children: Vec<TocNode> = sec
                    .subsections
                    .iter()
                    .map(|(sub_t, sub_a)| {
                        TocNode::leaf(sub_t.clone(), format!("{}#{}", sec_page_href, sub_a))
                    })
                    .collect();

                vol_children_toc.push(TocNode::branch(sec.title.clone(), sec_page_href, sec_children));
            }

            hierarchical_toc.push(TocNode::branch(
                format!("Parte {}", v_num_str),
                title_page_href,
                vol_children_toc,
            ));
        }

        if let Some(ref cov) = global_cover {
            image_files.push((&global_cover_name, &cov.data));
        }
        for (name, data) in &named_images {
            image_files.push((name.as_str(), data));
        }

        let author = self.custom_author.as_deref().unwrap_or("");
        let illustrator = self.custom_illustrator.as_deref().unwrap_or("");
        let translator = self.custom_translator.as_deref().unwrap_or("");
        let publisher = self.custom_publisher.as_deref().unwrap_or("");
        let pub_year = self.custom_published_year.as_deref().unwrap_or("");
        let isbn = self.custom_isbn.as_deref().unwrap_or("");
        let cr_notice = self.custom_copyright_notice.as_deref().unwrap_or("");

        let has_colophon = !author.is_empty()
            || !publisher.is_empty()
            || !cr_notice.is_empty()
            || !isbn.is_empty()
            || !translator.is_empty();

        if has_colophon {
            let global_col_id = "global_colophon".to_string();
            let ill = if illustrator.is_empty() {
                "N/A".to_string()
            } else {
                html_escape(illustrator)
            };
            let global_col_html = render_colophon(
                &self.language,
                &html_escape(&global_title),
                &html_escape(author),
                &ill,
                &html_escape(translator),
                &html_escape(publisher),
                &html_escape(pub_year),
                &html_escape(isbn),
                &html_escape(cr_notice),
            );
            html_files.push(("OEBPS/Text/global_colophon.xhtml".to_string(), global_col_html));
            manifest_items.push(ManifestItem {
                id: global_col_id.clone(),
                href: "Text/global_colophon.xhtml".to_string(),
                media_type: "application/xhtml+xml".to_string(),
                properties: None,
            });
            spine_items.push(global_col_id);
            hierarchical_toc.push(TocNode::leaf("Página de derechos", "Text/global_colophon.xhtml"));
        }

        let mut landmarks: Vec<Landmark> = Vec::new();
        if global_cover.is_some() {
            landmarks.push(Landmark {
                epub_type: "cover".to_string(),
                href: "Text/global_cover.xhtml".to_string(),
                label: "Portada".to_string(),
            });
        }
        landmarks.push(Landmark {
            epub_type: "titlepage".to_string(),
            href: "Text/titlepage.xhtml".to_string(),
            label: "Página de título".to_string(),
        });
        let first_vol_prefix = format!("v{:02}", sorted_vols.first().map(|v| v.volume_number).unwrap_or(1));
        let first_body = if sorted_vols.first().map_or(false, |v| !v.color_images.is_empty()) {
            format!("Text/{}_color_01.xhtml", first_vol_prefix)
        } else {
            format!("Text/{}_title.xhtml", first_vol_prefix)
        };
        landmarks.push(Landmark {
            epub_type: "bodymatter".to_string(),
            href: first_body,
            label: "Comienzo de la lectura".to_string(),
        });
        landmarks.push(Landmark {
            epub_type: "toc".to_string(),
            href: "nav.xhtml#toc".to_string(),
            label: "Tabla de Contenidos".to_string(),
        });
        if has_colophon {
            landmarks.push(Landmark {
                epub_type: "copyright-page".to_string(),
                href: "Text/global_colophon.xhtml".to_string(),
                label: "Página de derechos".to_string(),
            });
        }

        self.write_complete_epub(
            &global_title,
            if author.is_empty() { None } else { Some(author) },
            self.custom_publisher.as_deref(),
            self.custom_description.as_deref(),
            manifest_items,
            spine_items,
            &hierarchical_toc,
            &landmarks,
            &image_files,
            &html_files,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn write_complete_epub(
        &self,
        title: &str,
        author: Option<&str>,
        publisher: Option<&str>,
        description: Option<&str>,
        mut manifest_items: Vec<ManifestItem>,
        spine_items: Vec<String>,
        toc_tree: &[TocNode],
        landmarks: &[Landmark],
        image_files: &[(&str, &[u8])],
        html_files: &[(String, String)],
    ) -> Result<Vec<u8>, String> {
        let mut final_html = html_files.to_vec();

        let nav_html = self.render_nav_xhtml_nodes(title, toc_tree, landmarks);
        final_html.push(("OEBPS/nav.xhtml".to_string(), nav_html));
        manifest_items.push(ManifestItem {
            id: "nav".to_string(),
            href: "nav.xhtml".to_string(),
            media_type: "application/xhtml+xml".to_string(),
            properties: Some("nav".to_string()),
        });

        let ncx_xml = self.render_ncx_nodes(title, toc_tree);
        final_html.push(("OEBPS/toc.ncx".to_string(), ncx_xml));
        manifest_items.push(ManifestItem {
            id: "ncx".to_string(),
            href: "toc.ncx".to_string(),
            media_type: "application/x-dtbncx+xml".to_string(),
            properties: None,
        });

        let opf_xml = self.render_opf_metadata(
            title,
            author,
            publisher,
            description,
            &manifest_items,
            &spine_items,
        );
        let container_xml = self.get_container_xml();

        self.write_epub_zip(&container_xml, &opf_xml, image_files, &final_html)
    }

    fn write_epub_zip(
        &self,
        container_xml: &str,
        opf_xml: &str,
        image_files: &[(&str, &[u8])],
        html_files: &[(String, String)],
    ) -> Result<Vec<u8>, String> {
        let est_images: usize = image_files.iter().map(|(_, b)| b.len()).sum();
        let est_html: usize = html_files.iter().map(|(_, c)| c.len()).sum();
        let total_est = est_images + est_html + container_xml.len() + opf_xml.len() + EPUB_CSS.len() + 32768;
        let mut buf = Cursor::new(Vec::with_capacity(total_est));
        {
            let mut zw = ZipWriter::new(&mut buf);
            let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
            let deflated =
                SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

            zw.start_file("mimetype", stored)
                .map_err(|e| e.to_string())?;
            zw.write_all(b"application/epub+zip")
                .map_err(|e| e.to_string())?;

            zw.start_file("META-INF/container.xml", deflated)
                .map_err(|e| e.to_string())?;
            zw.write_all(container_xml.as_bytes())
                .map_err(|e| e.to_string())?;

            zw.start_file("OEBPS/content.opf", deflated)
                .map_err(|e| e.to_string())?;
            zw.write_all(opf_xml.as_bytes()).map_err(|e| e.to_string())?;

            zw.start_file("OEBPS/Styles/stylesheet.css", deflated)
                .map_err(|e| e.to_string())?;
            zw.write_all(EPUB_CSS.as_bytes())
                .map_err(|e| e.to_string())?;

            for (img_name, img_bytes) in image_files {
                let img_path = if img_name.starts_with("OEBPS/") {
                    img_name.to_string()
                } else {
                    format!("OEBPS/Images/{}", img_name)
                };
                zw.start_file(img_path, stored).map_err(|e| e.to_string())?;
                zw.write_all(img_bytes).map_err(|e| e.to_string())?;
            }

            for (text_path, text_content) in html_files {
                zw.start_file(text_path, deflated)
                    .map_err(|e| e.to_string())?;
                zw.write_all(text_content.as_bytes())
                    .map_err(|e| e.to_string())?;
            }

            zw.finish().map_err(|e| e.to_string())?;
        }
        Ok(buf.into_inner())
    }

    pub fn get_container_xml(&self) -> String {
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<container version=\"1.0\" xmlns=\"urn:oasis:names:tc:opendocument:xmlns:container\">\n    <rootfiles>\n        <rootfile full-path=\"OEBPS/content.opf\" media-type=\"application/oebps-package+xml\"/>\n    </rootfiles>\n</container>".to_string()
    }

    pub fn render_full_image_xhtml_custom(
        &self,
        title: &str,
        filename: &str,
        width: u32,
        height: u32,
    ) -> String {
        let safe_title = html_escape(title);
        render_full_image(&self.language, &safe_title, width, height, filename)
    }

    pub fn build_section_parts_custom(&self, sec: &Section, prefix: &str) -> RenderedSectionParts {
        let safe_title = html_escape(&sec.title);
        let title_parts: Vec<&str> = sec.title.splitn(2, ": ").collect();
        let main_heading = html_escape(title_parts[0]);
        let sub_heading = if title_parts.len() > 1 {
            html_escape(title_parts[1])
        } else {
            String::new()
        };
        let sub_heading_block = if !sub_heading.is_empty() {
            format!("  <h2 class=\"chapter-subtitle\">{}</h2>", sub_heading)
        } else {
            String::new()
        };

        let is_narrative_section = !self.non_narrative_sec_re.is_match(title_parts[0].trim());

        let mut body_parts: Vec<String> = Vec::new();
        let mut is_first_in_block = true;
        let mut has_narrative_started = false;

        for item in &sec.content_items {
            if item.item_type == "full_illustration" {
                if let ContentItemPayload::Image(ref img) = item.payload {
                    let fname = if !prefix.is_empty() {
                        format!("{}_{}", prefix, img.filename)
                    } else {
                        img.filename.clone()
                    };
                    body_parts.push(format!(
                        "  <div class=\"full-illustration-wrap\"><img src=\"../Images/{}\" alt=\"Ilustración\" class=\"full-illustration-img\" /></div>",
                        fname
                    ));
                    is_first_in_block = true;
                }
            } else if item.item_type == "image" {
                if let ContentItemPayload::Image(ref img) = item.payload {
                    let fname = if !prefix.is_empty() {
                        format!("{}_{}", prefix, img.filename)
                    } else {
                        img.filename.clone()
                    };
                    body_parts.push(format!(
                        "  <div class=\"inline-image\"><img src=\"../Images/{}\" alt=\"Ilustración\" /></div>",
                        fname
                    ));
                    is_first_in_block = true;
                }
            } else if item.item_type == "scene_break" {
                let break_text = if let ContentItemPayload::Paragraph(ref p) = item.payload {
                    let trimmed = p.text.trim();
                    if !trimmed.is_empty() {
                        html_escape(trimmed)
                    } else {
                        "◆ ◇ ◆ ◇ ◆".to_string()
                    }
                } else {
                    "◆ ◇ ◆ ◇ ◆".to_string()
                };
                body_parts.push(format!(
                    "  <div class=\"scene-break\" data-symbol=\"{}\" data-scene-break=\"true\">{}</div>",
                    break_text, break_text
                ));
                is_first_in_block = true;
            } else if item.item_type == "part_heading" {
                if let ContentItemPayload::Paragraph(ref p) = item.payload {
                    let mut escaped_part = html_escape(&p.text);
                    if escaped_part.contains('\u{E000}') || escaped_part.contains('\u{E001}') {
                        escaped_part = escaped_part
                            .replace('\u{E000}', "<strong>")
                            .replace('\u{E001}', "</strong>");
                    }
                    let anchor_attr = if !item.anchor.is_empty() {
                        format!(" id=\"{}\"", item.anchor)
                    } else {
                        String::new()
                    };
                    body_parts.push(format!(
                        "  <h3 class=\"part-heading\"{}>{}</h3>",
                        anchor_attr, escaped_part
                    ));
                    is_first_in_block = true;
                }
            } else if item.item_type == "text" {
                let ContentItemPayload::Paragraph(ref p) = item.payload else {
                    continue;
                };
                if p.text.is_empty() {
                    continue;
                }
                let mut escaped_p = html_escape(&p.text);
                if escaped_p.contains('\u{E000}') || escaped_p.contains('\u{E001}') {
                    escaped_p = escaped_p
                        .replace('\u{E000}', "<strong>")
                        .replace('\u{E001}', "</strong>");
                }
                escaped_p = self
                    .fn_token_re
                    .replace_all(
                        &escaped_p,
                        "<a href=\"#fn-$1\" id=\"fnref-$1\" class=\"noteref\" epub:type=\"noteref\"><sup>[$1]</sup></a>",
                    )
                    .to_string();
                escaped_p = self
                    .sup_token_re
                    .replace_all(&escaped_p, "<sup class=\"ordinal-sup\">$1</sup>")
                    .to_string();

                if p.style == "verse_or_chant" {
                    let verse_lines: Vec<String> = escaped_p
                        .split('\n')
                        .map(|l| l.trim())
                        .filter(|l| !l.is_empty())
                        .map(|l| format!("    <p class=\"verse-line\">{}</p>", l))
                        .collect();
                    body_parts.push(format!(
                        "  <div class=\"verse-block\">\n{}\n  </div>",
                        verse_lines.join("\n")
                    ));
                    is_first_in_block = true;
                    has_narrative_started = true;
                } else if p.style == "epistolary_doc" {
                    body_parts.push(format!(
                        "  <blockquote class=\"document-card\"><p>{}</p></blockquote>",
                        escaped_p
                    ));
                    is_first_in_block = true;
                    has_narrative_started = true;
                } else {
                    let mut css_classes: Vec<&str> = Vec::new();
                    let plain_text = self.strip_tokens_re.replace_all(&p.text, "");
                    let plain_trimmed = plain_text.trim();
                    if is_first_in_block {
                        css_classes.push("first-paragraph");
                        if !has_narrative_started
                            && self.preset.enable_drop_caps
                            && is_narrative_section
                            && p.style == "narrative"
                            && plain_trimmed.chars().count() >= 90
                            && plain_trimmed.chars().next().is_some_and(char::is_alphabetic)
                        {
                            css_classes.push("drop-cap");
                        }
                    }

                    if p.style == "dialogue" {
                        css_classes.push("dialogue");
                    } else if p.style == "bracket_speech" {
                        css_classes.push("bracket-dialogue");
                    } else if p.style == "thought" {
                        css_classes.push("thought");
                    }

                    let class_attr = if !css_classes.is_empty() {
                        format!(" class=\"{}\"", css_classes.join(" "))
                    } else {
                        String::new()
                    };
                    body_parts.push(format!("  <p{}>{}</p>", class_attr, escaped_p));
                    is_first_in_block = false;
                    has_narrative_started = true;
                }

                if self.preset.footnote_style == "callout_card" && !p.footnotes.is_empty() {
                    for fn_item in &p.footnotes {
                        let esc_fn = html_escape(&fn_item.text);
                        body_parts.push(format!(
                            "  <aside id=\"{}\" class=\"footnote-card\" epub:type=\"footnote\">\n    <div class=\"footnote-card-tag\">Nota {}</div>\n    <div class=\"footnote-card-body\">{}</div>\n  </aside>",
                            fn_item.id, fn_item.number, esc_fn
                        ));
                    }
                }
            }
        }

        if self.preset.footnote_style == "end_of_chapter" && !sec.footnotes.is_empty() {
            let mut fn_parts: Vec<String> = Vec::new();
            fn_parts.push("  <section class=\"chapter-footnotes\" epub:type=\"footnotes\">".to_string());
            fn_parts.push("    <hr class=\"footnotes-divider\" />".to_string());
            fn_parts.push("    <h4 class=\"footnotes-heading\">Notas al pie</h4>".to_string());
            for fn_item in &sec.footnotes {
                let esc_fn_text = html_escape(&fn_item.text);
                fn_parts.push(format!(
                    "    <aside id=\"{}\" class=\"footnote\" epub:type=\"footnote\">\n      <p><a href=\"#fnref-{}\" class=\"footnote-backref\" title=\"Volver al texto\">{}.</a> {}</p>\n    </aside>",
                    fn_item.id, fn_item.number, fn_item.number, esc_fn_text
                ));
            }
            fn_parts.push("  </section>".to_string());
            body_parts.push(fn_parts.join("\n"));
        }

        RenderedSectionParts {
            safe_title,
            main_heading,
            sub_heading_block,
            body_joined: body_parts.join("\n"),
        }
    }

    pub fn render_section_body_html(&self, sec: &Section, prefix: &str) -> String {
        let parts = self.build_section_parts_custom(sec, prefix);
        let mut out = format!("  <h1 class=\"chapter-title\">{}</h1>\n", parts.main_heading);
        if !parts.sub_heading_block.is_empty() {
            out.push_str(&parts.sub_heading_block);
            out.push('\n');
        }
        out.push_str(&parts.body_joined);
        out
    }

    pub fn render_section_xhtml_custom(&self, sec: &Section, prefix: &str) -> String {
        let sec_type = sec.section_type.as_deref().unwrap_or_else(|| {
            classify_section_type(&sec.title)
        });
        if let Some(ref custom_html) = sec.custom_html {
            let sanitized = sanitize_html_for_xhtml(custom_html);
            return render_custom_section_xhtml(&self.language, &html_escape(&sec.title), &sanitized, sec_type);
        }
        let parts = self.build_section_parts_custom(sec, prefix);
        render_section(
            &self.language,
            &parts.safe_title,
            &parts.main_heading,
            &parts.sub_heading_block,
            &parts.body_joined,
            sec_type,
        )
    }

    pub fn render_nav_xhtml_nodes(
        &self,
        volume_title: &str,
        toc_tree: &[TocNode],
        landmarks: &[Landmark],
    ) -> String {
        let safe_vol_title = html_escape(volume_title);
        let mut items = Vec::new();
        for node in toc_tree {
            items.push(self.render_toc_node_html(node, 6));
        }

        let landmarks_items: Vec<String> = landmarks
            .iter()
            .map(|l| {
                format!(
                    "      <li><a epub:type=\"{}\" href=\"{}\">{}</a></li>",
                    l.epub_type,
                    l.href,
                    html_escape(&l.label)
                )
            })
            .collect();
        let landmarks_str = landmarks_items.join("\n");

        render_nav(&self.language, &safe_vol_title, &items.join("\n"), &landmarks_str)
    }

    fn render_toc_node_html(&self, node: &TocNode, indent: usize) -> String {
        let pad = " ".repeat(indent);
        let title_esc = html_escape(&node.title);
        if node.children.is_empty() {
            format!("{}<li><a href=\"{}\">{}</a></li>", pad, node.href, title_esc)
        } else {
            let child_lines: Vec<String> = node
                .children
                .iter()
                .map(|c| self.render_toc_node_html(c, indent + 4))
                .collect();
            format!(
                "{}<li>\n{}  <a href=\"{}\">{}</a>\n{}  <ol>\n{}\n{}  </ol>\n{}</li>",
                pad,
                pad,
                node.href,
                title_esc,
                pad,
                child_lines.join("\n"),
                pad,
                pad
            )
        }
    }

    pub fn render_nav_xhtml(
        &self,
        volume_title: &str,
        toc_tree: &[(String, String, Vec<(String, String)>)],
        landmarks: &[(String, String, String)],
    ) -> String {
        let nodes: Vec<TocNode> = toc_tree
            .iter()
            .map(|(p_title, p_href, children)| {
                let c_nodes = children
                    .iter()
                    .map(|(c_t, c_h)| TocNode::leaf(c_t, c_h))
                    .collect();
                TocNode::branch(p_title, p_href, c_nodes)
            })
            .collect();

        let l_nodes: Vec<Landmark> = landmarks
            .iter()
            .map(|(ep, href, lbl)| Landmark {
                epub_type: ep.clone(),
                href: href.clone(),
                label: lbl.clone(),
            })
            .collect();

        self.render_nav_xhtml_nodes(volume_title, &nodes, &l_nodes)
    }

    pub fn render_ncx_nodes(&self, volume_title: &str, toc_tree: &[TocNode]) -> String {
        let safe_title = html_escape(volume_title);
        let mut play_order = 1usize;
        let mut points = Vec::new();

        for node in toc_tree {
            points.push(self.render_ncx_node_xml(node, &mut play_order, 4));
        }

        let navpoints = points.join("\n");
        let uid = deterministic_uuid(volume_title);
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<ncx xmlns=\"http://www.daisy.org/z3986/2005/ncx/\" version=\"2005-1\">\n  <head>\n    <meta name=\"dtb:uid\" content=\"urn:uuid:{}\"/>\n    <meta name=\"dtb:depth\" content=\"3\"/>\n    <meta name=\"dtb:totalPageCount\" content=\"0\"/>\n    <meta name=\"dtb:maxPageNumber\" content=\"0\"/>\n  </head>\n  <docTitle><text>{}</text></docTitle>\n  <navMap>\n{}\n  </navMap>\n</ncx>",
            uid, safe_title, navpoints
        )
    }

    fn render_ncx_node_xml(&self, node: &TocNode, play_order: &mut usize, indent: usize) -> String {
        let pad = " ".repeat(indent);
        let np_id = format!("navpoint-{}", *play_order);
        let curr_order = *play_order;
        *play_order += 1;
        let title_esc = html_escape(&node.title);

        if node.children.is_empty() {
            format!(
                "{}<navPoint id=\"{}\" playOrder=\"{}\">\n{}  <navLabel><text>{}</text></navLabel>\n{}  <content src=\"{}\"/>\n{}</navPoint>",
                pad, np_id, curr_order, pad, title_esc, pad, node.href, pad
            )
        } else {
            let child_xml: Vec<String> = node
                .children
                .iter()
                .map(|c| self.render_ncx_node_xml(c, play_order, indent + 2))
                .collect();
            format!(
                "{}<navPoint id=\"{}\" playOrder=\"{}\">\n{}  <navLabel><text>{}</text></navLabel>\n{}  <content src=\"{}\"/>\n{}\n{}</navPoint>",
                pad, np_id, curr_order, pad, title_esc, pad, node.href, child_xml.join("\n"), pad
            )
        }
    }

    pub fn render_ncx(
        &self,
        volume_title: &str,
        toc_tree: &[(String, String, Vec<(String, String)>)],
    ) -> String {
        let nodes: Vec<TocNode> = toc_tree
            .iter()
            .map(|(p_title, p_href, children)| {
                let c_nodes = children
                    .iter()
                    .map(|(c_t, c_h)| TocNode::leaf(c_t, c_h))
                    .collect();
                TocNode::branch(p_title, p_href, c_nodes)
            })
            .collect();
        self.render_ncx_nodes(volume_title, &nodes)
    }

    pub fn render_opf_metadata(
        &self,
        title: &str,
        author: Option<&str>,
        publisher: Option<&str>,
        description: Option<&str>,
        manifest: &[ManifestItem],
        spine: &[String],
    ) -> String {
        let manifest_lines: Vec<String> = manifest
            .iter()
            .map(|item| {
                let prop_str = item
                    .properties
                    .as_ref()
                    .map(|p| format!(" properties=\"{}\"", p))
                    .unwrap_or_default();
                format!(
                    "    <item id=\"{}\" href=\"{}\" media-type=\"{}\"{}/>",
                    item.id, item.href, item.media_type, prop_str
                )
            })
            .collect();

        let spine_lines: Vec<String> = spine
            .iter()
            .map(|item_id| format!("    <itemref idref=\"{}\"/>", item_id))
            .collect();

        let uid = format!("urn:uuid:{}", deterministic_uuid(title));
        let pub_tag = match publisher {
            Some(p) if !p.trim().is_empty() => {
                format!("\n    <dc:publisher>{}</dc:publisher>", html_escape(p.trim()))
            }
            _ => String::new(),
        };
        let desc_tag = match description {
            Some(d) if !d.trim().is_empty() => {
                format!("\n    <dc:description>{}</dc:description>", html_escape(d.trim()))
            }
            _ => String::new(),
        };

        let isbn_tag = match self.custom_isbn.as_deref() {
            Some(isbn) if !isbn.trim().is_empty() => {
                let clean = html_escape(isbn.trim());
                format!(
                    "\n    <dc:identifier id=\"pub-isbn\">{}</dc:identifier>\n    <meta refines=\"#pub-isbn\" property=\"identifier-type\" scheme=\"onix:codelist5\">15</meta>",
                    clean
                )
            }
            _ => String::new(),
        };

        let date_tag = match self.custom_published_year.as_deref() {
            Some(yr) if !yr.trim().is_empty() => {
                format!("\n    <dc:date>{}</dc:date>", html_escape(yr.trim()))
            }
            _ => String::new(),
        };

        let translator_tag = match self.custom_translator.as_deref() {
            Some(tr) if !tr.trim().is_empty() => {
                let clean = html_escape(tr.trim());
                format!(
                    "\n    <dc:contributor id=\"translator\">{}</dc:contributor>\n    <meta refines=\"#translator\" property=\"role\" scheme=\"marc:relators\">trl</meta>",
                    clean
                )
            }
            _ => String::new(),
        };

        let rights_tag = match self.custom_copyright_notice.as_deref() {
            Some(cr) if !cr.trim().is_empty() => {
                format!("\n    <dc:rights>{}</dc:rights>", html_escape(cr.trim()))
            }
            _ => String::new(),
        };

        let author_escaped = html_escape(author.unwrap_or(""));

        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<package xmlns=\"http://www.idpf.org/2007/opf\" version=\"3.0\" unique-identifier=\"book-id\" xml:lang=\"{}\">\n  <metadata xmlns:dc=\"http://purl.org/dc/elements/1.1/\">\n    <dc:identifier id=\"book-id\">{}</dc:identifier>{}\n    <dc:title>{}</dc:title>\n    <dc:language>{}</dc:language>\n    <dc:creator>{}</dc:creator>{}{}{}{}{}\n    <meta property=\"dcterms:modified\">2026-09-12T00:00:00Z</meta>\n  </metadata>\n  <manifest>\n{}\n  </manifest>\n  <spine toc=\"ncx\">\n{}\n  </spine>\n</package>",
            &self.language,
            uid,
            isbn_tag,
            html_escape(title),
            &self.language,
            author_escaped,
            translator_tag,
            pub_tag,
            date_tag,
            rights_tag,
            desc_tag,
            manifest_lines.join("\n"),
            spine_lines.join("\n")
        )
    }

    pub fn render_opf(
        &self,
        title: &str,
        manifest: &[(String, String, String, Option<String>)],
        spine: &[String],
    ) -> String {
        let typed_manifest: Vec<ManifestItem> = manifest
            .iter()
            .map(|(id, href, media, props)| ManifestItem {
                id: id.clone(),
                href: href.clone(),
                media_type: media.clone(),
                properties: props.clone(),
            })
            .collect();

        let author = self.custom_author.as_deref();
        let publisher = self.custom_publisher.as_deref();
        let description = self.custom_description.as_deref();

        self.render_opf_metadata(
            title,
            author,
            publisher,
            description,
            &typed_manifest,
            spine,
        )
    }
}

pub fn sanitize_html_for_xhtml(html: &str) -> String {
    let normalized = html.replace("&nbsp;", "&#160;");
    let settings = RewriteStrSettings::new()
        .append_element_content_handler(element!("span.noteref", |el| {
            let fn_num = el
                .get_attribute("data-fn-id")
                .and_then(|v| {
                    v.strip_prefix("fn-")
                        .map(|s| s.to_string())
                        .or(Some(v))
                })
                .or_else(|| {
                    el.get_attribute("id").and_then(|v| {
                        v.strip_prefix("fnref-")
                            .map(|s| s.to_string())
                            .or(Some(v))
                    })
                })
                .unwrap_or_else(|| "1".to_string());
            let _ = el.set_tag_name("a");
            el.remove_attribute("data-fn-id");
            el.remove_attribute("href");
            el.remove_attribute("id");
            el.remove_attribute("class");
            el.remove_attribute("epub:type");
            el.remove_attribute("role");
            let _ = el.set_attribute("href", &format!("#fn-{fn_num}"));
            let _ = el.set_attribute("id", &format!("fnref-{fn_num}"));
            let _ = el.set_attribute("class", "noteref");
            let _ = el.set_attribute("epub:type", "noteref");
            let _ = el.set_attribute("role", "doc-noteref");
            Ok(())
        }))
        .append_element_content_handler(element!("img, br, hr, input, wbr", |el| {
            let tag = el.tag_name();
            let mut rebuilt = format!("<{}", tag);
            for attr in el.attributes() {
                let name = attr.name();
                if name == "/" {
                    continue;
                }
                rebuilt.push_str(&format!(" {}=\"{}\"", name, html_escape(&attr.value())));
            }
            rebuilt.push_str(" />");
            el.replace(&rebuilt, ContentType::Html);
            Ok(())
        }));
    rewrite_str(&normalized, settings).unwrap_or(normalized)
}

pub fn html_escape(s: &str) -> String {
    quick_xml::escape::escape(s).to_string()
}

pub fn deterministic_title_hash(title: &str) -> u64 {
    let mut h: u64 = 14695981039346656037;
    for b in title.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    h
}

pub fn detect_synopsis_from_sections(sections: &[Section]) -> Option<String> {
    let rx = get_static_regexes();

    for sec in sections.iter().take(6) {
        if rx.synopsis_title_re.is_match(sec.title.trim()) {
            let paras: Vec<&str> = if !sec.paragraphs.is_empty() {
                sec.paragraphs
                    .iter()
                    .map(|p| p.text.trim())
                    .filter(|t| !t.is_empty())
                    .collect()
            } else {
                sec.content_items
                    .iter()
                    .filter_map(|item| {
                        if let ContentItemPayload::Paragraph(ref p) = item.payload {
                            let t = p.text.trim();
                            if !t.is_empty() {
                                Some(t)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    })
                    .collect()
            };

            if !paras.is_empty() {
                let joined = paras.join("\n\n");
                if joined.len() >= 20 {
                    return Some(joined);
                }
            }
        }
    }

    for sec in sections.iter().take(4) {
        let mut collecting = false;
        let mut collected: Vec<String> = Vec::new();

        let paras: Vec<(&str, bool, bool)> = if !sec.paragraphs.is_empty() {
            sec.paragraphs
                .iter()
                .map(|p| (p.text.trim(), p.is_scene_break, p.is_part_heading))
                .collect()
        } else {
            sec.content_items
                .iter()
                .filter_map(|item| {
                    if let ContentItemPayload::Paragraph(ref p) = item.payload {
                        Some((p.text.trim(), p.is_scene_break, p.is_part_heading))
                    } else {
                        None
                    }
                })
                .collect()
        };

        for (text, is_break, is_heading) in paras {
            if text.is_empty() {
                continue;
            }

            if let Some(cap) = rx.synopsis_prefix_re.captures(text) {
                let remainder = cap[1].trim();
                if !remainder.is_empty() {
                    collected.push(remainder.to_string());
                    collecting = true;
                    continue;
                }
            }

            if rx.synopsis_alone_re.is_match(text) {
                collecting = true;
                continue;
            }

            if collecting {
                if is_break || is_heading {
                    break;
                }
                collected.push(text.to_string());
            }
        }

        if !collected.is_empty() {
            let joined = collected.join("\n\n");
            if joined.len() >= 20 {
                return Some(joined);
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_synopsis_from_section_title() {
        let mut sec = Section::new("Sinopsis".to_string(), None, 1, 1);
        sec.paragraphs.push(Paragraph::new("Esta es una novela ligera que relata las aventuras del héroe.".to_string()));
        let sections = vec![sec];
        let synopsis = detect_synopsis_from_sections(&sections);
        assert_eq!(
            synopsis,
            Some("Esta es una novela ligera que relata las aventuras del héroe.".to_string())
        );
    }

    #[test]
    fn test_detect_synopsis_from_paragraph_prefix() {
        let mut sec = Section::new("Prólogo".to_string(), None, 1, 1);
        sec.paragraphs.push(Paragraph::new("Sinopsis: Esta es la emocionante historia que comienza aquí.".to_string()));
        let sections = vec![sec];
        let synopsis = detect_synopsis_from_sections(&sections);
        assert_eq!(
            synopsis,
            Some("Esta es la emocionante historia que comienza aquí.".to_string())
        );
    }

    #[test]
    fn test_detect_synopsis_none() {
        let mut sec = Section::new("Capítulo 1".to_string(), None, 1, 1);
        sec.paragraphs.push(Paragraph::new("Texto normal de la novela sin mención de resumen.".to_string()));
        let sections = vec![sec];
        let synopsis = detect_synopsis_from_sections(&sections);
        assert_eq!(synopsis, None);
    }

    #[test]
    fn test_epub_package_zip_and_xml_validity() {
        let builder = EpubBuilder::new();
        let mut sec = Section::new("Capítulo 1: El Comienzo".to_string(), None, 1, 2);
        sec.paragraphs.push(Paragraph::with_style("—Hola mundo.", "dialogue"));
        let vol = VolumeInfo {
            pdf_path: "test.pdf".to_string(),
            volume_number: 1,
            title: "Test Novel - Volumen 01".to_string(),
            total_pages: 10,
            cover_image: None,
            color_images: Vec::new(),
            sections: vec![sec],
            all_verdicts: Vec::new(),
            spell_transformations: Vec::new(),
            publisher: Some("Test Publisher".to_string()),
            description: Some("Test Synopsis".to_string()),
        };

        let bytes = builder.build_epub_bytes(&vol).expect("Failed to build EPUB");
        assert!(!bytes.is_empty());

        let reader = Cursor::new(&bytes);
        let mut zip = zip::ZipArchive::new(reader).expect("Invalid ZIP archive");

        {
            let mut mimetype_file = zip.by_name("mimetype").expect("Missing mimetype");
            let mut mime_content = Vec::new();
            std::io::Read::read_to_end(&mut mimetype_file, &mut mime_content).unwrap();
            assert_eq!(mime_content, b"application/epub+zip");
            assert_eq!(mimetype_file.compression(), CompressionMethod::Stored);
        }

        let opf_str = {
            let mut opf_file = zip.by_name("OEBPS/content.opf").expect("Missing content.opf");
            let mut s = String::new();
            std::io::Read::read_to_string(&mut opf_file, &mut s).unwrap();
            s
        };
        assert!(opf_str.contains("<dc:publisher>Test Publisher</dc:publisher>"));
        assert!(opf_str.contains("<dc:description>Test Synopsis</dc:description>"));
        assert!(opf_str.contains("urn:uuid:"));

        let mut xml_reader = quick_xml::Reader::from_str(&opf_str);
        let mut buf = Vec::new();
        loop {
            match xml_reader.read_event_into(&mut buf) {
                Ok(quick_xml::events::Event::Eof) => break,
                Err(e) => panic!("XML validation failed in OPF: {:?}", e),
                _ => {}
            }
            buf.clear();
        }

        let nav_str = {
            let mut nav_file = zip.by_name("OEBPS/nav.xhtml").expect("Missing nav.xhtml");
            let mut s = String::new();
            std::io::Read::read_to_string(&mut nav_file, &mut s).unwrap();
            s
        };
        let mut xml_reader_nav = quick_xml::Reader::from_str(&nav_str);
        loop {
            match xml_reader_nav.read_event_into(&mut buf) {
                Ok(quick_xml::events::Event::Eof) => break,
                Err(e) => panic!("XML validation failed in Nav: {:?}", e),
                _ => {}
            }
            buf.clear();
        }
    }
}
