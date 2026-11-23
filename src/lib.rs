pub mod cleaner;
pub mod config;
pub mod detector;
pub mod epub_builder;
pub mod image_classifier;
pub mod models;
pub mod pdf_extractor;
pub mod spell_corrector;
pub mod templates;
pub mod text_utils;

use crate::config::{
    infer_series_and_volume, resolve_language, LanguageProfile, ParseOptions, PresetProfile,
};
use crate::epub_builder::EpubBuilder;
use crate::models::{
    ocr_page_to_pdf_page_data, ContentItem, ContentItemPayload, ExtractedImage, OcrPageInput,
    Paragraph, PdfPageData, Section, VolumeInfo,
};
use crate::pdf_extractor::{detect_scanned_pages_from_pages, ParsedPdfDocument};
use aho_corasick::{AhoCorasick, AhoCorasickBuilder, MatchKind};
use lol_html::{element, html_content::ContentType, rewrite_str, text, RewriteStrSettings};
use rc_zip_sync::ReadZip;
use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::io::Cursor;
use std::sync::{Arc, OnceLock, RwLock};

pub struct StoredVolume {
    pub volume: VolumeInfo,
    pub section_ids: Vec<String>,
    pub raw_pages: Vec<PdfPageData>,
    pub scanned_page_numbers: Vec<usize>,
}

static LOADED_VOLUMES: OnceLock<RwLock<HashMap<String, Arc<StoredVolume>>>> = OnceLock::new();
static CUSTOM_COVER: OnceLock<RwLock<Option<ExtractedImage>>> = OnceLock::new();

fn loaded_volumes_store() -> &'static RwLock<HashMap<String, Arc<StoredVolume>>> {
    LOADED_VOLUMES.get_or_init(|| RwLock::new(HashMap::new()))
}

