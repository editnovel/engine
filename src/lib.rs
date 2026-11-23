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

use crate::config::{infer_series_and_volume, LANGUAGE};
use crate::epub_builder::EpubBuilder;
use crate::models::{ContentItem, ContentItemPayload, ExtractedImage, Paragraph, Section, VolumeInfo};
use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::io::{Cursor, Read};
use zip::ZipArchive;

struct StoredVolume {
    volume: VolumeInfo,
    section_ids: Vec<String>,
}

thread_local! {
    static LOADED_VOLUMES: RefCell<HashMap<String, StoredVolume>> = RefCell::new(HashMap::new());
    static CUSTOM_COVER: RefCell<Option<ExtractedImage>> = RefCell::new(None);
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

fn parse_epub_bytes(epub_bytes: &[u8], filename: &str) -> Result<VolumeInfo, String> {
    let reader = Cursor::new(epub_bytes);
    let mut archive = ZipArchive::new(reader).map_err(|e| e.to_string())?;

    let mut opf_content = String::new();
    for i in 0..archive.len() {
        let mut f = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = f.name().map_err(|e| e.to_string())?.to_string();
        if name.ends_with(".opf") {
            f.read_to_string(&mut opf_content).map_err(|e| e.to_string())?;
            break;
        }
    }

    let title_re = Regex::new(r"(?s)<dc:title[^>]*>(.*?)</dc:title>").unwrap();
    let title = title_re
        .captures(&opf_content)
        .map(|c| c[1].trim().to_string())
        .unwrap_or_else(|| filename.replace(".epub", ""));

    let vol_re = Regex::new(r"(?i)Volumen\s+(\d+)").unwrap();
    let vol_num = vol_re
        .captures(&title)
        .or_else(|| vol_re.captures(filename))
        .and_then(|c| c[1].parse::<usize>().ok())
        .unwrap_or(1);

    let pub_re = Regex::new(r"(?s)<dc:publisher[^>]*>(.*?)</dc:publisher>").unwrap();
    let extracted_publisher = pub_re
        .captures(&opf_content)
        .map(|c| c[1].trim().to_string())
        .filter(|p| !p.is_empty() && p != "Agnostic EPUB Converter");

    let desc_re = Regex::new(r"(?s)<dc:description[^>]*>(.*?)</dc:description>").unwrap();
    let mut extracted_description = desc_re
        .captures(&opf_content)
        .map(|c| {
            let tag_strip_re = Regex::new(r"<[^>]+>").unwrap();
            tag_strip_re.replace_all(&c[1], "").trim().to_string()
        })
        .filter(|d| !d.is_empty());

    let mut cover_image: Option<ExtractedImage> = None;
    let mut color_images: Vec<ExtractedImage> = Vec::new();
    let mut all_verdicts: Vec<ExtractedImage> = Vec::new();

    for i in 0..archive.len() {
        let mut f = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = f.name().map_err(|e| e.to_string())?.to_string();
        let lower = name.to_lowercase();
        if lower.ends_with(".jpg")
            || lower.ends_with(".jpeg")
            || lower.ends_with(".png")
            || lower.ends_with(".webp")
        {
            let mut data = Vec::new();
            f.read_to_end(&mut data).map_err(|e| e.to_string())?;
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
            let is_cov = cover_image.is_none() && (short_name.contains("p001") || short_name.contains("cover") || all_verdicts.is_empty());
            let cat = if is_cov {
                "covers"
            } else if short_name.contains("p002") || short_name.contains("p003") || short_name.contains("p004") || short_name.contains("color") {
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
        }
    }

    let mut xhtml_entries: Vec<(String, String)> = Vec::new();
    for i in 0..archive.len() {
        let mut f = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = f.name().map_err(|e| e.to_string())?.to_string();
        if (name.ends_with(".xhtml") || name.ends_with(".html"))
            && !name.ends_with("nav.xhtml")
            && !name.contains("cover")
            && !name.contains("color_")
            && !name.contains("colophon")
            && !name.contains("_title")
        {
            let mut text = String::new();
            if f.read_to_string(&mut text).is_ok() {
                xhtml_entries.push((name, text));
            }
        }
    }
    xhtml_entries.sort_by(|a, b| a.0.cmp(&b.0));

    let h1_re = Regex::new(r"(?s)<h1[^>]*>(.*?)</h1>").unwrap();
    let h2_re = Regex::new(r"(?s)<h2[^>]*>(.*?)</h2>").unwrap();
    let block_re = Regex::new(
        r#"(?is)<(?P<tag>p|div|hr)\b(?P<attrs>[^>]*)>(?:(?P<content>.*?)</(?:p|div)>)?"#,
    )
    .unwrap();
    let tag_re = Regex::new(r"<[^>]+>").unwrap();
    let bold_tag_re = Regex::new(r"(?is)<(?:strong|b)\b[^>]*>(.*?)</(?:strong|b)>").unwrap();
    let span_bold_re = Regex::new(
        r#"(?is)<span\b[^>]*style="[^"]*font-weight:\s*(?:bold|[6-9]00)[^"]*"[^>]*>(.*?)</span>"#,
    )
    .unwrap();
    let cleaner = crate::cleaner::TextCleaner::new();

    let mut sections: Vec<Section> = Vec::new();
    for (idx, (_, xhtml)) in xhtml_entries.into_iter().enumerate() {
        let h1 = h1_re
            .captures(&xhtml)
            .map(|c| tag_re.replace_all(&c[1], "").trim().to_string())
            .unwrap_or_else(|| format!("Capítulo {}", idx + 1));
        let h2 = h2_re
            .captures(&xhtml)
            .map(|c| tag_re.replace_all(&c[1], "").trim().to_string());
        let full_title = match h2 {
            Some(ref sub) if !sub.is_empty() => format!("{}: {}", h1, sub),
            _ => h1,
        };
        let mut sec = Section::new(full_title, h2, idx + 1, idx + 1);
        sec.section_type = Some(classify_section_type(&sec.title).to_string());
        for cap in block_re.captures_iter(&xhtml) {
            let tag = cap.name("tag").map(|m| m.as_str().to_lowercase()).unwrap_or_default();
            let attrs = cap.name("attrs").map(|m| m.as_str()).unwrap_or_default();
            let raw_content = cap.name("content").map(|m| m.as_str()).unwrap_or_default();

            let is_break_tag = tag == "hr" || attrs.contains("scene-break") || attrs.contains("scene_break");
            if tag == "div" && !is_break_tag {
                continue;
            }

            let with_bold = bold_tag_re.replace_all(raw_content, "\u{E000}$1\u{E001}");
            let with_bold = span_bold_re.replace_all(&with_bold, "\u{E000}$1\u{E001}");
            let inner = tag_re.replace_all(&with_bold, "").trim().to_string();
            let inner = crate::text_utils::merge_bold_tokens(&inner);

            if is_break_tag || cleaner.is_scene_break(&inner) || cleaner.is_subsection(&inner) {
                let sym = if inner.is_empty() { "* * *".to_string() } else { inner };
                sec.paragraphs.push(Paragraph::scene_break_with_text(&sym));
                sec.content_items.push(ContentItem::scene_break_with_text(&sym, idx + 1));
                continue;
            }

            if inner.is_empty() {
                continue;
            }
            let style = if attrs.contains("dialogue") {
                "dialogue"
            } else if attrs.contains("thought") {
                "thought"
            } else {
                "narrative"
            };
            sec.paragraphs
                .push(Paragraph::with_style(inner.clone(), style));
            sec.content_items.push(ContentItem::text(
                inner,
                style.to_string(),
                Vec::new(),
                idx + 1,
            ));
        }
        sections.push(sec);
    }

    if extracted_description.is_none() {
        extracted_description = crate::epub_builder::detect_synopsis_from_sections(&sections);
    }

    Ok(VolumeInfo {
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
    })
}

pub fn parse_book_file_json(bytes: &[u8], filename: &str) -> Result<String, String> {
    let mut builder = EpubBuilder::new();
    let is_epub = filename.to_lowercase().ends_with(".epub");

    let mut vol = if is_epub {
        parse_epub_bytes(bytes, filename)?
    } else {
        builder.process_pdf_bytes(bytes, filename, true)?
    };

    let detected_rules = compute_detected_rules(&vol);

    let mut frontend_sections: Vec<FrontendChapterSection> = Vec::with_capacity(vol.sections.len() + 2);
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
        let end_p = non_cover_colors.last().map(|i| i.page_num).unwrap_or(start_p);
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
        language: LANGUAGE.to_string(),
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
    let audit_opts = crate::spell_corrector::SpellAuditOptions::default();
    let mut audit_stats = crate::spell_corrector::SpellAuditStats::default();
    let mut all_spell_corrections = Vec::new();

    for sec in &mut vol.sections {
        let mut paras_clone = sec.paragraphs.clone();
        let (f_corrections, _) = spell_corrector.audit_section_paragraphs(
            filename,
            &mut paras_clone,
            vol.volume_number,
            sec.start_page,
            &sec.title,
            &audit_opts,
            &mut audit_stats,
            false,
            all_spell_corrections.len() + 1,
        );
        all_spell_corrections.extend(f_corrections);
    }

    LOADED_VOLUMES.with(|store| {
        store.borrow_mut().insert(
            filename.to_string(),
            StoredVolume {
                volume: vol,
                section_ids,
            },
        );
    });

    let payload = WasmParsedBookPayload {
        metadata,
        groups: vec![group],
        images: frontend_images,
        detected_rules,
        spell_corrections: all_spell_corrections,
        ignored_japanese_count: audit_stats.ignored_japanese_count,
        ignored_foreign_count: audit_stats.ignored_foreign_count,
        ignored_proper_nouns_count: audit_stats.ignored_proper_nouns_count,
    };

    serde_json::to_string(&payload).map_err(|e| e.to_string())
}

pub fn audit_spelling_json(file_id: &str, options_json: &str) -> Result<String, String> {
    let options: crate::spell_corrector::SpellAuditOptions =
        serde_json::from_str(options_json).unwrap_or_default();
    let spell_corrector = crate::spell_corrector::ContextualSpellCorrector::default();
    let mut stats = crate::spell_corrector::SpellAuditStats::default();
    let mut all_corrections = Vec::new();

    LOADED_VOLUMES.with(|store| {
        let map = store.borrow();
        if let Some(stored) = map.get(file_id) {
            for sec in &stored.volume.sections {
                let mut paras_clone = sec.paragraphs.clone();
                let (corrections, _) = spell_corrector.audit_section_paragraphs(
                    file_id,
                    &mut paras_clone,
                    stored.volume.volume_number,
                    sec.start_page,
                    &sec.title,
                    &options,
                    &mut stats,
                    false,
                    all_corrections.len() + 1,
                );
                all_corrections.extend(corrections);
            }
        }
    });

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
    CUSTOM_COVER.with(|cell| {
        *cell.borrow_mut() = Some(ExtractedImage {
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
    });
}

pub fn clear_custom_cover() {
    CUSTOM_COVER.with(|cell| {
        *cell.borrow_mut() = None;
    });
}

pub fn get_image_bytes(file_id: &str, image_name: &str) -> Option<Vec<u8>> {
    let custom = CUSTOM_COVER.with(|cell| {
        cell.borrow().as_ref().and_then(|img| {
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

    LOADED_VOLUMES.with(|store| {
        let map = store.borrow();
        let stored = map.get(file_id)?;
        stored
            .volume
            .all_verdicts
            .iter()
            .find(|img| img.filename == image_name)
            .map(|img| img.data.clone())
    })
}

pub fn remove_book_file(file_id: &str) {
    LOADED_VOLUMES.with(|store| {
        store.borrow_mut().remove(file_id);
    });
}

pub fn apply_transforms_internal(
    content: &str,
    editorial_rules: &[FrontendEditorialRule],
    regex_rules: &[FrontendRegexRule],
    glossary: &[FrontendGlossaryReplacement],
) -> String {
    let mut result = content.to_string();

    for rule in editorial_rules {
        if !rule.enabled {
            continue;
        }
        match rule.id.as_str() {
            "rule-dialogue-dashes" => {
                result = result.replace("--", "—");
                if let Ok(re) = Regex::new(r"(?m)^-\s+") {
                    result = re.replace_all(&result, "—").to_string();
                }
            }
            "rule-curly-quotes" => {
                if let Ok(re) = Regex::new(r#"([\s(\[>])"(\w)"#) {
                    result = re.replace_all(&result, "$1“$2").to_string();
                }
            }
            "rule-ellipsis" => {
                result = result.replace("...", "…");
            }
            "rule-clean-breaks" => {
                if let Ok(re) = Regex::new(r"(<p>\s*</p>\s*){2,}") {
                    result = re.replace_all(&result, "").to_string();
                }
            }
            _ => {}
        }
    }

    for reg in regex_rules {
        if !reg.enabled || reg.pattern.is_empty() {
            continue;
        }
        let case_insensitive = reg.flags.contains('i');
        let multi_line = reg.flags.contains('m');
        if let Ok(rx) = RegexBuilder::new(&reg.pattern)
            .case_insensitive(case_insensitive)
            .multi_line(multi_line)
            .build()
        {
            if reg.flags.contains('g') || reg.flags.is_empty() {
                result = rx.replace_all(&result, reg.replacement.as_str()).to_string();
            } else {
                result = rx.replace(&result, reg.replacement.as_str()).to_string();
            }
        }
    }

    for term in glossary {
        if !term.enabled || term.find.is_empty() {
            continue;
        }
        result = result.replace(&term.find, &term.replace);
    }

    result
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

    LOADED_VOLUMES.with(|store| {
        let map = store.borrow();
        for (g_idx, group) in req.groups.iter().enumerate() {
            let stored_opt = map.get(&group.file_id);
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
            } else {
                let custom_cov = CUSTOM_COVER.with(|cell| cell.borrow().clone());
                if let Some(cov) = custom_cov {
                    vol.cover_image = Some(cov.clone());
                    vol.all_verdicts.retain(|img| !img.is_cover);
                    vol.all_verdicts.push(cov);
                } else if let Some(ref cov) = vol.cover_image {
                    let cover_section_exists = group.sections.iter().any(|s| s.id.ends_with("-cover"));
                    if discarded_names.contains(&cov.filename) || !cover_section_exists {
                        vol.cover_image = None;
                    }
                }
            }
            let color_section_exists = group.sections.iter().any(|s| s.id.ends_with("-color-inserts"));
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
                        stored.volume.sections.get(orig_idx).cloned().unwrap_or_else(|| {
                            Section::new(f_sec.title.clone(), None, 1, 1)
                        })
                    } else {
                        Section::new(f_sec.title.clone(), None, 1, 1)
                    }
                } else {
                    Section::new(f_sec.title.clone(), None, 1, 1)
                };

                sec.title = f_sec.title.clone();
                sec.section_type = Some(f_sec.section_type.clone());
                if !f_sec.html_content.trim().is_empty() {
                    let mut custom = f_sec.html_content.clone();
                    for discarded in &discarded_names {
                        let pat = format!(r#"(?s)<div[^>]*class="[^"]*inline-image[^"]*"[^>]*>.*?<img[^>]*{}[^>]*>.*?</div>"#, regex::escape(discarded));
                        if let Ok(re) = Regex::new(&pat) {
                            custom = re.replace_all(&custom, "").to_string();
                        }
                        let pat_img = format!(r#"<img[^>]*{}[^>]*>"#, regex::escape(discarded));
                        if let Ok(re) = Regex::new(&pat_img) {
                            custom = re.replace_all(&custom, "").to_string();
                        }
                    }
                    sec.custom_html = Some(custom);
                }
                sec.content_items.retain(|item| match item.payload {
                    ContentItemPayload::Image(ref img) => {
                        !discarded_names.contains(&img.filename)
                    }
                    _ => true,
                });
                let is_volume_spell_enabled = req
                    .global_spell_enabled
                    .unwrap_or(true)
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
                            .filter(|c| c.file_id == group.file_id && c.section_title == sec.title && c.enabled)
                            .collect()
                    })
                    .unwrap_or_default();

                if let Some(ref mut custom) = sec.custom_html {
                    if is_volume_spell_enabled {
                        for corr in &active_corrections {
                            if custom.contains(&corr.original_token) {
                                *custom = custom.replace(&corr.original_token, &corr.replacement_token);
                            }
                        }
                    }
                    *custom = apply_transforms_internal(
                        custom,
                        &req.editorial_rules,
                        &req.regex_rules,
                        &req.glossary,
                    );
                }

                for item in &mut sec.content_items {
                    if let ContentItemPayload::Paragraph(ref mut p) = item.payload {
                        if is_volume_spell_enabled {
                            for corr in &active_corrections {
                                if p.text.contains(&corr.original_token) {
                                    p.text = p.text.replace(&corr.original_token, &corr.replacement_token);
                                }
                            }
                        }
                        p.text = apply_transforms_internal(
                            &p.text,
                            &req.editorial_rules,
                            &req.regex_rules,
                            &req.glossary,
                        );
                    }
                }
                reordered_sections.push(sec);
            }
            vol.sections = reordered_sections;
            active_volumes.push(vol);
        }
    });

    if active_volumes.is_empty() {
        return Err("No loaded volumes available for export".to_string());
    }

    if active_volumes.len() == 1 {
        builder.build_epub_bytes(&active_volumes[0])
    } else {
        builder.build_omnibus_bytes(&active_volumes)
    }
}
