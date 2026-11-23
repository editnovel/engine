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
