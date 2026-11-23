use crate::models::{
    PdfBlock, PdfExtractedXrefImage, PdfLine, PdfPageData, PdfRawImageRef, PdfSpan,
};
use image::{DynamicImage, GrayImage, ImageFormat, RgbImage, Rgba, RgbaImage};
use img_hash::{HashAlg, HasherConfig, ImageHash};
use lopdf::{Dictionary, Document, Object, ObjectId};
use pdf_oxide::content::{parse_content_stream, Matrix, Operator, TextElement};
use pdf_oxide::crypto::{CryptoProvider, HashAlgorithm, RustCryptoProvider};
use pdf_oxide::fonts::cmap::LazyCMap;
#[cfg(any(feature = "parallel", not(target_arch = "wasm32")))]
use rayon::prelude::*;
use std::collections::HashMap;
use std::io::Cursor;
use std::sync::{Arc, RwLock};

const WIN_ANSI_TABLE: [char; 256] = [
    '\u{0000}', '\u{0001}', '\u{0002}', '\u{0003}', '\u{0004}', '\u{0005}', '\u{0006}', '\u{0007}',
    '\u{0008}', '\u{0009}', '\u{000A}', '\u{000B}', '\u{000C}', '\u{000D}', '\u{000E}', '\u{000F}',
    '\u{0010}', '\u{0011}', '\u{0012}', '\u{0013}', '\u{0014}', '\u{0015}', '\u{0016}', '\u{0017}',
    '\u{0018}', '\u{0019}', '\u{001A}', '\u{001B}', '\u{001C}', '\u{001D}', '\u{001E}', '\u{001F}',
    ' ', '!', '"', '#', '$', '%', '&', '\'', '(', ')', '*', '+', ',', '-', '.', '/',
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', ':', ';', '<', '=', '>', '?',
    '@', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O',
    'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', '[', '\\', ']', '^', '_',
    '`', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o',
    'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z', '{', '|', '}', '~', '\u{007F}',
    '\u{20AC}', '\u{0081}', '\u{201A}', '\u{0192}', '\u{201E}', '\u{2026}', '\u{2020}', '\u{2021}',
    '\u{02C6}', '\u{2030}', '\u{0160}', '\u{2039}', '\u{0152}', '\u{008D}', '\u{017D}', '\u{008F}',
    '\u{0090}', '\u{2018}', '\u{2019}', '\u{201C}', '\u{201D}', '\u{2022}', '\u{2013}', '\u{2014}',
    '\u{02DC}', '\u{2122}', '\u{0161}', '\u{203A}', '\u{0153}', '\u{009D}', '\u{017E}', '\u{0178}',
    '\u{00A0}', '\u{00A1}', '\u{00A2}', '\u{00A3}', '\u{00A4}', '\u{00A5}', '\u{00A6}', '\u{00A7}',
    '\u{00A8}', '\u{00A9}', '\u{00AA}', '\u{00AB}', '\u{00AC}', '\u{00AD}', '\u{00AE}', '\u{00AF}',
    '\u{00B0}', '\u{00B1}', '\u{00B2}', '\u{00B3}', '\u{00B4}', '\u{00B5}', '\u{00B6}', '\u{00B7}',
    '\u{00B8}', '\u{00B9}', '\u{00BA}', '\u{00BB}', '\u{00BC}', '\u{00BD}', '\u{00BE}', '\u{00BF}',
    '\u{00C0}', '\u{00C1}', '\u{00C2}', '\u{00C3}', '\u{00C4}', '\u{00C5}', '\u{00C6}', '\u{00C7}',
    '\u{00C8}', '\u{00C9}', '\u{00CA}', '\u{00CB}', '\u{00CC}', '\u{00CD}', '\u{00CE}', '\u{00CF}',
    '\u{00D0}', '\u{00D1}', '\u{00D2}', '\u{00D3}', '\u{00D4}', '\u{00D5}', '\u{00D6}', '\u{00D7}',
    '\u{00D8}', '\u{00D9}', '\u{00DA}', '\u{00DB}', '\u{00DC}', '\u{00DD}', '\u{00DE}', '\u{00DF}',
    '\u{00E0}', '\u{00E1}', '\u{00E2}', '\u{00E3}', '\u{00E4}', '\u{00E5}', '\u{00E6}', '\u{00E7}',
    '\u{00E8}', '\u{00E9}', '\u{00EA}', '\u{00EB}', '\u{00EC}', '\u{00ED}', '\u{00EE}', '\u{00EF}',
    '\u{00F0}', '\u{00F1}', '\u{00F2}', '\u{00F3}', '\u{00F4}', '\u{00F5}', '\u{00F6}', '\u{00F7}',
    '\u{00F8}', '\u{00F9}', '\u{00FA}', '\u{00FB}', '\u{00FC}', '\u{00FD}', '\u{00FE}', '\u{00FF}',
];

#[derive(Debug, Clone)]
struct FontInfo {
    base_font: String,
    is_cid: bool,
    ascender: f64,
    descender: f64,
    style_flags: u32,
    default_width: f64,
    widths: HashMap<u16, f64>,
    to_unicode: Option<LazyCMap>,
}

impl Default for FontInfo {
    fn default() -> Self {
        Self {
            base_font: "Unknown".to_string(),
            is_cid: false,
            ascender: 0.905,
            descender: -0.210,
            style_flags: 0,
            default_width: 500.0,
            widths: HashMap::new(),
            to_unicode: None,
        }
    }
}

#[derive(Debug, Clone)]
struct GraphicsState {
    ctm: Matrix,
    font_key: String,
    font_size: f64,
    char_space: f64,
    word_space: f64,
    horiz_scale: f64,
    leading: f64,
    text_rise: f64,
}

