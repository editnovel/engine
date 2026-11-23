use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Footnote {
    pub id: String,
    pub number: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Paragraph {
    pub text: String,
    pub is_scene_break: bool,
    pub is_part_heading: bool,
    pub style: String,
    pub footnotes: Vec<Footnote>,
}

impl Paragraph {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            is_scene_break: false,
            is_part_heading: false,
            style: "narrative".to_string(),
            footnotes: Vec::new(),
        }
    }

    pub fn with_style(text: impl Into<String>, style: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            is_scene_break: false,
            is_part_heading: false,
            style: style.into(),
            footnotes: Vec::new(),
        }
    }

    pub fn part_heading(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            is_scene_break: false,
            is_part_heading: true,
            style: "part_heading".to_string(),
            footnotes: Vec::new(),
        }
    }

    pub fn scene_break() -> Self {
        Self {
            text: "* * *".to_string(),
            is_scene_break: true,
            is_part_heading: false,
            style: "scene_break".to_string(),
            footnotes: Vec::new(),
        }
    }

    pub fn scene_break_with_text(text: impl Into<String>) -> Self {
        let t = text.into();
        let sym = if t.trim().is_empty() {
            "* * *".to_string()
        } else {
            t.trim().to_string()
        };
        Self {
            text: sym,
            is_scene_break: true,
            is_part_heading: false,
            style: "scene_break".to_string(),
            footnotes: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtractedImage {
    pub page_num: usize,
    pub image_index: usize,
    pub ext: String,
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub filename: String,
    pub category: String,
    pub y_pos: f64,
    pub is_full_page: bool,
    pub is_cover: bool,
    pub is_kept: bool,
    pub reason: String,
}

impl ExtractedImage {
    pub fn media_type(&self) -> &'static str {
        crate::epub_builder::image_mime_type(&self.ext)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ContentItemPayload {
    Paragraph(Paragraph),
    Image(ExtractedImage),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContentItem {
    pub item_type: String,
    pub payload: ContentItemPayload,
    pub page_num: usize,
    pub anchor: String,
}

impl ContentItem {
    pub fn text(text: String, style: String, footnotes: Vec<Footnote>, page_num: usize) -> Self {
        Self {
            item_type: "text".to_string(),
            payload: ContentItemPayload::Paragraph(Paragraph {
                text,
                is_scene_break: false,
                is_part_heading: false,
                style,
                footnotes,
            }),
            page_num,
            anchor: String::new(),
        }
    }

    pub fn scene_break(page_num: usize) -> Self {
        Self::scene_break_with_text("◆ ◇ ◆ ◇ ◆", page_num)
    }

    pub fn scene_break_with_text(text: impl Into<String>, page_num: usize) -> Self {
        let t = text.into();
        let sym = if t.trim().is_empty() {
            "◆ ◇ ◆ ◇ ◆".to_string()
        } else {
            t.trim().to_string()
        };
        Self {
            item_type: "scene_break".to_string(),
            payload: ContentItemPayload::Paragraph(Paragraph {
                text: sym,
                is_scene_break: true,
                is_part_heading: false,
                style: "scene_break".to_string(),
                footnotes: Vec::new(),
            }),
            page_num,
            anchor: String::new(),
        }
    }

    pub fn image(item_type: &str, img: ExtractedImage, page_num: usize) -> Self {
        Self {
            item_type: item_type.to_string(),
            payload: ContentItemPayload::Image(img),
            page_num,
            anchor: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Section {
    pub title: String,
    pub subtitle: Option<String>,
    pub start_page: usize,
    pub end_page: usize,
    pub start_y: f64,
    pub end_y: Option<f64>,
    pub paragraphs: Vec<Paragraph>,
    pub footnotes: Vec<Footnote>,
    pub content_items: Vec<ContentItem>,
    pub subsections: Vec<(String, String)>,
    pub level: usize,
    #[serde(default)]
    pub custom_html: Option<String>,
    #[serde(default)]
    pub section_type: Option<String>,
}

impl Section {
    pub fn new(
        title: String,
        subtitle: Option<String>,
        start_page: usize,
        end_page: usize,
    ) -> Self {
        Self {
            title,
            subtitle,
            start_page,
            end_page,
            start_y: 0.0,
            end_y: None,
            paragraphs: Vec::new(),
            footnotes: Vec::new(),
            content_items: Vec::new(),
            subsections: Vec::new(),
            level: 1,
            custom_html: None,
            section_type: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VolumeInfo {
    pub pdf_path: String,
    pub volume_number: usize,
    pub title: String,
    pub total_pages: usize,
    pub cover_image: Option<ExtractedImage>,
    pub color_images: Vec<ExtractedImage>,
    pub sections: Vec<Section>,
    pub all_verdicts: Vec<ExtractedImage>,
    pub spell_transformations: Vec<SpellTransformation>,
    #[serde(default)]
    pub publisher: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpellTransformation {
    pub index: usize,
    pub volume_num: usize,
    pub page_num: usize,
    pub section_title: String,
    pub category: String,
    pub confidence: f64,
    pub original_token: String,
    pub replacement_token: String,
    pub paragraph_before: String,
    pub paragraph_after: String,
    pub context_before: String,
    pub context_after: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendSpellCorrection {
    pub id: String,
    pub file_id: String,
    pub volume_num: usize,
    pub page_num: usize,
    pub section_title: String,
    pub category: String,
    pub confidence: f64,
    pub is_smell: bool,
    pub original_token: String,
    pub replacement_token: String,
    #[serde(default)]
    pub suggestions: Vec<String>,
    #[serde(default)]
    pub paragraph_id: Option<String>,
    pub paragraph_before: String,
    pub paragraph_after: String,
    pub context_snippet: String,
    pub language: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PdfSpan {
    pub text: String,
    pub size: f64,
    pub flags: u32,
    pub font: String,
    pub origin: (f64, f64),
    pub bbox: (f64, f64, f64, f64),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PdfLine {
    pub bbox: (f64, f64, f64, f64),
    pub spans: Vec<PdfSpan>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PdfBlock {
    pub block_type: u32,
    pub bbox: (f64, f64, f64, f64),
    pub lines: Vec<PdfLine>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PdfRawImageRef {
    pub xref: u32,
    pub rects: Vec<(f64, f64, f64, f64)>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PdfExtractedXrefImage {
    pub width: u32,
    pub height: u32,
    pub ext: String,
    pub data: Vec<u8>,
    pub raw_md5: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PdfPageData {
    pub width: f64,
    pub height: f64,
    pub blocks: Vec<PdfBlock>,
    pub image_refs: Vec<PdfRawImageRef>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrLineObject {
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub bbox: Option<(f64, f64, f64, f64)>,
    #[serde(default)]
    pub confidence: Option<f64>,
    #[serde(default, alias = "font_size", alias = "fontSize")]
    pub size: Option<f64>,
    #[serde(default, alias = "is_bold")]
    pub is_bold: Option<bool>,
    #[serde(default)]
    pub spans: Option<Vec<PdfSpan>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OcrLineEntry {
    Plain(String),
    Detailed(OcrLineObject),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrBlockInput {
    #[serde(default, alias = "block_type")]
    pub block_type: u32,
    #[serde(default)]
    pub bbox: Option<(f64, f64, f64, f64)>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub lines: Vec<OcrLineEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrPageInput {
    #[serde(
        default,
        alias = "page_number",
        alias = "page_num",
        alias = "pageNum",
        alias = "page"
    )]
    pub page_number: usize,
    #[serde(default)]
    pub width: Option<f64>,
    #[serde(default)]
    pub height: Option<f64>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub lines: Vec<OcrLineEntry>,
    #[serde(default)]
    pub blocks: Vec<OcrBlockInput>,
}

fn is_probable_heading_line(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 75 {
        return false;
    }
    let low = trimmed.to_lowercase();
    low.starts_with("capítulo ")
        || low.starts_with("capitulo ")
        || low.starts_with("chapter ")
        || low.starts_with("prólogo")
        || low.starts_with("prologo")
        || low.starts_with("prologue")
        || low.starts_with("epílogo")
        || low.starts_with("epilogo")
        || low.starts_with("epilogue")
        || low.starts_with("interludio")
        || low.starts_with("interlude")
        || low.starts_with("posfacio")
        || low.starts_with("afterword")
        || low.starts_with("palabras del autor")
}

fn make_ocr_pdf_line(
    text: &str,
    bbox_opt: Option<(f64, f64, f64, f64)>,
    size_opt: Option<f64>,
    bold_opt: Option<bool>,
    spans_opt: Option<Vec<PdfSpan>>,
    cursor_y: &mut f64,
    page_height: f64,
) -> Option<PdfLine> {
    let trimmed = text.trim();
    if trimmed.is_empty() && spans_opt.as_ref().is_none_or(|s| s.is_empty()) {
        *cursor_y += 14.0;
        return None;
    }
    let is_heading = bold_opt.unwrap_or_else(|| is_probable_heading_line(trimmed));
    let font_size = size_opt.unwrap_or(if is_heading { 16.0 } else { 11.0 });
    let flags = if is_heading { 16 } else { 0 };
    let bbox = match bbox_opt {
        Some((x0, y0, x1, y1)) => {
            let safe_y0 = y0.clamp(65.0, (page_height - 65.0).max(70.0));
            let safe_y1 = y1.max(safe_y0 + font_size);
            *cursor_y = safe_y1 + 5.0;
            (x0.max(50.0), safe_y0, x1.max(x0 + 40.0), safe_y1)
        }
        None => {
            let y0 = (*cursor_y).clamp(65.0, (page_height - 65.0).max(70.0));
            let y1 = y0 + font_size;
            *cursor_y = y1 + if is_heading { 16.0 } else { 5.0 };
            (72.0, y0, 520.0, y1)
        }
    };
    let spans = match spans_opt {
        Some(s) if !s.is_empty() => s,
        _ => vec![PdfSpan {
            text: trimmed.to_string(),
            size: font_size,
            flags,
            font: if is_heading {
                "OCR-Bold".to_string()
            } else {
                "OCR-Regular".to_string()
            },
            origin: (bbox.0, bbox.3 - 2.0),
            bbox,
        }],
    };
    Some(PdfLine { bbox, spans })
}

pub fn ocr_page_to_pdf_page_data(input: &OcrPageInput) -> PdfPageData {
    let width = input.width.unwrap_or(595.0).max(200.0);
    let height = input.height.unwrap_or(842.0).max(300.0);
    let mut blocks: Vec<PdfBlock> = Vec::new();
    let mut cursor_y = 75.0;

    for blk in &input.blocks {
        let mut pdf_lines: Vec<PdfLine> = Vec::new();
        for entry in &blk.lines {
            let line_opt = match entry {
                OcrLineEntry::Plain(s) => {
                    make_ocr_pdf_line(s, None, None, None, None, &mut cursor_y, height)
                }
                OcrLineEntry::Detailed(obj) => make_ocr_pdf_line(
                    &obj.text,
                    obj.bbox,
                    obj.size,
                    obj.is_bold,
                    obj.spans.clone(),
                    &mut cursor_y,
                    height,
                ),
            };
            if let Some(l) = line_opt {
                pdf_lines.push(l);
            }
        }
        if pdf_lines.is_empty() {
            if let Some(ref raw_txt) = blk.text {
                for raw_line in raw_txt.lines() {
                    if let Some(l) =
                        make_ocr_pdf_line(raw_line, None, None, None, None, &mut cursor_y, height)
                    {
                        pdf_lines.push(l);
                    }
                }
            }
        }
        if !pdf_lines.is_empty() {
            let bbox = blk.bbox.unwrap_or_else(|| {
                let mut x0 = f64::MAX;
                let mut y0 = f64::MAX;
                let mut x1 = f64::MIN;
                let mut y1 = f64::MIN;
                for l in &pdf_lines {
                    x0 = x0.min(l.bbox.0);
                    y0 = y0.min(l.bbox.1);
                    x1 = x1.max(l.bbox.2);
                    y1 = y1.max(l.bbox.3);
                }
                (x0, y0, x1, y1)
            });
            blocks.push(PdfBlock {
                block_type: blk.block_type,
                bbox,
                lines: pdf_lines,
            });
            cursor_y += 12.0;
        }
    }

    if blocks.is_empty() && !input.lines.is_empty() {
        let mut current_lines: Vec<PdfLine> = Vec::new();
        for entry in &input.lines {
            let is_blank = match entry {
                OcrLineEntry::Plain(s) => s.trim().is_empty(),
                OcrLineEntry::Detailed(o) => {
                    o.text.trim().is_empty() && o.spans.as_ref().is_none_or(|sp| sp.is_empty())
                }
            };
            if is_blank {
                if !current_lines.is_empty() {
                    let x0 = current_lines.iter().map(|l| l.bbox.0).fold(f64::MAX, f64::min);
                    let y0 = current_lines.iter().map(|l| l.bbox.1).fold(f64::MAX, f64::min);
                    let x1 = current_lines.iter().map(|l| l.bbox.2).fold(f64::MIN, f64::max);
                    let y1 = current_lines.iter().map(|l| l.bbox.3).fold(f64::MIN, f64::max);
                    blocks.push(PdfBlock {
                        block_type: 0,
                        bbox: (x0, y0, x1, y1),
                        lines: std::mem::take(&mut current_lines),
                    });
                }
                cursor_y += 14.0;
                continue;
            }
            let line_opt = match entry {
                OcrLineEntry::Plain(s) => {
                    make_ocr_pdf_line(s, None, None, None, None, &mut cursor_y, height)
                }
                OcrLineEntry::Detailed(obj) => make_ocr_pdf_line(
                    &obj.text,
                    obj.bbox,
                    obj.size,
                    obj.is_bold,
                    obj.spans.clone(),
                    &mut cursor_y,
                    height,
                ),
            };
            if let Some(l) = line_opt {
                let is_heading = (l.spans.first().map(|s| s.flags).unwrap_or(0) & 16) != 0;
                if is_heading && !current_lines.is_empty() {
                    let x0 = current_lines.iter().map(|ln| ln.bbox.0).fold(f64::MAX, f64::min);
                    let y0 = current_lines.iter().map(|ln| ln.bbox.1).fold(f64::MAX, f64::min);
                    let x1 = current_lines.iter().map(|ln| ln.bbox.2).fold(f64::MIN, f64::max);
                    let y1 = current_lines.iter().map(|ln| ln.bbox.3).fold(f64::MIN, f64::max);
                    blocks.push(PdfBlock {
                        block_type: 0,
                        bbox: (x0, y0, x1, y1),
                        lines: std::mem::take(&mut current_lines),
                    });
                }
                current_lines.push(l);
                if is_heading {
                    let x0 = current_lines.iter().map(|ln| ln.bbox.0).fold(f64::MAX, f64::min);
                    let y0 = current_lines.iter().map(|ln| ln.bbox.1).fold(f64::MAX, f64::min);
                    let x1 = current_lines.iter().map(|ln| ln.bbox.2).fold(f64::MIN, f64::max);
                    let y1 = current_lines.iter().map(|ln| ln.bbox.3).fold(f64::MIN, f64::max);
                    blocks.push(PdfBlock {
                        block_type: 0,
                        bbox: (x0, y0, x1, y1),
                        lines: std::mem::take(&mut current_lines),
                    });
                    cursor_y += 12.0;
                }
            }
        }
        if !current_lines.is_empty() {
            let x0 = current_lines.iter().map(|l| l.bbox.0).fold(f64::MAX, f64::min);
            let y0 = current_lines.iter().map(|l| l.bbox.1).fold(f64::MAX, f64::min);
            let x1 = current_lines.iter().map(|l| l.bbox.2).fold(f64::MIN, f64::max);
            let y1 = current_lines.iter().map(|l| l.bbox.3).fold(f64::MIN, f64::max);
            blocks.push(PdfBlock {
                block_type: 0,
                bbox: (x0, y0, x1, y1),
                lines: current_lines,
            });
        }
    }

    if blocks.is_empty() {
        if let Some(ref raw_text) = input.text {
            for para_chunk in raw_text.split("\n\n") {
                let mut pdf_lines = Vec::new();
                for line_str in para_chunk.lines() {
                    if let Some(l) =
                        make_ocr_pdf_line(line_str, None, None, None, None, &mut cursor_y, height)
                    {
                        pdf_lines.push(l);
                    }
                }
                if !pdf_lines.is_empty() {
                    let x0 = pdf_lines.iter().map(|l| l.bbox.0).fold(f64::MAX, f64::min);
                    let y0 = pdf_lines.iter().map(|l| l.bbox.1).fold(f64::MAX, f64::min);
                    let x1 = pdf_lines.iter().map(|l| l.bbox.2).fold(f64::MIN, f64::max);
                    let y1 = pdf_lines.iter().map(|l| l.bbox.3).fold(f64::MIN, f64::max);
                    blocks.push(PdfBlock {
                        block_type: 0,
                        bbox: (x0, y0, x1, y1),
                        lines: pdf_lines,
                    });
                    cursor_y += 14.0;
                }
            }
        }
    }

    PdfPageData {
        width,
        height,
        blocks,
        image_refs: Vec::new(),
    }
}