fn custom_cover_store() -> &'static RwLock<Option<ExtractedImage>> {
    CUSTOM_COVER.get_or_init(|| RwLock::new(None))
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FrontendBookMetadata {
    pub title: String,
    pub author: String,
    pub illustrator: String,
    pub series: String,
    pub volume: String,
    pub language: String,
    pub publisher: String,
    pub description: String,
    #[serde(default)]
    pub isbn: Option<String>,
    #[serde(default)]
    pub published_year: Option<String>,
    #[serde(default)]
    pub copyright_notice: Option<String>,
    #[serde(default)]
    pub translator: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FrontendChapterSection {
    pub id: String,
    pub file_id: String,
    pub original_title: String,
    pub title: String,
    #[serde(rename = "type")]
    pub section_type: String,
    pub order: usize,
    pub html_content: String,
    pub start_page: usize,
    pub end_page: usize,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FrontendFileSectionGroup {
    pub file_id: String,
    pub file_name: String,
    pub sections: Vec<FrontendChapterSection>,
    pub collapsed: bool,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FrontendExtractedImage {
    pub id: String,
    pub file_id: String,
    pub name: String,
    pub url: String,
    pub resolution: String,
    pub is_discarded: bool,
    #[serde(rename = "type")]
    pub image_type: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FrontendEditorialRule {
    pub id: String,
    pub label: String,
    pub description: String,
    pub enabled: bool,
    pub affected_count: usize,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FrontendRegexRule {
    pub id: String,
    pub pattern: String,
    pub replacement: String,
    pub enabled: bool,
    pub flags: String,
    pub affected_count: usize,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FrontendGlossaryReplacement {
    pub id: String,
    pub find: String,
    pub replace: String,
    pub enabled: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WasmParsedBookPayload {
    pub metadata: FrontendBookMetadata,
    pub groups: Vec<FrontendFileSectionGroup>,
    pub images: Vec<FrontendExtractedImage>,
    pub detected_rules: Vec<FrontendEditorialRule>,
    pub spell_corrections: Vec<crate::models::FrontendSpellCorrection>,
    pub ignored_japanese_count: usize,
    pub ignored_foreign_count: usize,
    pub ignored_proper_nouns_count: usize,
    #[serde(default)]
    pub needs_ocr: bool,
    #[serde(default)]
    pub scanned_page_numbers: Vec<usize>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpellAuditResult {
    pub corrections: Vec<crate::models::FrontendSpellCorrection>,
    pub ignored_japanese_count: usize,
    pub ignored_foreign_count: usize,
    pub ignored_proper_nouns_count: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WasmExportRequest {
    pub metadata: FrontendBookMetadata,
    pub groups: Vec<FrontendFileSectionGroup>,
    pub images: Vec<FrontendExtractedImage>,
    pub editorial_rules: Vec<FrontendEditorialRule>,
    pub regex_rules: Vec<FrontendRegexRule>,
    pub glossary: Vec<FrontendGlossaryReplacement>,
    pub spell_corrections: Option<Vec<crate::models::FrontendSpellCorrection>>,
    pub global_spell_enabled: Option<bool>,
    pub volume_spell_settings: Option<HashMap<String, bool>>,
    pub remove_cover: Option<bool>,
}

pub fn engine_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

fn classify_section_type(title: &str) -> &'static str {
    crate::templates::classify_section_type(title)
}

fn classify_frontend_image_type(img: &ExtractedImage) -> &'static str {
    if img.is_cover || img.category == "covers" {
        "cover"
    } else if img.category == "color_inserts" {
        "color"
    } else if img.category == "story_illustrations" {
        "bw"
    } else {
        "tiled"
    }
}

fn compute_detected_rules(vol: &VolumeInfo) -> Vec<FrontendEditorialRule> {
    let mut dash_count = 0usize;
    let mut quote_count = 0usize;
    let mut ellipsis_count = 0usize;
    let mut break_count = 0usize;

    for sec in &vol.sections {
        for item in &sec.content_items {
            if item.item_type == "scene_break" {
                break_count += 1;
            } else if let ContentItemPayload::Paragraph(ref p) = item.payload {
                if p.style == "dialogue" || p.text.contains('—') {
                    dash_count += 1;
                }
                if p.style == "bracket_speech"
                    || p.text.contains('«')
                    || p.text.contains('»')
                    || p.text.contains('“')
                    || p.text.contains('”')
                {
                    quote_count += 1;
                }
                ellipsis_count += p.text.matches('…').count() + p.text.matches("...").count();
            }
        }
    }

    let header_and_spell_count = vol.total_pages + vol.spell_transformations.len();

    vec![
        FrontendEditorialRule {
            id: "rule-dialogue-dashes".to_string(),
            label: "Standardize Dialogue Em-Dashes".to_string(),
            description: "Replaces double hyphens and standard dashes in spoken lines with typographic em-dashes (—).".to_string(),
            enabled: true,
            affected_count: dash_count,
        },
        FrontendEditorialRule {
            id: "rule-curly-quotes".to_string(),
            label: "Smart Curly Quotes & Brackets".to_string(),
            description: "Transforms straight quotation marks and angle brackets into curved or guillemet quotes (« » / “ ”).".to_string(),
            enabled: true,
            affected_count: quote_count,
        },
        FrontendEditorialRule {
            id: "rule-ellipsis".to_string(),
            label: "Typographic Ellipses".to_string(),
            description: "Substitutes three dots (...) with a single horizontal ellipsis glyph (…).".to_string(),
            enabled: true,
            affected_count: ellipsis_count,
        },
        FrontendEditorialRule {
            id: "rule-clean-breaks".to_string(),
            label: "Consolidate Multiple Blank Lines & Breaks".to_string(),
            description: "Removes unnecessary blank lines and preserves deliberate scene transitions.".to_string(),
            enabled: true,
            affected_count: break_count,
        },
        FrontendEditorialRule {
            id: "rule-strip-headers".to_string(),
            label: "Strip Running Headers, Page Numbers & OCR Fixes".to_string(),
            description: "Strips repeated PDF page markers and applies contextual spell/OCR corrections.".to_string(),
            enabled: true,
            affected_count: header_and_spell_count,
        },
    ]
}

struct EpubParseRegexes {
    title_re: Regex,
    vol_re: Regex,
    pub_re: Regex,
    desc_re: Regex,
    tag_strip_re: Regex,
    h1_re: Regex,
    h2_re: Regex,
    block_re: Regex,
    bold_tag_re: Regex,
    span_bold_re: Regex,
}

static EPUB_PARSE_REGEXES: OnceLock<EpubParseRegexes> = OnceLock::new();

fn get_epub_parse_regexes() -> &'static EpubParseRegexes {
    EPUB_PARSE_REGEXES.get_or_init(|| EpubParseRegexes {
        title_re: Regex::new(r"(?s)<dc:title[^>]*>(.*?)</dc:title>").unwrap(),
        vol_re: Regex::new(r"(?i)(?:Volumen|Volume|Vol\.?)\s+(\d+)").unwrap(),
        pub_re: Regex::new(r"(?s)<dc:publisher[^>]*>(.*?)</dc:publisher>").unwrap(),
        desc_re: Regex::new(r"(?s)<dc:description[^>]*>(.*?)</dc:description>").unwrap(),
        tag_strip_re: Regex::new(r"<[^>]+>").unwrap(),
        h1_re: Regex::new(r"(?s)<h1[^>]*>(.*?)</h1>").unwrap(),
        h2_re: Regex::new(r"(?s)<h2[^>]*>(.*?)</h2>").unwrap(),
        block_re: Regex::new(
            r#"(?is)<(?P<tag>p|div|hr)\b(?P<attrs>[^>]*)>(?:(?P<content>.*?)</(?:p|div)>)?"#,
        )
        .unwrap(),
        bold_tag_re: Regex::new(r"(?is)<(?:strong|b)\b[^>]*>(.*?)</(?:strong|b)>").unwrap(),
        span_bold_re: Regex::new(
            r#"(?is)<span\b[^>]*style="[^"]*font-weight:\s*(?:bold|[6-9]00)[^"]*"[^>]*>(.*?)</span>"#,
        )
        .unwrap(),
    })
}

pub fn read_zip_entries_rc(zip_bytes: &[u8]) -> Result<HashMap<String, Vec<u8>>, String> {
    let archive = zip_bytes.read_zip().map_err(|e| e.to_string())?;
    let mut map = HashMap::new();
    for entry in archive.entries() {
        let data = entry.bytes().map_err(|e| e.to_string())?;
        map.insert(entry.name.to_string(), data);
    }
    Ok(map)
}

pub fn parse_epub_bytes(epub_bytes: &[u8], filename: &str) -> Result<VolumeInfo, String> {
    let (vol, _) = parse_epub_bytes_with_options(epub_bytes, filename, &ParseOptions::default())?;
    Ok(vol)
}

pub fn parse_epub_bytes_with_options(
    epub_bytes: &[u8],
    filename: &str,
    options: &ParseOptions,
) -> Result<(VolumeInfo, String), String> {
    let archive = epub_bytes.read_zip().map_err(|e| e.to_string())?;
    let rx = get_epub_parse_regexes();

    let mut opf_content = String::new();
    let mut editnovel_manifest = String::new();
    for entry in archive.entries() {
        if entry.name.ends_with(".opf") {
            let bytes = entry.bytes().map_err(|e| e.to_string())?;
            opf_content = String::from_utf8_lossy(&bytes).to_string();
            break;
        } else if entry.name == "manifest.json" {
            let bytes = entry.bytes().map_err(|e| e.to_string())?;
            editnovel_manifest = String::from_utf8_lossy(&bytes).to_string();
        }
    }

    let mut title = rx
        .title_re
        .captures(&opf_content)
        .map(|c| c[1].trim().to_string())
        .unwrap_or_else(|| {
            filename
                .trim_end_matches(".epub")
                .trim_end_matches(".editnovel")
                .to_string()
        });

    if opf_content.is_empty() && !editnovel_manifest.is_empty() {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&editnovel_manifest) {
            if let Some(t) = val
                .get("title")
                .or_else(|| val.get("metadata").and_then(|m| m.get("title")))
                .and_then(|v| v.as_str())
            {
                if !t.trim().is_empty() {
                    title = t.trim().to_string();
                }
            }
        }
    }

    let vol_num = rx
        .vol_re
        .captures(&title)
        .or_else(|| rx.vol_re.captures(filename))
        .and_then(|c| c[1].parse::<usize>().ok())
        .unwrap_or(1);

    let extracted_publisher = rx
        .pub_re
        .captures(&opf_content)
        .map(|c| c[1].trim().to_string())
        .filter(|p| !p.is_empty() && p != "Agnostic EPUB Converter");

    let mut extracted_description = rx
        .desc_re
        .captures(&opf_content)
        .map(|c| rx.tag_strip_re.replace_all(&c[1], "").trim().to_string())
        .filter(|d| !d.is_empty());

    let mut cover_image: Option<ExtractedImage> = None;
    let mut color_images: Vec<ExtractedImage> = Vec::new();
    let mut all_verdicts: Vec<ExtractedImage> = Vec::new();
    let mut xhtml_entries: Vec<(String, String)> = Vec::new();

    for entry in archive.entries() {
        let name = entry.name.to_string();
        let lower = name.to_lowercase();
        if lower.ends_with(".jpg")
            || lower.ends_with(".jpeg")
            || lower.ends_with(".png")
            || lower.ends_with(".webp")
        {
            let data = entry.bytes().map_err(|e| e.to_string())?;
            let (width, height) = image::ImageReader::new(Cursor::new(&data))
                .with_guessed_format()
                .ok()
                .and_then(|r| r.into_dimensions().ok())
                .unwrap_or((800, 1200));
            let ext = std::path::Path::new(&name)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("jpg")
                .to_string();
            let short_name = std::path::Path::new(&name)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or(&name)
                .to_string();
            let is_cov = cover_image.is_none()
                && (short_name.contains("p001")
                    || short_name.contains("cover")
                    || all_verdicts.is_empty());
            let cat = if is_cov {
                "covers"
            } else if short_name.contains("p002")
                || short_name.contains("p003")
                || short_name.contains("p004")
                || short_name.contains("color")
            {
                "color_inserts"
            } else {
                "story_illustrations"
            };
            let img = ExtractedImage {
                page_num: all_verdicts.len() + 1,
                image_index: 1,
                ext,
                data,
                width,
                height,
                filename: short_name,
                category: cat.to_string(),
                y_pos: 0.0,
                is_full_page: true,
                is_cover: is_cov,
                is_kept: true,
                reason: "EPUB embedded image".to_string(),
            };
            if is_cov {
                cover_image = Some(img.clone());
            } else if cat == "color_inserts" {
                color_images.push(img.clone());
            }
            all_verdicts.push(img);
        } else if (name.ends_with(".xhtml") || name.ends_with(".html"))
            && !name.ends_with("nav.xhtml")
            && !name.contains("cover")
            && !name.contains("color_")
            && !name.contains("colophon")
            && !name.contains("_title")
            && !name.contains("titlepage")
        {
            if let Ok(bytes) = entry.bytes() {
                if let Ok(text) = String::from_utf8(bytes) {
                    xhtml_entries.push((name, text));
                }
            }
        }
    }
    xhtml_entries.sort_by(|a, b| a.0.cmp(&b.0));

    let sample_text: String = xhtml_entries
        .iter()
        .take(5)
        .map(|(_, t)| rx.tag_strip_re.replace_all(t, " ").into_owned())
        .collect::<Vec<_>>()
        .join(" ");
    let resolved_lang = resolve_language(&options.language, &sample_text, &options.preset_id);
    let preset = PresetProfile::from_id(&options.preset_id);
    let lang_profile = LanguageProfile::for_language(resolved_lang);
    let cleaner = crate::cleaner::TextCleaner::with_profiles(lang_profile, preset);

    let mut sections: Vec<Section> = Vec::new();
    for (idx, (_, xhtml)) in xhtml_entries.into_iter().enumerate() {
        let default_h1 = if resolved_lang == "en" {
            format!("Chapter {}", idx + 1)
        } else {
            format!("Capítulo {}", idx + 1)
        };
        let h1 = rx
            .h1_re
            .captures(&xhtml)
            .map(|c| rx.tag_strip_re.replace_all(&c[1], "").trim().to_string())
            .unwrap_or(default_h1);
        let h2 = rx
            .h2_re
            .captures(&xhtml)
            .map(|c| rx.tag_strip_re.replace_all(&c[1], "").trim().to_string());
        let full_title = match h2 {
            Some(ref sub) if !sub.is_empty() => format!("{}: {}", h1, sub),
            _ => h1,
        };
        let mut sec = Section::new(full_title, h2, idx + 1, idx + 1);
        sec.section_type = Some(classify_section_type(&sec.title).to_string());
        for cap in rx.block_re.captures_iter(&xhtml) {
            let tag = cap
                .name("tag")
                .map(|m| m.as_str().to_lowercase())
                .unwrap_or_default();
            let attrs = cap.name("attrs").map(|m| m.as_str()).unwrap_or_default();
            let raw_content = cap.name("content").map(|m| m.as_str()).unwrap_or_default();

            let is_break_tag =
                tag == "hr" || attrs.contains("scene-break") || attrs.contains("scene_break");
            if tag == "div" && !is_break_tag {
                continue;
            }

            let with_bold = rx.bold_tag_re.replace_all(raw_content, "\u{E000}$1\u{E001}");
            let with_bold = rx.span_bold_re.replace_all(&with_bold, "\u{E000}$1\u{E001}");
            let inner = rx.tag_strip_re.replace_all(&with_bold, "").trim().to_string();
            let inner = crate::text_utils::merge_bold_tokens(&inner);

            if is_break_tag || cleaner.is_scene_break(&inner) || cleaner.is_subsection(&inner) {
                let sym = if inner.is_empty() {
                    "* * *".to_string()
                } else {
                    inner
                };
                sec.paragraphs.push(Paragraph::scene_break_with_text(&sym));
                sec.content_items
                    .push(ContentItem::scene_break_with_text(&sym, idx + 1));
                continue;
            }

            if inner.is_empty() {
                continue;
            }
            let initial_style = if attrs.contains("dialogue") {
                "dialogue"
            } else if attrs.contains("thought") {
                "thought"
            } else {
                "narrative"
            };
            let standardized = cleaner.standardize_paragraph_text(&inner, initial_style);
            let final_style = if attrs.contains("dialogue") || attrs.contains("thought") {
                initial_style.to_string()
            } else {
                cleaner.classify_paragraph_style(&standardized, None)
            };
            sec.paragraphs
                .push(Paragraph::with_style(standardized.clone(), &final_style));
            sec.content_items.push(ContentItem::text(
                standardized,
                final_style,
                Vec::new(),
                idx + 1,
            ));
        }
        sections.push(sec);
    }

    if extracted_description.is_none() {
        extracted_description = crate::epub_builder::detect_synopsis_from_sections(&sections);
    }

    Ok((
        VolumeInfo {
            pdf_path: filename.to_string(),
            volume_number: vol_num,
            title,
            total_pages: sections.len(),
            cover_image,
            color_images,
            sections,
            all_verdicts,
            spell_transformations: Vec::new(),
            publisher: extracted_publisher,
            description: extracted_description,
        },
        resolved_lang.to_string(),
    ))
}

fn build_parsed_book_payload(
    vol: VolumeInfo,
    filename: &str,
    builder: &EpubBuilder,
    raw_pages: Vec<PdfPageData>,
    scanned_page_numbers: Vec<usize>,
) -> Result<String, String> {
    let detected_rules = compute_detected_rules(&vol);

    let mut frontend_sections: Vec<FrontendChapterSection> =
        Vec::with_capacity(vol.sections.len() + 2);
    let mut section_ids: Vec<String> = Vec::with_capacity(vol.sections.len());
    let mut section_order = 0usize;

    if let Some(ref cov) = vol.cover_image {
        let sec_id = format!("sec-{}-cover", filename);
        let cov_html = format!(
            r#"<div class="full-illustration-wrap"><img src="../Images/{}" alt="Portada" class="full-illustration-img" /></div>"#,
            cov.filename
        );
        frontend_sections.push(FrontendChapterSection {
            id: sec_id,
            file_id: filename.to_string(),
            original_title: "Portada".to_string(),
            title: "Portada".to_string(),
            section_type: "cover".to_string(),
            order: section_order,
            html_content: cov_html,
            start_page: cov.page_num,
            end_page: cov.page_num,
        });
        section_order += 1;
    }

    let non_cover_colors: Vec<&ExtractedImage> = vol
        .color_images
        .iter()
        .filter(|img| {
            vol.cover_image
                .as_ref()
                .map_or(true, |c| c.filename != img.filename)
        })
        .collect();

    if !non_cover_colors.is_empty() {
        let sec_id = format!("sec-{}-color-inserts", filename);
        let mut color_html_parts: Vec<String> = Vec::new();
        for img in &non_cover_colors {
            color_html_parts.push(format!(
                r#"<div class="full-illustration-wrap"><img src="../Images/{}" alt="Ilustración a Color" class="full-illustration-img" /></div>"#,
                img.filename
            ));
        }
        let start_p = non_cover_colors.first().map(|i| i.page_num).unwrap_or(1);
        let end_p = non_cover_colors
            .last()
            .map(|i| i.page_num)
            .unwrap_or(start_p);
        frontend_sections.push(FrontendChapterSection {
            id: sec_id,
            file_id: filename.to_string(),
            original_title: "Ilustraciones a Color".to_string(),
            title: "Ilustraciones a Color".to_string(),
            section_type: "color".to_string(),
            order: section_order,
            html_content: color_html_parts.join("\n"),
            start_page: start_p,
            end_page: end_p,
        });
        section_order += 1;
    }

    for (idx, sec) in vol.sections.iter().enumerate() {
        let sec_id = format!("sec-{}-{}", filename, idx);
        let body_html = builder.render_section_body_html(sec, "");
        section_ids.push(sec_id.clone());
        frontend_sections.push(FrontendChapterSection {
            id: sec_id,
            file_id: filename.to_string(),
            original_title: sec.title.clone(),
            title: sec.title.clone(),
            section_type: classify_section_type(&sec.title).to_string(),
            order: section_order,
            html_content: body_html,
            start_page: sec.start_page,
            end_page: sec.end_page,
        });
        section_order += 1;
    }

    let frontend_images: Vec<FrontendExtractedImage> = vol
        .all_verdicts
        .iter()
        .enumerate()
        .map(|(idx, img)| FrontendExtractedImage {
            id: format!("img-{}-{}-{}", filename, idx, img.filename),
            file_id: filename.to_string(),
            name: img.filename.clone(),
            url: String::new(),
            resolution: format!("{}x{}", img.width, img.height),
            is_discarded: !img.is_kept,
            image_type: classify_frontend_image_type(img).to_string(),
        })
        .collect();

    let (inferred_series, _) = infer_series_and_volume(filename);
    let metadata = FrontendBookMetadata {
        title: vol.title.clone(),
        author: String::new(),
        illustrator: String::new(),
        series: inferred_series,
        volume: format!("Volumen {:02}", vol.volume_number),
        language: builder.language.clone(),
        publisher: vol.publisher.clone().unwrap_or_default(),
        description: vol.description.clone().unwrap_or_default(),
        isbn: None,
        published_year: None,
        copyright_notice: None,
        translator: None,
    };

    let group = FrontendFileSectionGroup {
        file_id: filename.to_string(),
        file_name: filename.to_string(),
        sections: frontend_sections,
        collapsed: false,
    };

    let spell_corrector = crate::spell_corrector::ContextualSpellCorrector::default();
    let mut audit_opts = crate::spell_corrector::SpellAuditOptions::default();
    if builder.language == "en" {
        audit_opts.language_mode = "en".to_string();
    }
    let (all_spell_corrections, audit_stats) = spell_corrector.audit_sections_parallel(
        filename,
        &vol.sections,
        vol.volume_number,
        &audit_opts,
    );

    let needs_ocr = !scanned_page_numbers.is_empty();

    if let Ok(mut map) = loaded_volumes_store().write() {
        map.insert(
            filename.to_string(),
            Arc::new(StoredVolume {
                volume: vol,
                section_ids,
                raw_pages,
                scanned_page_numbers: scanned_page_numbers.clone(),
            }),
        );
    }

    let payload = WasmParsedBookPayload {
        metadata,
        groups: vec![group],
        images: frontend_images,
        detected_rules,
        spell_corrections: all_spell_corrections,
        ignored_japanese_count: audit_stats.ignored_japanese_count,
        ignored_foreign_count: audit_stats.ignored_foreign_count,
        ignored_proper_nouns_count: audit_stats.ignored_proper_nouns_count,
        needs_ocr,
        scanned_page_numbers,
    };

    serde_json::to_string(&payload).map_err(|e| e.to_string())
}

pub fn parse_book_file_json(bytes: &[u8], filename: &str) -> Result<String, String> {
    parse_book_file_with_options_json(bytes, filename, "{}")
}

pub fn parse_book_file_with_options_json(
    bytes: &[u8],
    filename: &str,
    options_json: &str,
) -> Result<String, String> {
    let options: ParseOptions = serde_json::from_str(options_json).unwrap_or_default();
    let mut builder = EpubBuilder::with_options(&options);
    let lower_name = filename.to_lowercase();
    let is_epub_or_bundle =
        lower_name.ends_with(".epub") || lower_name.ends_with(".editnovel");

    let (vol, raw_pages, scanned_pages) = if is_epub_or_bundle {
        let (v, resolved_lang) = parse_epub_bytes_with_options(bytes, filename, &options)?;
        builder.language = resolved_lang;
        (v, Vec::new(), Vec::new())
    } else {
        let doc = ParsedPdfDocument::from_bytes(bytes)?;
        let scanned = doc.detect_scanned_pages();
        let pages_copy = doc.pages.clone();
        builder.configure_for_pages(&doc.pages, &options);
        let v = builder.process_parsed_pdf(&doc, filename, true);
        (v, pages_copy, scanned)
    };

    build_parsed_book_payload(vol, filename, &builder, raw_pages, scanned_pages)
}

pub fn audit_spelling_json(file_id: &str, options_json: &str) -> Result<String, String> {
    let options: crate::spell_corrector::SpellAuditOptions =
        serde_json::from_str(options_json).unwrap_or_default();
    let spell_corrector = crate::spell_corrector::ContextualSpellCorrector::default();

    let stored_opt = loaded_volumes_store()
        .read()
        .ok()
        .and_then(|map| map.get(file_id).cloned());

    let (all_corrections, stats) = if let Some(stored) = stored_opt {
        spell_corrector.audit_sections_parallel(
            file_id,
            &stored.volume.sections,
            stored.volume.volume_number,
            &options,
        )
    } else {
        (Vec::new(), crate::spell_corrector::SpellAuditStats::default())
    };

    let res = SpellAuditResult {
        corrections: all_corrections,
        ignored_japanese_count: stats.ignored_japanese_count,
        ignored_foreign_count: stats.ignored_foreign_count,
        ignored_proper_nouns_count: stats.ignored_proper_nouns_count,
    };

    serde_json::to_string(&res).map_err(|e| e.to_string())
}

pub fn audit_dynamic_paragraphs_json(request_json: &str) -> Result<String, String> {
    let req: crate::spell_corrector::DynamicAuditRequest =
        serde_json::from_str(request_json).map_err(|e| e.to_string())?;
    let corrector = crate::spell_corrector::ContextualSpellCorrector::default();
    let corrections = corrector.audit_dynamic_paragraphs(&req);
    serde_json::to_string(&corrections).map_err(|e| e.to_string())
}

pub fn set_custom_cover(filename: &str, ext: &str, data: &[u8]) {
    if let Ok(mut guard) = custom_cover_store().write() {
        *guard = Some(ExtractedImage {
            page_num: 1,
            image_index: 0,
            ext: ext.to_string(),
            data: data.to_vec(),
            width: 0,
            height: 0,
            filename: filename.to_string(),
            category: "covers".to_string(),
            y_pos: 0.0,
            is_full_page: true,
            is_cover: true,
            is_kept: true,
            reason: "User custom cover".to_string(),
        });
    }
}

pub fn clear_custom_cover() {
    if let Ok(mut guard) = custom_cover_store().write() {
        *guard = None;
    }
}

pub fn get_image_bytes(file_id: &str, image_name: &str) -> Option<Vec<u8>> {
    let custom = custom_cover_store().read().ok().and_then(|guard| {
        guard.as_ref().and_then(|img| {
            if img.filename == image_name {
                Some(img.data.clone())
            } else {
                None
            }
        })
    });
    if custom.is_some() {
        return custom;
    }

    let stored = loaded_volumes_store()
        .read()
        .ok()
        .and_then(|map| map.get(file_id).cloned())?;
    stored
        .volume
        .all_verdicts
        .iter()
        .find(|img| img.filename == image_name)
        .map(|img| img.data.clone())
}

pub fn remove_book_file(file_id: &str) {
    if let Ok(mut map) = loaded_volumes_store().write() {
        map.remove(file_id);
    }
}

struct TransformStaticRegexes {
    dialogue_dash_line_re: Regex,
    curly_quote_open_re: Regex,
    curly_quote_close_re: Regex,
    clean_breaks_re: Regex,
    empty_image_div_re: Regex,
}

static TRANSFORM_STATIC_REGEXES: OnceLock<TransformStaticRegexes> = OnceLock::new();

fn get_transform_static_regexes() -> &'static TransformStaticRegexes {
    TRANSFORM_STATIC_REGEXES.get_or_init(|| TransformStaticRegexes {
        dialogue_dash_line_re: Regex::new(r"(?m)^-\s+").unwrap(),
        curly_quote_open_re: Regex::new(r#"(^|[\s(\[>—«])"(\w)"#).unwrap(),
        curly_quote_close_re: Regex::new(r#"(\w|[.!?…,;:])""#).unwrap(),
        clean_breaks_re: Regex::new(r"(<p>\s*</p>\s*){2,}").unwrap(),
        empty_image_div_re: Regex::new(
            r#"<div\b[^>]*class="[^"]*(?:inline-image|full-illustration-wrap)[^"]*"[^>]*>\s*</div>"#,
        )
        .unwrap(),
    })
}

static REGEX_RULE_CACHE: OnceLock<RwLock<HashMap<String, Option<Regex>>>> = OnceLock::new();
static GLOSSARY_AC_CACHE: OnceLock<RwLock<HashMap<u64, Arc<(AhoCorasick, Vec<String>)>>>> =
    OnceLock::new();
static BLOCK_TRANSFORM_CACHE: OnceLock<RwLock<HashMap<(String, String), String>>> =
    OnceLock::new();

fn get_cached_regex(pattern: &str, case_insensitive: bool, multi_line: bool) -> Option<Regex> {
    let cache = REGEX_RULE_CACHE.get_or_init(|| RwLock::new(HashMap::new()));
    let key = format!("{}\u{0}{}\u{0}{}", pattern, case_insensitive, multi_line);
    if let Ok(guard) = cache.read() {
        if let Some(found) = guard.get(&key) {
            return found.clone();
        }
    }
    let compiled = RegexBuilder::new(pattern)
        .case_insensitive(case_insensitive)
        .multi_line(multi_line)
        .build()
        .ok();
    if let Ok(mut guard) = cache.write() {
        if guard.len() >= 256 {
            guard.clear();
        }
        guard.insert(key, compiled.clone());
    }
    compiled
}

fn fnv1a_update(mut hash: u64, bytes: &[u8]) -> u64 {
    for &b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(1099511628211);
    }
    hash
}

fn get_cached_glossary_ac(
    glossary: &[FrontendGlossaryReplacement],
) -> Option<Arc<(AhoCorasick, Vec<String>)>> {
    let mut active_terms: Vec<(&str, &str)> = Vec::new();
    let mut seen_finds: HashSet<&str> = HashSet::new();
    let mut sig: u64 = 14695981039346656037;

    for term in glossary {
        if !term.enabled || term.find.is_empty() {
            continue;
        }
        if seen_finds.insert(term.find.as_str()) {
            sig = fnv1a_update(sig, term.find.as_bytes());
            sig = fnv1a_update(sig, &[0]);
            sig = fnv1a_update(sig, term.replace.as_bytes());
            sig = fnv1a_update(sig, &[0xff]);
            active_terms.push((term.find.as_str(), term.replace.as_str()));
        }
    }

    if active_terms.is_empty() {
        return None;
    }

    let cache = GLOSSARY_AC_CACHE.get_or_init(|| RwLock::new(HashMap::new()));
    if let Ok(guard) = cache.read() {
        if let Some(found) = guard.get(&sig) {
            return Some(found.clone());
        }
    }

    let patterns: Vec<&str> = active_terms.iter().map(|(f, _)| *f).collect();
    let replacements: Vec<String> = active_terms.iter().map(|(_, r)| (*r).to_string()).collect();

    let ac = AhoCorasickBuilder::new()
        .match_kind(MatchKind::LeftmostLongest)
        .build(&patterns)
        .ok()?;
    let entry = Arc::new((ac, replacements));

    if let Ok(mut guard) = cache.write() {
        if guard.len() >= 32 {
            guard.clear();
        }
        guard.insert(sig, entry.clone());
    }
    Some(entry)
}

pub fn compute_rules_signature(
    editorial_rules: &[FrontendEditorialRule],
    regex_rules: &[FrontendRegexRule],
    glossary: &[FrontendGlossaryReplacement],
) -> String {
    let mut h: u64 = 14695981039346656037;
    for rule in editorial_rules {
        if rule.enabled {
            h = fnv1a_update(h, b"E:");
            h = fnv1a_update(h, rule.id.as_bytes());
            h = fnv1a_update(h, &[0]);
        }
    }
    for reg in regex_rules {
        if reg.enabled && !reg.pattern.is_empty() {
            h = fnv1a_update(h, b"R:");
            h = fnv1a_update(h, reg.pattern.as_bytes());
            h = fnv1a_update(h, &[0]);
            h = fnv1a_update(h, reg.replacement.as_bytes());
            h = fnv1a_update(h, &[0]);
            h = fnv1a_update(h, reg.flags.as_bytes());
            h = fnv1a_update(h, &[0]);
        }
    }
    for term in glossary {
        if term.enabled && !term.find.is_empty() {
            h = fnv1a_update(h, b"G:");
            h = fnv1a_update(h, term.find.as_bytes());
            h = fnv1a_update(h, &[0]);
            h = fnv1a_update(h, term.replace.as_bytes());
            h = fnv1a_update(h, &[0]);
        }
    }
    format!("{:016x}", h)
}

fn apply_text_node_transforms(
    text_str: &str,
    editorial_rules: &[FrontendEditorialRule],
    compiled_regexes: &[(Regex, &str, bool)],
    glossary_ac: Option<&(AhoCorasick, Vec<String>)>,
) -> String {
    if text_str.is_empty() {
        return String::new();
    }
    let static_rx = get_transform_static_regexes();
    let mut result = text_str.to_string();

    for rule in editorial_rules {
        if !rule.enabled {
            continue;
        }
        match rule.id.as_str() {
            "rule-dialogue-dashes" => {
                result = result.replace("--", "—");
                result = static_rx
                    .dialogue_dash_line_re
                    .replace_all(&result, "—")
                    .to_string();
            }
            "rule-curly-quotes" => {
                result = static_rx
                    .curly_quote_open_re
                    .replace_all(&result, "$1“$2")
                    .to_string();
                result = static_rx
                    .curly_quote_close_re
                    .replace_all(&result, "$1”")
                    .to_string();
            }
            "rule-ellipsis" => {
                result = result.replace("...", "…");
            }
            _ => {}
        }
    }

    for (rx, replacement, is_global) in compiled_regexes {
        if *is_global {
            result = rx.replace_all(&result, *replacement).to_string();
        } else {
            result = rx.replace(&result, *replacement).to_string();
        }
    }

    if let Some((ac, replacements)) = glossary_ac {
        result = ac.replace_all(&result, replacements);
    }

    result
}

pub fn filter_discarded_images_html(html: &str, discarded_names: &HashSet<String>) -> String {
    if discarded_names.is_empty() || !html.contains("<img") {
        return html.to_string();
    }
    let discarded_vec: Vec<String> = discarded_names.iter().cloned().collect();
    let settings =
        RewriteStrSettings::new().append_element_content_handler(element!("img", move |el| {
            if let Some(src) = el.get_attribute("src") {
                let is_disc = discarded_vec.iter().any(|d| {
                    src == *d || src.ends_with(&format!("/{}", d)) || src.contains(d.as_str())
                });
                if is_disc {
                    el.remove();
                }
            }
            Ok(())
        }));
    let rewritten = rewrite_str(html, settings).unwrap_or_else(|_| html.to_string());

    get_transform_static_regexes()
        .empty_image_div_re
        .replace_all(&rewritten, "")
        .to_string()
}

pub fn apply_transforms_internal(
    content: &str,
    editorial_rules: &[FrontendEditorialRule],
    regex_rules: &[FrontendRegexRule],
    glossary: &[FrontendGlossaryReplacement],
) -> String {
    if content.is_empty() {
        return String::new();
    }
    let static_rx = get_transform_static_regexes();
    let mut working = content.to_string();

    let clean_breaks_enabled = editorial_rules
        .iter()
        .any(|r| r.enabled && r.id == "rule-clean-breaks");
    if clean_breaks_enabled && working.contains("<p>") {
        working = static_rx.clean_breaks_re.replace_all(&working, "").to_string();
    }

    let mut compiled_regexes: Vec<(Regex, &str, bool)> = Vec::new();
    for reg in regex_rules {
        if !reg.enabled || reg.pattern.is_empty() {
            continue;
        }
        let case_insensitive = reg.flags.contains('i');
        let multi_line = reg.flags.contains('m');
        let is_global = reg.flags.contains('g') || reg.flags.is_empty();
        if let Some(rx) = get_cached_regex(&reg.pattern, case_insensitive, multi_line) {
            compiled_regexes.push((rx, reg.replacement.as_str(), is_global));
        }
    }

    let glossary_ac = get_cached_glossary_ac(glossary);
    let glossary_ref = glossary_ac.as_deref();

    if !working.contains('<') || !working.contains('>') {
        return apply_text_node_transforms(
            &working,
            editorial_rules,
            &compiled_regexes,
            glossary_ref,
        );
    }

    let wrapped = format!("<editnovel-root>{}</editnovel-root>", working);
    let text_buffer = RefCell::new(String::new());
    let settings = RewriteStrSettings::new().append_element_content_handler(text!(
        "*:not(script):not(style)",
        |t| {
            if !t.as_str().is_empty() {
                text_buffer.borrow_mut().push_str(t.as_str());
                t.remove();
            }
            if t.last_in_text_node() {
                let full_text = std::mem::take(&mut *text_buffer.borrow_mut());
                if !full_text.is_empty() {
                    let transformed = apply_text_node_transforms(
                        &full_text,
                        editorial_rules,
                        &compiled_regexes,
                        glossary_ref,
                    );
                    t.after(&transformed, ContentType::Html);
                }
            }
            Ok(())
        }
    ));
    let rewritten = rewrite_str(&wrapped, settings).unwrap_or(wrapped);

    rewritten
        .strip_prefix("<editnovel-root>")
        .unwrap_or(&rewritten)
        .strip_suffix("</editnovel-root>")
        .unwrap_or(&rewritten)
        .to_string()
}

pub fn apply_editorial_transforms_json(
    content: &str,
    editorial_rules_json: &str,
    regex_rules_json: &str,
    glossary_json: &str,
) -> String {
    let editorial_rules: Vec<FrontendEditorialRule> =
        serde_json::from_str(editorial_rules_json).unwrap_or_default();
    let regex_rules: Vec<FrontendRegexRule> =
        serde_json::from_str(regex_rules_json).unwrap_or_default();
    let glossary: Vec<FrontendGlossaryReplacement> =
        serde_json::from_str(glossary_json).unwrap_or_default();

    apply_transforms_internal(content, &editorial_rules, &regex_rules, &glossary)
}

pub fn build_epub_from_json(payload_json: &str) -> Result<Vec<u8>, String> {
    let req: WasmExportRequest =
        serde_json::from_str(payload_json).map_err(|e| e.to_string())?;

    let mut builder = EpubBuilder::new();
    if !req.metadata.language.trim().is_empty() {
        builder.language = req.metadata.language.trim().to_string();
    }
    if !req.metadata.publisher.trim().is_empty() {
        builder.custom_publisher = Some(req.metadata.publisher.trim().to_string());
    }
    if !req.metadata.description.trim().is_empty() {
        builder.custom_description = Some(req.metadata.description.trim().to_string());
    }
    if !req.metadata.series.trim().is_empty() {
        builder.custom_series_title = Some(req.metadata.series.trim().to_string());
    }
    if !req.metadata.author.trim().is_empty() {
        builder.custom_author = Some(req.metadata.author.trim().to_string());
    }
    if !req.metadata.illustrator.trim().is_empty() {
        builder.custom_illustrator = Some(req.metadata.illustrator.trim().to_string());
    }
    if let Some(ref isbn) = req.metadata.isbn {
        if !isbn.trim().is_empty() {
            builder.custom_isbn = Some(isbn.trim().to_string());
        }
    }
    if let Some(ref yr) = req.metadata.published_year {
        if !yr.trim().is_empty() {
            builder.custom_published_year = Some(yr.trim().to_string());
        }
    }
    if let Some(ref notice) = req.metadata.copyright_notice {
        if !notice.trim().is_empty() {
            builder.custom_copyright_notice = Some(notice.trim().to_string());
        }
    }
    if let Some(ref tr) = req.metadata.translator {
        if !tr.trim().is_empty() {
            builder.custom_translator = Some(tr.trim().to_string());
        }
    }
    let mut active_volumes: Vec<VolumeInfo> = Vec::new();

    let custom_cov_global = custom_cover_store()
        .read()
        .ok()
        .and_then(|guard| guard.clone());

    let loaded_snapshot: HashMap<String, Arc<StoredVolume>> = loaded_volumes_store()
        .read()
        .map(|m| m.clone())
        .unwrap_or_default();

    for (g_idx, group) in req.groups.iter().enumerate() {
        let stored_opt = loaded_snapshot.get(&group.file_id);
        let mut vol = if let Some(stored) = stored_opt {
            stored.volume.clone()
        } else {
            VolumeInfo {
                pdf_path: String::new(),
                volume_number: g_idx + 1,
                title: if !group.file_name.trim().is_empty() {
                    group.file_name.clone()
                } else {
                    format!("Parte {}", g_idx + 1)
                },
                total_pages: 0,
                cover_image: None,
                color_images: Vec::new(),
                sections: Vec::new(),
                all_verdicts: Vec::new(),
                spell_transformations: Vec::new(),
                publisher: None,
                description: None,
            }
        };
        if !req.metadata.title.trim().is_empty() && req.groups.len() == 1 {
            vol.title = req.metadata.title.trim().to_string();
        }

        let discarded_names: HashSet<String> = req
            .images
            .iter()
            .filter(|img| img.file_id == group.file_id && img.is_discarded)
            .map(|img| img.name.clone())
            .collect();

        let is_cover_removed = req.remove_cover.unwrap_or(false);
        if is_cover_removed {
            vol.cover_image = None;
        } else if let Some(cov) = custom_cov_global.clone() {
            vol.cover_image = Some(cov.clone());
            vol.all_verdicts.retain(|img| !img.is_cover);
            vol.all_verdicts.push(cov);
        } else if let Some(ref cov) = vol.cover_image {
            let cover_section_exists = group.sections.iter().any(|s| s.id.ends_with("-cover"));
            if discarded_names.contains(&cov.filename) || !cover_section_exists {
                vol.cover_image = None;
            }
        }
        let color_section_exists = group
            .sections
            .iter()
            .any(|s| s.id.ends_with("-color-inserts"));
        if !color_section_exists {
            vol.color_images.clear();
        } else {
            vol.color_images
                .retain(|img| !discarded_names.contains(&img.filename));
        }

        let mut reordered_sections: Vec<Section> = Vec::with_capacity(group.sections.len());
        for f_sec in &group.sections {
            let mut sec = if let Some(stored) = stored_opt {
                if let Some(orig_idx) = stored.section_ids.iter().position(|id| id == &f_sec.id) {
                    stored
                        .volume
                        .sections
                        .get(orig_idx)
                        .cloned()
                        .unwrap_or_else(|| Section::new(f_sec.title.clone(), None, 1, 1))
                } else {
                    Section::new(f_sec.title.clone(), None, 1, 1)
                }
            } else {
                Section::new(f_sec.title.clone(), None, 1, 1)
            };

            sec.title = f_sec.title.clone();
            sec.section_type = Some(f_sec.section_type.clone());
            if !f_sec.html_content.trim().is_empty() {
                let custom = filter_discarded_images_html(&f_sec.html_content, &discarded_names);
                sec.custom_html = Some(custom);
            }
            sec.content_items.retain(|item| match item.payload {
                ContentItemPayload::Image(ref img) => !discarded_names.contains(&img.filename),
                _ => true,
            });
            let is_volume_spell_enabled = req.global_spell_enabled.unwrap_or(true)
                && req
                    .volume_spell_settings
                    .as_ref()
                    .and_then(|m| m.get(&group.file_id))
                    .copied()
                    .unwrap_or(true);

            let active_corrections: Vec<&crate::models::FrontendSpellCorrection> = req
                .spell_corrections
                .as_ref()
                .map(|list| {
                    list.iter()
                        .filter(|c| {
                            c.file_id == group.file_id
                                && c.section_title == sec.title
                                && c.enabled
                        })
                        .collect()
                })
                .unwrap_or_default();

            let spell_glossary: Vec<FrontendGlossaryReplacement> = if is_volume_spell_enabled {
                active_corrections
                    .iter()
                    .enumerate()
                    .map(|(idx, corr)| FrontendGlossaryReplacement {
                        id: format!("sp-{}", idx),
                        find: corr.original_token.clone(),
                        replace: corr.replacement_token.clone(),
                        enabled: true,
                    })
                    .collect()
            } else {
                Vec::new()
            };

            let mut combined_glossary = spell_glossary;
            combined_glossary.extend(req.glossary.iter().cloned());

            if let Some(ref mut custom) = sec.custom_html {
                *custom = apply_transforms_internal(
                    custom,
                    &req.editorial_rules,
                    &req.regex_rules,
                    &combined_glossary,
                );
            }

            for item in &mut sec.content_items {
                if let ContentItemPayload::Paragraph(ref mut p) = item.payload {
                    p.text = apply_transforms_internal(
                        &p.text,
                        &req.editorial_rules,
                        &req.regex_rules,
                        &combined_glossary,
                    );
                }
            }
            reordered_sections.push(sec);
        }
        vol.sections = reordered_sections;
        active_volumes.push(vol);
    }

    if active_volumes.is_empty() {
        return Err("No loaded volumes available for export".to_string());
    }

    if active_volumes.len() == 1 {
        builder.build_epub_bytes(&active_volumes[0])
    } else {
        builder.build_omnibus_bytes(&active_volumes)
    }
}

pub fn init_dictionary(lang: &str, bytes: &[u8]) -> Result<(), String> {
    crate::spell_corrector::init_dictionary(lang, bytes)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransformSectionRequest {
    pub request_id: u64,
    pub section_id: String,
    pub html: String,
    pub editorial_rules: Vec<FrontendEditorialRule>,
    pub regex_rules: Vec<FrontendRegexRule>,
    pub glossary: Vec<FrontendGlossaryReplacement>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransformSectionResult {
    pub request_id: u64,
    pub section_id: String,
    pub html: String,
    pub rules_signature: String,
}

pub fn transform_section_blocks_json(request_json: &str) -> Result<String, String> {
    let req: TransformSectionRequest =
        serde_json::from_str(request_json).map_err(|e| e.to_string())?;
    let rules_signature = compute_rules_signature(
        &req.editorial_rules,
        &req.regex_rules,
        &req.glossary,
    );

    let static_rx = get_transform_static_regexes();
    let clean_breaks_enabled = req
        .editorial_rules
        .iter()
        .any(|r| r.enabled && r.id == "rule-clean-breaks");
    let source_html = if clean_breaks_enabled && req.html.contains("<p>") {
        static_rx.clean_breaks_re.replace_all(&req.html, "").to_string()
    } else {
        req.html.clone()
    };

    let block_cache = BLOCK_TRANSFORM_CACHE.get_or_init(|| RwLock::new(HashMap::new()));
    let lines: Vec<&str> = source_html.split('\n').collect();
    let mut transformed_lines: Vec<String> = Vec::with_capacity(lines.len());

    for line in lines {
        if line.trim().is_empty() {
            transformed_lines.push(line.to_string());
            continue;
        }
        let cache_key = (rules_signature.clone(), line.to_string());
        let cached_opt = block_cache
            .read()
            .ok()
            .and_then(|guard| guard.get(&cache_key).cloned());
        if let Some(hit) = cached_opt {
            transformed_lines.push(hit);
        } else {
            let out_line = apply_transforms_internal(
                line,
                &req.editorial_rules,
                &req.regex_rules,
                &req.glossary,
            );
            if let Ok(mut guard) = block_cache.write() {
                if guard.len() >= 4096 {
                    guard.clear();
                }
                guard.insert(cache_key, out_line.clone());
            }
            transformed_lines.push(out_line);
        }
    }

    let res = TransformSectionResult {
        request_id: req.request_id,
        section_id: req.section_id,
        html: transformed_lines.join("\n"),
        rules_signature,
    };
    serde_json::to_string(&res).map_err(|e| e.to_string())
}

#[derive(Deserialize)]
#[serde(untagged)]
enum OcrPagesPayload {
    List(Vec<OcrPageInput>),
    Wrapped { pages: Vec<OcrPageInput> },
}

pub fn inject_ocr_pages_json(
    file_id: &str,
    pages_json: &str,
    options_json: &str,
) -> Result<String, String> {
    let parsed_payload: OcrPagesPayload =
        serde_json::from_str(pages_json).map_err(|e| format!("Invalid OCR pages JSON: {}", e))?;
    let ocr_pages = match parsed_payload {
        OcrPagesPayload::List(list) => list,
        OcrPagesPayload::Wrapped { pages } => pages,
    };
    if ocr_pages.is_empty() {
        return Err("No OCR pages provided".to_string());
    }

    let options: ParseOptions = serde_json::from_str(options_json).unwrap_or_default();

    let existing_stored = loaded_volumes_store()
        .read()
        .ok()
        .and_then(|map| map.get(file_id).cloned());

    let (mut pages, existing_imgs, existing_pub, existing_desc) =
        if let Some(ref stored) = existing_stored {
            let mut p_vec = stored.raw_pages.clone();
            let max_pno = ocr_pages
                .iter()
                .enumerate()
                .map(|(i, p)| if p.page_number > 0 { p.page_number } else { i + 1 })
                .max()
                .unwrap_or(1);
            if p_vec.len() < max_pno {
                p_vec.resize(
                    max_pno,
                    PdfPageData {
                        width: 595.0,
                        height: 842.0,
                        blocks: Vec::new(),
                        image_refs: Vec::new(),
                    },
                );
            }
            let mut page_images_map: HashMap<usize, Vec<ExtractedImage>> = HashMap::new();
            for img in &stored.volume.all_verdicts {
                if img.is_kept && !img.is_cover && img.category != "color_inserts" {
                    page_images_map
                        .entry(img.page_num)
                        .or_default()
                        .push(img.clone());
                }
            }
            let imgs = Some((
                stored.volume.cover_image.clone(),
                stored.volume.color_images.clone(),
                page_images_map,
                stored.volume.all_verdicts.clone(),
            ));
            (
                p_vec,
                imgs,
                stored.volume.publisher.clone(),
                stored.volume.description.clone(),
            )
        } else {
            let max_pno = ocr_pages
                .iter()
                .enumerate()
                .map(|(i, p)| if p.page_number > 0 { p.page_number } else { i + 1 })
                .max()
                .unwrap_or(1);
            let p_vec = vec![
                PdfPageData {
                    width: 595.0,
                    height: 842.0,
                    blocks: Vec::new(),
                    image_refs: Vec::new(),
                };
                max_pno
            ];
            (p_vec, None, None, None)
        };

    let mut injected_page_nums: HashSet<usize> = HashSet::new();
    for (idx, ocr_page) in ocr_pages.iter().enumerate() {
        let pno = if ocr_page.page_number > 0 {
            ocr_page.page_number
        } else {
            idx + 1
        };
        let converted = ocr_page_to_pdf_page_data(ocr_page);
        if pno >= 1 && pno <= pages.len() {
            pages[pno - 1].width = converted.width;
            pages[pno - 1].height = converted.height;
            pages[pno - 1].blocks = converted.blocks;
            injected_page_nums.insert(pno);
        }
    }

    let mut builder = EpubBuilder::with_options(&options);
    builder.configure_for_pages(&pages, &options);
    let vol = builder.process_pdf_pages(&pages, file_id, existing_imgs, existing_pub, existing_desc);

    let mut remaining_scanned = detect_scanned_pages_from_pages(&pages, None);
    remaining_scanned.retain(|pno| !injected_page_nums.contains(pno));

    build_parsed_book_payload(vol, file_id, &builder, pages, remaining_scanned)
}

pub fn compress_brotli(bytes: &[u8]) -> Result<Vec<u8>, String> {
    use std::io::Write;
    let mut out = Vec::new();
    let mut writer = brotli::CompressorWriter::new(&mut out, 4096, 6, 22);
    writer.write_all(bytes).map_err(|e| e.to_string())?;
    drop(writer);
    Ok(out)
}

pub fn decompress_brotli(bytes: &[u8]) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let mut out = Vec::new();
    let mut reader = brotli::Decompressor::new(bytes, 4096);
    reader.read_to_end(&mut out).map_err(|e| e.to_string())?;
    Ok(out)
}