impl Default for GraphicsState {
    fn default() -> Self {
        Self {
            ctm: Matrix::identity(),
            font_key: String::new(),
            font_size: 11.0,
            char_space: 0.0,
            word_space: 0.0,
            horiz_scale: 100.0,
            leading: 0.0,
            text_rise: 0.0,
        }
    }
}

pub struct ParsedPdfDocument {
    pub pages: Vec<PdfPageData>,
    pub raw_doc: Document,
}

impl ParsedPdfDocument {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        let doc = Document::load_mem(bytes).map_err(|e| format!("PDF load error: {}", e))?;
        let page_map = doc.get_pages();
        let page_ids: Vec<ObjectId> = page_map.values().copied().collect();
        let font_cache: RwLock<HashMap<ObjectId, Arc<FontInfo>>> = RwLock::new(HashMap::new());

        #[cfg(any(feature = "parallel", not(target_arch = "wasm32")))]
        let pages: Vec<PdfPageData> = page_ids
            .par_iter()
            .map(|&page_id| extract_single_page(&doc, page_id, &font_cache))
            .collect();

        #[cfg(all(not(feature = "parallel"), target_arch = "wasm32"))]
        let pages: Vec<PdfPageData> = page_ids
            .iter()
            .map(|&page_id| extract_single_page(&doc, page_id, &font_cache))
            .collect();

        Ok(Self {
            pages,
            raw_doc: doc,
        })
    }

    pub fn detect_scanned_pages(&self) -> Vec<usize> {
        detect_scanned_pages_from_pages(&self.pages, Some(&self.raw_doc))
    }

    pub fn extract_xref_image(&self, xref: u32) -> Option<PdfExtractedXrefImage> {
        extract_image_by_xref(&self.raw_doc, (xref, 0))
    }

    pub fn extract_image_by_xref(&self, xref: u32) -> Option<PdfExtractedXrefImage> {
        extract_image_by_xref(&self.raw_doc, (xref, 0))
    }

    pub fn extract_pdf_metadata(&self) -> (Option<String>, Option<String>) {
        let mut publisher = None;
        let mut description = None;

        if let Ok(info_obj) = self.raw_doc.trailer.get(b"Info") {
            if let Some(info_dict) = resolve_dict(&self.raw_doc, info_obj) {
                if let Ok(subj_obj) = info_dict.get(b"Subject") {
                    if let Some(s) = obj_to_string(subj_obj) {
                        let trimmed = s.trim();
                        if trimmed.len() >= 20 {
                            description = Some(trimmed.to_string());
                        }
                    }
                }
                if description.is_none() {
                    if let Ok(desc_obj) = info_dict.get(b"Description") {
                        if let Some(s) = obj_to_string(desc_obj) {
                            let trimmed = s.trim();
                            if trimmed.len() >= 20 {
                                description = Some(trimmed.to_string());
                            }
                        }
                    }
                }
                if let Ok(pub_obj) = info_dict.get(b"Publisher") {
                    if let Some(s) = obj_to_string(pub_obj) {
                        let trimmed = s.trim();
                        if !trimmed.is_empty() && trimmed != "Agnostic EPUB Converter" {
                            publisher = Some(trimmed.to_string());
                        }
                    }
                }
            }
        }

        if description.is_none() || publisher.is_none() {
            if let Ok(root_obj) = self.raw_doc.trailer.get(b"Root") {
                if let Some(root_dict) = resolve_dict(&self.raw_doc, root_obj) {
                    if let Ok(meta_obj) = root_dict.get(b"Metadata") {
                        let meta_stream_res = match meta_obj {
                            Object::Reference(id) => {
                                self.raw_doc.get_object(*id).ok().and_then(|o| o.as_stream().ok())
                            }
                            Object::Stream(s) => Some(s),
                            _ => None,
                        };
                        if let Some(stream) = meta_stream_res {
                            let decompressed = stream
                                .decompressed_content()
                                .unwrap_or_else(|_| stream.content.clone());
                            let xmp = String::from_utf8_lossy(&decompressed);
                            if description.is_none() {
                                if let Ok(re) = regex::Regex::new(
                                    r"(?s)<dc:description[^>]*>.*?<rdf:li[^>]*>(.*?)</rdf:li>",
                                ) {
                                    if let Some(cap) = re.captures(&xmp) {
                                        let clean = cap[1].trim();
                                        if clean.len() >= 20 {
                                            description = Some(clean.to_string());
                                        }
                                    }
                                }
                            }
                            if publisher.is_none() {
                                if let Ok(re) = regex::Regex::new(
                                    r"(?s)<dc:publisher[^>]*>.*?<rdf:li[^>]*>(.*?)</rdf:li>",
                                ) {
                                    if let Some(cap) = re.captures(&xmp) {
                                        let clean = cap[1].trim();
                                        if !clean.is_empty() && clean != "Agnostic EPUB Converter" {
                                            publisher = Some(clean.to_string());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        (publisher, description)
    }
}

fn obj_to_string(obj: &Object) -> Option<String> {
    match obj {
        Object::String(bytes, _) => {
            if bytes.starts_with(&[0xFE, 0xFF]) {
                let u16_chars: Vec<u16> = bytes[2..]
                    .chunks_exact(2)
                    .map(|c| u16::from_be_bytes([c[0], c[1]]))
                    .collect();
                Some(String::from_utf16_lossy(&u16_chars).trim().to_string())
            } else if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
                Some(String::from_utf8_lossy(&bytes[3..]).trim().to_string())
            } else {
                Some(String::from_utf8_lossy(bytes).trim().to_string())
            }
        }
        Object::Name(bytes) => Some(String::from_utf8_lossy(bytes).trim().to_string()),
        _ => None,
    }
}

fn obj_as_f64(obj: &Object) -> Option<f64> {
    match obj {
        Object::Real(v) => Some(*v as f64),
        Object::Integer(v) => Some(*v as f64),
        _ => None,
    }
}

fn resolve_dict<'a>(doc: &'a Document, obj: &'a Object) -> Option<&'a Dictionary> {
    match obj {
        Object::Dictionary(d) => Some(d),
        Object::Reference(id) => doc.get_object(*id).ok()?.as_dict().ok(),
        _ => None,
    }
}

fn resolve_array<'a>(doc: &'a Document, obj: &'a Object) -> Option<&'a Vec<Object>> {
    match obj {
        Object::Array(a) => Some(a),
        Object::Reference(id) => doc.get_object(*id).ok()?.as_array().ok(),
        _ => None,
    }
}

fn get_inherited_object<'a>(doc: &'a Document, page_id: ObjectId, key: &[u8]) -> Option<&'a Object> {
    let mut curr_id = Some(page_id);
    let mut visited = 0;
    while let Some(id) = curr_id {
        if visited > 16 {
            break;
        }
        visited += 1;
        let dict = doc.get_object(id).ok()?.as_dict().ok()?;
        if let Ok(val) = dict.get(key) {
            return Some(val);
        }
        if let Ok(Object::Reference(parent_id)) = dict.get(b"Parent") {
            curr_id = Some(*parent_id);
        } else {
            curr_id = None;
        }
    }
    None
}

fn get_page_dimensions(doc: &Document, page_id: ObjectId) -> (f64, f64) {
    let box_obj = get_inherited_object(doc, page_id, b"CropBox")
        .or_else(|| get_inherited_object(doc, page_id, b"MediaBox"));
    if let Some(obj) = box_obj {
        if let Some(arr) = resolve_array(doc, obj) {
            if arr.len() >= 4 {
                let x0 = obj_as_f64(&arr[0]).unwrap_or(0.0);
                let y0 = obj_as_f64(&arr[1]).unwrap_or(0.0);
                let x1 = obj_as_f64(&arr[2]).unwrap_or(595.0);
                let y1 = obj_as_f64(&arr[3]).unwrap_or(842.0);
                return ((x1 - x0).abs(), (y1 - y0).abs());
            }
        }
    }
    (595.0, 842.0)
}

fn strip_subset_prefix(name: &str) -> String {
    if let Some(idx) = name.find('+') {
        if idx == 6 && name[..6].chars().all(|c| c.is_ascii_uppercase()) {
            return name[7..].to_string();
        }
    }
    name.to_string()
}

fn parse_font_descriptor(doc: &Document, desc_dict: &Dictionary, info: &mut FontInfo) {
    let _ = doc;
    if let Ok(asc) = desc_dict.get(b"Ascent") {
        if let Some(v) = obj_as_f64(asc) {
            if v != 0.0 {
                info.ascender = v / 1000.0;
            }
        }
    }
    if let Ok(desc) = desc_dict.get(b"Descent") {
        if let Some(v) = obj_as_f64(desc) {
            if v != 0.0 {
                info.descender = v / 1000.0;
            }
        }
    }
    if let Ok(ia) = desc_dict.get(b"ItalicAngle") {
        if let Some(v) = obj_as_f64(ia) {
            if v.abs() > 0.1 {
                info.style_flags |= 2;
            }
        }
    }
    if let Ok(fn_obj) = desc_dict.get(b"FontName") {
        if let Ok(fn_bytes) = fn_obj.as_name() {
            let fn_str = String::from_utf8_lossy(fn_bytes).to_lowercase();
            if fn_str.contains("bold")
                || fn_str.contains("black")
                || fn_str.contains("heavy")
                || fn_str.contains("semibold")
                || fn_str.contains("demi")
            {
                info.style_flags |= 16;
            }
        }
    }
    if let Ok(fw) = desc_dict.get(b"FontWeight") {
        if let Some(v) = obj_as_f64(fw) {
            if v >= 600.0 {
                info.style_flags |= 16;
            }
        }
    }
}

fn parse_cid_widths(doc: &Document, w_arr: &[Object], widths: &mut HashMap<u16, f64>) {
    let mut i = 0;
    while i < w_arr.len() {
        let Some(first_cid) = obj_as_f64(&w_arr[i]).map(|v| v as u16) else {
            i += 1;
            continue;
        };
        if i + 1 >= w_arr.len() {
            break;
        }
        if let Some(sub_arr) = resolve_array(doc, &w_arr[i + 1]) {
            for (offset, w_obj) in sub_arr.iter().enumerate() {
                if let Some(w_val) = obj_as_f64(w_obj) {
                    widths.insert(first_cid.saturating_add(offset as u16), w_val);
                }
            }
            i += 2;
        } else if i + 2 < w_arr.len() {
            if let (Some(last_cid), Some(w_val)) = (
                obj_as_f64(&w_arr[i + 1]).map(|v| v as u16),
                obj_as_f64(&w_arr[i + 2]),
            ) {
                for cid in first_cid..=last_cid {
                    widths.insert(cid, w_val);
                }
            }
            i += 3;
        } else {
            break;
        }
    }
}

fn parse_font(doc: &Document, font_dict: &Dictionary) -> FontInfo {
    let mut info = FontInfo::default();

    if let Ok(bf) = font_dict.get(b"BaseFont") {
        if let Ok(name_bytes) = bf.as_name() {
            let raw_name = String::from_utf8_lossy(name_bytes).to_string();
            info.base_font = strip_subset_prefix(&raw_name);
        }
    }

    let low_name = info.base_font.to_lowercase();
    if low_name.contains("italic") || low_name.contains("oblique") {
        info.style_flags |= 2;
    }
    if low_name.contains("bold")
        || low_name.contains("black")
        || low_name.contains("heavy")
        || low_name.contains("semibold")
        || low_name.contains("demi")
    {
        info.style_flags |= 16;
    }

    let subtype = font_dict
        .get(b"Subtype")
        .ok()
        .and_then(|o| o.as_name().ok())
        .unwrap_or(b"TrueType");

    if subtype == b"Type0" {
        info.is_cid = true;
        info.default_width = 1000.0;

        if let Ok(desc_fonts_obj) = font_dict.get(b"DescendantFonts") {
            if let Some(arr) = resolve_array(doc, desc_fonts_obj) {
                if let Some(first_desc) = arr.first() {
                    if let Some(cid_dict) = resolve_dict(doc, first_desc) {
                        if let Ok(dw_obj) = cid_dict.get(b"DW") {
                            if let Some(dw) = obj_as_f64(dw_obj) {
                                info.default_width = dw;
                            }
                        }
                        if let Ok(fd_obj) = cid_dict.get(b"FontDescriptor") {
                            if let Some(fd_dict) = resolve_dict(doc, fd_obj) {
                                parse_font_descriptor(doc, fd_dict, &mut info);
                            }
                        }
                        if let Ok(w_obj) = cid_dict.get(b"W") {
                            if let Some(w_arr) = resolve_array(doc, w_obj) {
                                parse_cid_widths(doc, w_arr, &mut info.widths);
                            }
                        }
                    }
                }
            }
        }
    } else {
        let first_char = font_dict
            .get(b"FirstChar")
            .ok()
            .and_then(obj_as_f64)
            .unwrap_or(32.0) as u16;
        if let Ok(w_obj) = font_dict.get(b"Widths") {
            if let Some(w_arr) = resolve_array(doc, w_obj) {
                for (idx, val) in w_arr.iter().enumerate() {
                    if let Some(w) = obj_as_f64(val) {
                        info.widths.insert(first_char + idx as u16, w);
                    }
                }
            }
        }
        if let Ok(fd_obj) = font_dict.get(b"FontDescriptor") {
            if let Some(fd_dict) = resolve_dict(doc, fd_obj) {
                parse_font_descriptor(doc, fd_dict, &mut info);
            }
        }
    }

    if let Ok(tu_obj) = font_dict.get(b"ToUnicode") {
        let stream_res = match tu_obj {
            Object::Reference(id) => doc.get_object(*id).ok().and_then(|o| o.as_stream().ok()),
            Object::Stream(s) => Some(s),
            _ => None,
        };
        if let Some(stream) = stream_res {
            let cmap_bytes = stream
                .decompressed_content()
                .unwrap_or_else(|_| stream.content.clone());
            info.to_unicode = Some(LazyCMap::new(cmap_bytes));
        }
    }

    info
}

fn decode_pdf_bytes_and_advance(
    bytes: &[u8],
    font: &FontInfo,
    gs: &GraphicsState,
) -> (String, f64) {
    let mut text = String::new();
    let mut advance_x = 0.0;
    let scale = gs.horiz_scale / 100.0;
    let cmap = font.to_unicode.as_ref().and_then(|lc| lc.get());

    if font.is_cid {
        let mut i = 0;
        while i + 1 < bytes.len() {
            let cid = u16::from_be_bytes([bytes[i], bytes[i + 1]]);
            let decoded = if let Some(s) = cmap.as_ref().and_then(|c| c.get(&(cid as u32))) {
                s.into_owned()
            } else if let Some(ch) = char::from_u32(cid as u32) {
                ch.to_string()
            } else {
                String::new()
            };
            let is_space = decoded == " ";
            text.push_str(&decoded);
            let w0 = font.widths.get(&cid).copied().unwrap_or(font.default_width);
            let tx = ((w0 / 1000.0) * gs.font_size
                + gs.char_space
                + if is_space { gs.word_space } else { 0.0 })
                * scale;
            advance_x += tx;
            i += 2;
        }
    } else {
        for &b in bytes {
            let code = b as u16;
            let decoded = if let Some(s) = cmap.as_ref().and_then(|c| c.get(&(code as u32))) {
                s.into_owned()
            } else {
                WIN_ANSI_TABLE[b as usize].to_string()
            };
            let is_space = b == b' ' || decoded == " ";
            text.push_str(&decoded);
            let w0 = font.widths.get(&code).copied().unwrap_or(font.default_width);
            let tx = ((w0 / 1000.0) * gs.font_size
                + gs.char_space
                + if is_space { gs.word_space } else { 0.0 })
                * scale;
            advance_x += tx;
        }
    }

    (text, advance_x)
}

#[derive(Debug, Clone)]
enum StreamEvent {
    SpanRun {
        text: String,
        size: f64,
        style_flags: u32,
        font_name: String,
        origin_x: f64,
        origin_y: f64,
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
    },
    PathBarrier,
}

fn extract_single_page(
    doc: &Document,
    page_id: ObjectId,
    font_cache: &RwLock<HashMap<ObjectId, Arc<FontInfo>>>,
) -> PdfPageData {
    let (page_width, page_height) = get_page_dimensions(doc, page_id);

    let mut page_fonts: HashMap<String, Arc<FontInfo>> = HashMap::new();
    let mut xobject_order: Vec<(String, u32)> = Vec::new();
    let mut xobject_rects: HashMap<u32, Vec<(f64, f64, f64, f64)>> = HashMap::new();

    if let Some(res_obj) = get_inherited_object(doc, page_id, b"Resources") {
        if let Some(res_dict) = resolve_dict(doc, res_obj) {
            if let Ok(font_obj) = res_dict.get(b"Font") {
                if let Some(font_map_dict) = resolve_dict(doc, font_obj) {
                    for (k, v) in font_map_dict.iter() {
                        let key_str = String::from_utf8_lossy(k).to_string();
                        match v {
                            Object::Reference(fid) => {
                                let cached = font_cache
                                    .read()
                                    .ok()
                                    .and_then(|guard| guard.get(fid).cloned());
                                let fi = if let Some(existing) = cached {
                                    existing
                                } else {
                                    let parsed = Arc::new(
                                        doc.get_object(*fid)
                                            .ok()
                                            .and_then(|o| o.as_dict().ok())
                                            .map(|d| parse_font(doc, d))
                                            .unwrap_or_default(),
                                    );
                                    if let Ok(mut guard) = font_cache.write() {
                                        guard.entry(*fid).or_insert_with(|| parsed.clone()).clone()
                                    } else {
                                        parsed
                                    }
                                };
                                page_fonts.insert(key_str, fi);
                            }
                            Object::Dictionary(fd) => {
                                page_fonts.insert(key_str, Arc::new(parse_font(doc, fd)));
                            }
                            _ => {}
                        }
                    }
                }
            }

            if let Ok(xobj_obj) = res_dict.get(b"XObject") {
                if let Some(xobj_dict) = resolve_dict(doc, xobj_obj) {
                    for (k, v) in xobj_dict.iter() {
                        if let Object::Reference(xid) = v {
                            if let Ok(target) = doc.get_object(*xid) {
                                if let Ok(stream) = target.as_stream() {
                                    let is_img = stream
                                        .dict
                                        .get(b"Subtype")
                                        .ok()
                                        .and_then(|o| o.as_name().ok())
                                        == Some(b"Image");
                                    if is_img {
                                        xobject_order.push((
                                            String::from_utf8_lossy(k).to_string(),
                                            xid.0,
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let mut events: Vec<StreamEvent> = Vec::new();
    let content_bytes = doc.get_page_content(page_id);
    if !content_bytes.is_empty() {
        if let Ok(operators) = parse_content_stream(&content_bytes) {
            let mut gs_stack: Vec<GraphicsState> = Vec::new();
            let mut gs = GraphicsState::default();
            let mut tm = Matrix::identity();
            let mut tlm = Matrix::identity();
            let default_font = FontInfo::default();

            for op in &operators {
                match op {
                    Operator::SaveState => {
                        gs_stack.push(gs.clone());
                    }
                    Operator::RestoreState => {
                        if let Some(prev) = gs_stack.pop() {
                            gs = prev;
                        }
                    }
                    Operator::Cm { a, b, c, d, e, f } => {
                        let m = Matrix {
                            a: *a,
                            b: *b,
                            c: *c,
                            d: *d,
                            e: *e,
                            f: *f,
                        };
                        gs.ctm = m.multiply(&gs.ctm);
                    }
                    Operator::Do { name } => {
                        if let Some((_, xref)) = xobject_order.iter().find(|(k, _)| k == name) {
                            let x_a = gs.ctm.e as f64;
                            let x_b = (gs.ctm.e + gs.ctm.a) as f64;
                            let y_a = gs.ctm.f as f64;
                            let y_b = (gs.ctm.f + gs.ctm.d) as f64;
                            let x0 = x_a.min(x_b);
                            let x1 = x_a.max(x_b);
                            let y_bot = y_a.min(y_b);
                            let y_top = y_a.max(y_b);
                            let rect = (
                                x0 as f32 as f64,
                                (page_height - y_top) as f32 as f64,
                                x1 as f32 as f64,
                                (page_height - y_bot) as f32 as f64,
                            );
                            xobject_rects.entry(*xref).or_default().push(rect);
                        }
                    }
                    Operator::Fill | Operator::FillEvenOdd => {
                        events.push(StreamEvent::PathBarrier);
                    }
                    Operator::BeginText => {
                        tm = Matrix::identity();
                        tlm = Matrix::identity();
                    }
                    Operator::Tf { font, size } => {
                        gs.font_key = font.clone();
                        gs.font_size = *size as f64;
                    }
                    Operator::Tc { char_space } => {
                        gs.char_space = *char_space as f64;
                    }
                    Operator::Tw { word_space } => {
                        gs.word_space = *word_space as f64;
                    }
                    Operator::Tz { scale } => {
                        gs.horiz_scale = *scale as f64;
                    }
                    Operator::TL { leading } => {
                        gs.leading = *leading as f64;
                    }
                    Operator::Ts { rise } => {
                        gs.text_rise = *rise as f64;
                    }
                    Operator::Tm { a, b, c, d, e, f } => {
                        tm = Matrix {
                            a: *a,
                            b: *b,
                            c: *c,
                            d: *d,
                            e: *e,
                            f: *f,
                        };
                        tlm = tm;
                    }
                    Operator::Td { tx, ty } => {
                        tlm = Matrix::translation(*tx, *ty).multiply(&tlm);
                        tm = tlm;
                    }
                    Operator::TD { tx, ty } => {
                        gs.leading = -(*ty as f64);
                        tlm = Matrix::translation(*tx, *ty).multiply(&tlm);
                        tm = tlm;
                    }
                    Operator::TStar => {
                        tlm = Matrix::translation(0.0, -(gs.leading as f32)).multiply(&tlm);
                        tm = tlm;
                    }
                    Operator::Tj { text } => {
                        let font = page_fonts.get(&gs.font_key).map(|f| f.as_ref()).unwrap_or(&default_font);
                        emit_text_run(
                            &[TextElement::String(text.clone())],
                            font,
                            &gs,
                            &mut tm,
                            page_height,
                            &mut events,
                        );
                    }
                    Operator::Quote { text } => {
                        tlm = Matrix::translation(0.0, -(gs.leading as f32)).multiply(&tlm);
                        tm = tlm;
                        let font = page_fonts.get(&gs.font_key).map(|f| f.as_ref()).unwrap_or(&default_font);
                        emit_text_run(
                            &[TextElement::String(text.clone())],
                            font,
                            &gs,
                            &mut tm,
                            page_height,
                            &mut events,
                        );
                    }
                    Operator::DoubleQuote {
                        word_space,
                        char_space,
                        text,
                    } => {
                        gs.word_space = *word_space as f64;
                        gs.char_space = *char_space as f64;
                        tlm = Matrix::translation(0.0, -(gs.leading as f32)).multiply(&tlm);
                        tm = tlm;
                        let font = page_fonts.get(&gs.font_key).map(|f| f.as_ref()).unwrap_or(&default_font);
                        emit_text_run(
                            &[TextElement::String(text.clone())],
                            font,
                            &gs,
                            &mut tm,
                            page_height,
                            &mut events,
                        );
                    }
                    Operator::TJ { array } => {
                        let font = page_fonts.get(&gs.font_key).map(|f| f.as_ref()).unwrap_or(&default_font);
                        emit_text_run(array, font, &gs, &mut tm, page_height, &mut events);
                    }
                    _ => {}
                }
            }
        }
    }

    let blocks = build_blocks_from_events(events);

    let mut image_refs = Vec::new();
    let mut seen_xrefs = std::collections::HashSet::new();
    for (_, xref) in xobject_order {
        if seen_xrefs.insert(xref) {
            let rects = xobject_rects.remove(&xref).unwrap_or_default();
            image_refs.push(PdfRawImageRef { xref, rects });
        }
    }

    PdfPageData {
        width: page_width,
        height: page_height,
        blocks,
        image_refs,
    }
}

fn emit_text_run(
    arr: &[TextElement],
    font: &FontInfo,
    gs: &GraphicsState,
    tm: &mut Matrix,
    page_height: f64,
    events: &mut Vec<StreamEvent>,
) {
    let trm = tm.multiply(&gs.ctm);
    let scale_y = ((trm.c * trm.c + trm.d * trm.d) as f64).sqrt();
    let eff_size = (gs.font_size * scale_y) as f32 as f64;
    let origin_x = trm.e as f64;
    let origin_y = (page_height - (trm.f as f64 + gs.text_rise)) as f32 as f64;

    let mut full_text = String::new();
    let mut total_adv = 0.0;
    let scale = gs.horiz_scale / 100.0;
    let mut pending_tj_space = false;

    for item in arr {
        match item {
            TextElement::String(bytes) => {
                let (t, adv) = decode_pdf_bytes_and_advance(bytes, font, gs);
                if !t.is_empty() {
                    if pending_tj_space
                        && !full_text.is_empty()
                        && !full_text.ends_with(char::is_whitespace)
                        && !t.starts_with(char::is_whitespace)
                    {
                        full_text.push(' ');
                    }
                    pending_tj_space = false;
                    full_text.push_str(&t);
                }
                total_adv += adv;
            }
            TextElement::Offset(n) => {
                if *n <= -280.0 {
                    pending_tj_space = true;
                }
                total_adv -= (*n as f64 / 1000.0) * gs.font_size * scale;
            }
        }
    }

    tm.e += total_adv as f32;

    if full_text.is_empty() {
        return;
    }

    let end_trm = tm.multiply(&gs.ctm);
    let x1 = (end_trm.e as f64).max(origin_x);
    let y0 = (origin_y - eff_size * font.ascender) as f32 as f64;
    let y1 = (origin_y - eff_size * font.descender) as f32 as f64;

    events.push(StreamEvent::SpanRun {
        text: full_text,
        size: eff_size,
        style_flags: font.style_flags,
        font_name: font.base_font.clone(),
        origin_x,
        origin_y,
        x0: origin_x,
        y0,
        x1,
        y1,
    });
}

struct ActiveLine {
    main_origin_y: f64,
    main_size: f64,
    has_regular_baseline: bool,
    bbox: (f64, f64, f64, f64),
    spans: Vec<PdfSpan>,
}

fn build_blocks_from_events(events: Vec<StreamEvent>) -> Vec<PdfBlock> {
    let mut blocks: Vec<PdfBlock> = Vec::new();
    let mut curr_block_lines: Vec<PdfLine> = Vec::new();
    let mut curr_line: Option<ActiveLine> = None;
    let mut had_barrier = false;

    for ev in events {
        match ev {
            StreamEvent::PathBarrier => {
                if curr_line.is_some() || !curr_block_lines.is_empty() {
                    had_barrier = true;
                }
            }
            StreamEvent::SpanRun {
                text,
                size,
                style_flags,
                font_name,
                origin_x,
                origin_y,
                x0,
                y0,
                x1,
                y1,
            } => {
                if let Some(ref mut line) = curr_line {
                    let same_baseline = (origin_y - line.main_origin_y).abs() <= 1.5
                        && x0 >= line.bbox.2 - 6.0;
                    let is_superscript_after_regular = line.has_regular_baseline
                        && size < line.main_size * 0.85
                        && (line.main_origin_y - origin_y) >= 1.5
                        && (line.main_origin_y - origin_y) <= line.main_size * 0.85
                        && (y0 - line.bbox.1).abs() <= 4.0
                        && x0 >= line.bbox.2 - 6.0;
                    let is_regular_after_leading_sup = !line.has_regular_baseline
                        && line.main_size < size * 0.85
                        && (origin_y - line.main_origin_y) >= 1.5
                        && (origin_y - line.main_origin_y) <= size * 0.85
                        && (y0 - line.bbox.1).abs() <= 4.0
                        && x0 >= line.bbox.2 - 6.0;

                    if !had_barrier
                        && (same_baseline
                            || is_superscript_after_regular
                            || is_regular_after_leading_sup)
                    {
                        let mut span_flags = style_flags;
                        if is_superscript_after_regular {
                            span_flags |= 1;
                        }
                        if is_regular_after_leading_sup {
                            line.main_origin_y = origin_y;
                            line.main_size = size;
                            line.has_regular_baseline = true;
                        } else if same_baseline && size >= 9.0 {
                            line.has_regular_baseline = true;
                        }

                        line.bbox.0 = line.bbox.0.min(x0);
                        line.bbox.1 = line.bbox.1.min(y0);
                        line.bbox.2 = line.bbox.2.max(x1);
                        line.bbox.3 = line.bbox.3.max(y1);

                        if let Some(last_span) = line.spans.last() {
                            let prev_x1 = last_span.bbox.2;
                            let gap = x0 - prev_x1;
                            let space_thresh = size * 0.278 * 0.6;
                            if gap >= space_thresh
                                && !last_span.text.ends_with(char::is_whitespace)
                                && !text.starts_with(char::is_whitespace)
                            {
                                line.spans.push(PdfSpan {
                                    text: " ".to_string(),
                                    size,
                                    flags: 0,
                                    font: font_name.clone(),
                                    origin: (prev_x1, line.main_origin_y),
                                    bbox: (prev_x1, y0, x0, y1),
                                });
                            }
                        }

                        let can_merge = if let Some(last_span) = line.spans.last() {
                            last_span.font == font_name
                                && (last_span.size - size).abs() < 0.05
                                && last_span.flags == span_flags
                                && (last_span.origin.1 - origin_y).abs() < 0.5
                        } else {
                            false
                        };

                        if can_merge {
                            if let Some(last_span) = line.spans.last_mut() {
                                last_span.text.push_str(&text);
                                last_span.bbox.1 = last_span.bbox.1.min(y0);
                                last_span.bbox.2 = last_span.bbox.2.max(x1);
                                last_span.bbox.3 = last_span.bbox.3.max(y1);
                            }
                        } else {
                            line.spans.push(PdfSpan {
                                text,
                                size,
                                flags: span_flags,
                                font: font_name,
                                origin: (origin_x, origin_y),
                                bbox: (x0, y0, x1, y1),
                            });
                        }
                        continue;
                    }
                }

                if let Some(finished_line) = curr_line.take() {
                    let pdf_line = PdfLine {
                        bbox: finished_line.bbox,
                        spans: finished_line.spans,
                    };
                    curr_block_lines.push(pdf_line);
                }

                let should_split_block = if had_barrier {
                    true
                } else if let Some(prev_line) = curr_block_lines.last() {
                    let prev_h = (prev_line.bbox.3 - prev_line.bbox.1).max(8.0);
                    let v_gap = y0 - prev_line.bbox.3;
                    v_gap > prev_h * 0.55 || y0 < prev_line.bbox.1 - 4.0
                } else {
                    false
                };

                if should_split_block && !curr_block_lines.is_empty() {
                    blocks.push(make_block(std::mem::take(&mut curr_block_lines)));
                }
                had_barrier = false;

                let has_reg = size >= 9.0;
                curr_line = Some(ActiveLine {
                    main_origin_y: origin_y,
                    main_size: size,
                    has_regular_baseline: has_reg,
                    bbox: (x0, y0, x1, y1),
                    spans: vec![PdfSpan {
                        text,
                        size,
                        flags: style_flags,
                        font: font_name,
                        origin: (origin_x, origin_y),
                        bbox: (x0, y0, x1, y1),
                    }],
                });
            }
        }
    }

    if let Some(finished_line) = curr_line.take() {
        curr_block_lines.push(PdfLine {
            bbox: finished_line.bbox,
            spans: finished_line.spans,
        });
    }
    if !curr_block_lines.is_empty() {
        blocks.push(make_block(curr_block_lines));
    }

    blocks
}

fn make_block(lines: Vec<PdfLine>) -> PdfBlock {
    let mut x0 = f64::MAX;
    let mut y0 = f64::MAX;
    let mut x1 = f64::MIN;
    let mut y1 = f64::MIN;
    for l in &lines {
        x0 = x0.min(l.bbox.0);
        y0 = y0.min(l.bbox.1);
        x1 = x1.max(l.bbox.2);
        y1 = y1.max(l.bbox.3);
    }
    PdfBlock {
        block_type: 0,
        bbox: (x0, y0, x1, y1),
        lines,
    }
}

fn extract_image_by_xref(doc: &Document, obj_id: ObjectId) -> Option<PdfExtractedXrefImage> {
    let stream = doc.get_object(obj_id).ok()?.as_stream().ok()?;
    let dict = &stream.dict;
    let width = dict.get(b"Width").ok().and_then(obj_as_f64)? as u32;
    let height = dict.get(b"Height").ok().and_then(obj_as_f64)? as u32;
    if width == 0 || height == 0 {
        return None;
    }

    let filter = match dict.get(b"Filter") {
        Ok(Object::Name(n)) => Some(n.as_slice()),
        Ok(Object::Array(arr)) => arr.first().and_then(|o| o.as_name().ok()),
        _ => None,
    };

    if filter == Some(b"DCTDecode") {
        let data = stream.content.clone();
        let raw_md5 = md5_hex(&data);
        return Some(PdfExtractedXrefImage {
            width,
            height,
            ext: "jpeg".to_string(),
            data,
            raw_md5,
        });
    }

    let decompressed = stream
        .decompressed_content()
        .unwrap_or_else(|_| stream.content.clone());

    let smask_bytes = if let Ok(Object::Reference(smask_id)) = dict.get(b"SMask") {
        doc.get_object(*smask_id)
            .ok()
            .and_then(|o| o.as_stream().ok())
            .and_then(|s| s.decompressed_content().ok())
    } else {
        None
    };

    let bpc = dict
        .get(b"BitsPerComponent")
        .ok()
        .and_then(obj_as_f64)
        .unwrap_or(8.0) as u32;

    if bpc != 8 {
        return None;
    }

    let expected_rgb = (width * height * 3) as usize;
    let expected_gray = (width * height) as usize;

    let dyn_img = if decompressed.len() >= expected_rgb {
        if let Some(alpha) = smask_bytes {
            if alpha.len() >= expected_gray {
                let mut rgba = RgbaImage::new(width, height);
                for y in 0..height {
                    for x in 0..width {
                        let idx = (y * width + x) as usize;
                        rgba.put_pixel(
                            x,
                            y,
                            Rgba([
                                decompressed[idx * 3],
                                decompressed[idx * 3 + 1],
                                decompressed[idx * 3 + 2],
                                alpha[idx],
                            ]),
                        );
                    }
                }
                DynamicImage::ImageRgba8(rgba)
            } else {
                let rgb = RgbImage::from_raw(width, height, decompressed[..expected_rgb].to_vec())?;
                DynamicImage::ImageRgb8(rgb)
            }
        } else {
            let rgb = RgbImage::from_raw(width, height, decompressed[..expected_rgb].to_vec())?;
            DynamicImage::ImageRgb8(rgb)
        }
    } else if decompressed.len() >= expected_gray {
        let gray = GrayImage::from_raw(width, height, decompressed[..expected_gray].to_vec())?;
        DynamicImage::ImageLuma8(gray)
    } else {
        return None;
    };

    let mut png_bytes = Vec::new();
    dyn_img
        .write_to(&mut Cursor::new(&mut png_bytes), ImageFormat::Png)
        .ok()?;
    let raw_md5 = md5_hex(&png_bytes);

    Some(PdfExtractedXrefImage {
        width,
        height,
        ext: "png".to_string(),
        data: png_bytes,
        raw_md5,
    })
}

pub fn compute_dhash_from_bytes(img_bytes: &[u8]) -> Option<String> {
    let img = img_hash::image::load_from_memory(img_bytes).ok()?;
    let hasher = HasherConfig::new()
        .hash_alg(HashAlg::Gradient)
        .hash_size(8, 8)
        .to_hasher();
    let hash = hasher.hash_image(&img);
    Some(hash.as_bytes().iter().map(|b| format!("{:02x}", b)).collect())
}

pub fn hamming_distance_hex(h1: &str, h2: &str) -> u32 {
    if h1.len() != 16 || h2.len() != 16 {
        return 64;
    }
    let Ok(v1) = u64::from_str_radix(h1, 16) else {
        return 64;
    };
    let Ok(v2) = u64::from_str_radix(h2, 16) else {
        return 64;
    };
    let Ok(ih1) = ImageHash::<[u8; 8]>::from_bytes(&v1.to_be_bytes()) else {
        return 64;
    };
    let Ok(ih2) = ImageHash::<[u8; 8]>::from_bytes(&v2.to_be_bytes()) else {
        return 64;
    };
    ih1.dist(&ih2)
}

pub fn md5_hex(data: &[u8]) -> String {
    let Ok(mut hasher) = RustCryptoProvider::new().hasher(HashAlgorithm::Md5) else {
        return String::new();
    };
    hasher.update(data);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect()
}

pub fn page_char_count(page: &PdfPageData) -> usize {
    let mut text = String::new();
    for b in &page.blocks {
        if b.block_type != 0 {
            continue;
        }
        for l in &b.lines {
            for s in &l.spans {
                text.push_str(&s.text);
            }
        }
    }
    text.trim().chars().count()
}

pub fn page_has_large_image(page: &PdfPageData, doc: Option<&Document>) -> bool {
    if page.image_refs.is_empty() {
        return false;
    }
    let min_rect_w = (page.width * 0.35).min(200.0).max(120.0);
    let min_rect_h = (page.height * 0.35).min(200.0).max(120.0);

    for img_ref in &page.image_refs {
        for &(x0, y0, x1, y1) in &img_ref.rects {
            if (x1 - x0).abs() >= min_rect_w && (y1 - y0).abs() >= min_rect_h {
                return true;
            }
        }
        if let Some(d) = doc {
            if let Ok(obj) = d.get_object((img_ref.xref, 0)) {
                if let Ok(stream) = obj.as_stream() {
                    let w = stream
                        .dict
                        .get(b"Width")
                        .ok()
                        .and_then(obj_as_f64)
                        .unwrap_or(0.0);
                    let h = stream
                        .dict
                        .get(b"Height")
                        .ok()
                        .and_then(obj_as_f64)
                        .unwrap_or(0.0);
                    if w >= 250.0 && h >= 250.0 {
                        return true;
                    }
                }
            }
        } else if img_ref.rects.is_empty() && img_ref.xref > 0 {
            return true;
        }
    }
    false
}

pub fn detect_scanned_pages_from_pages(
    pages: &[PdfPageData],
    doc: Option<&Document>,
) -> Vec<usize> {
    let mut scanned = Vec::new();
    for (idx, page) in pages.iter().enumerate() {
        if page_char_count(page) < 30 && page_has_large_image(page, doc) {
            scanned.push(idx + 1);
        }
    }
    scanned
}
