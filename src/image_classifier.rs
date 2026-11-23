use crate::models::{ExtractedImage, PdfExtractedXrefImage, PdfPageData};
use crate::pdf_extractor::{compute_dhash_from_bytes, ParsedPdfDocument};
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub struct HybridImageClassifier {
    toc_heading_re: Regex,
    credits_role_re: Regex,
}

impl Default for HybridImageClassifier {
    fn default() -> Self {
        Self::new()
    }
}

impl HybridImageClassifier {
    pub fn new() -> Self {
        Self {
            toc_heading_re: Regex::new(r"(?m)^\s*(?:índice|indice)\b").unwrap(),
            credits_role_re: Regex::new(
                r"(?im)^\s*(?:traduci[oó]n|traducci[oó]n|traductor(?:a)?|correc?ci[oó]n|corrector(?:a)?|edici[oó]n|editor(?:a)?|limpieza|redraw|typeset(?:ting)?|diseño|staff|cr[eé]ditos)\b\s*[:\-]",
            )
            .unwrap(),
        }
    }

    pub fn is_front_matter_template_page(&self, page_num: usize, text: &str) -> (bool, String) {
        if page_num > 10 {
            return (false, String::new());
        }
        let clean = text.to_lowercase();
        let toc_kw = [
            "índice de contenidos",
            "indice de contenidos",
            "tabla de contenido",
            "tabla de contenidos",
            "table of contents",
        ];
        for kw in toc_kw {
            if clean.contains(kw) {
                return (true, format!("Visual Table of Contents ('{}')", kw));
            }
        }
        if self.toc_heading_re.is_match(&clean) {
            let headings = [
                "capítulo",
                "capitulo",
                "prólogo",
                "prologo",
                "epílogo",
                "epilogo",
            ];
            if headings.iter().any(|h| clean.contains(h)) {
                return (true, "Visual Table of Contents heading".to_string());
            }
        }
        let credits_terms = [
            "traducción japonés",
            "traduccion japones",
            "traducción al inglés",
            "traduccion al ingles",
            "traducción al español",
            "traduccion al español",
            "traducción inglés",
            "traduccion ingles",
            "traducción español",
            "traduccion español",
        ];
        let has_credits = credits_terms.iter().any(|term| clean.contains(term))
            || self.credits_role_re.find_iter(&clean).count() >= 2;
        let has_orig = clean.contains("obra original");
        let disc_terms = [
            "no es la traducción oficial",
            "no es la traduccion oficial",
            "versión hecha de fans",
            "version hecha de fans",
            "agradecemos tu preferencia",
            "visítanos en nuestra página web",
            "visitanos en nuestra pagina web",
            "visítanos en nuestro",
            "visitanos en nuestro",
            "sin fines de lucro",
            "apoya al autor",
        ];
        let has_disc = disc_terms.iter().any(|d| clean.contains(d));
        if (has_orig && has_credits) || has_disc {
            return (true, "Scanlation credits colophon".to_string());
        }
        (false, String::new())
    }

    pub fn classify_and_filter_document(
        &self,
        doc: &ParsedPdfDocument,
        doc_name: &str,
    ) -> (
        Vec<ExtractedImage>,
        Vec<ExtractedImage>,
        HashMap<usize, Vec<ExtractedImage>>,
        Vec<ExtractedImage>,
    ) {
        let mut cover_image: Vec<ExtractedImage> = Vec::new();
        let mut color_inserts: Vec<ExtractedImage> = Vec::new();
        let mut page_images: HashMap<usize, Vec<ExtractedImage>> = HashMap::new();
        let mut verdicts: Vec<ExtractedImage> = Vec::new();

        let doc_stem = Path::new(doc_name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("volume");

        let mut seen_xrefs: HashSet<u32> = HashSet::new();
        let mut seen_dhashes: HashMap<String, usize> = HashMap::new();
        let mut dhash_xrefs: HashSet<u32> = HashSet::new();
        let mut extracted_cache: HashMap<u32, (PdfExtractedXrefImage, String)> = HashMap::new();

        for page in &doc.pages {
            for img_ref in &page.image_refs {
                if !dhash_xrefs.insert(img_ref.xref) {
                    continue;
                }
                if let Some(extracted) = doc.extract_image_by_xref(img_ref.xref) {
                    let dh = if extracted.width >= 250 && extracted.height >= 250 {
                        compute_dhash_from_bytes(&extracted.data).unwrap_or_default()
                    } else {
                        String::new()
                    };
                    if !dh.is_empty() {
                        *seen_dhashes.entry(dh.clone()).or_insert(0) += 1;
                    }
                    extracted_cache.insert(img_ref.xref, (extracted, dh));
                }
            }
        }

        for (pno, page) in doc.pages.iter().enumerate() {
            if page.image_refs.is_empty() {
                continue;
            }

            let text = extract_page_plain_text(page);
            let text_trimmed = text.trim();
            let text_len = text_trimmed.chars().count();

            let (is_template, template_reason) =
                self.is_front_matter_template_page(pno + 1, text_trimmed);
            if is_template {
                for (img_idx, img_ref) in page.image_refs.iter().enumerate() {
                    let xref = img_ref.xref;
                    if !seen_xrefs.insert(xref) {
                        continue;
                    }
                    let Some((extracted, _)) = extracted_cache.remove(&xref) else {
                        continue;
                    };
                    let fname = format!(
                        "{}_p{:03}_img{}.{}",
                        doc_stem,
                        pno + 1,
                        img_idx + 1,
                        extracted.ext
                    );
                    let y_pos = img_ref.rects.first().map(|r| r.1).unwrap_or(0.0);
                    verdicts.push(ExtractedImage {
                        page_num: pno + 1,
                        image_index: img_idx + 1,
                        ext: extracted.ext,
                        data: extracted.data,
                        width: extracted.width,
                        height: extracted.height,
                        filename: fname,
                        category: "template_backgrounds".to_string(),
                        y_pos,
                        is_full_page: false,
                        is_cover: false,
                        is_kept: false,
                        reason: format!(
                            "Scanlation layout template on front-matter page ({})",
                            template_reason
                        ),
                    });
                }
                continue;
            }

            for (img_idx, img_ref) in page.image_refs.iter().enumerate() {
                let xref = img_ref.xref;
                if !seen_xrefs.insert(xref) {
                    continue;
                }
                let Some((extracted, dhash)) = extracted_cache.remove(&xref) else {
                    continue;
                };
                let width = extracted.width;
                let height = extracted.height;
                let aspect = if width > 0 && height > 0 {
                    (width as f64 / height as f64).max(height as f64 / width as f64)
                } else {
                    1.0
                };
                let fname = format!(
                    "{}_p{:03}_img{}.{}",
                    doc_stem,
                    pno + 1,
                    img_idx + 1,
                    extracted.ext
                );
                let y_pos = img_ref.rects.first().map(|r| r.1).unwrap_or(0.0);

                if width < 250 || height < 250 {
                    verdicts.push(ExtractedImage {
                        page_num: pno + 1,
                        image_index: img_idx + 1,
                        ext: extracted.ext,
                        data: extracted.data,
                        width,
                        height,
                        filename: fname,
                        category: "icons_and_avatars".to_string(),
                        y_pos,
                        is_full_page: false,
                        is_cover: false,
                        is_kept: false,
                        reason: format!("Sub-resolution icon or stamp ({}x{} < 250px)", width, height),
                    });
                    continue;
                }

                let is_repeated_dhash =
                    !dhash.is_empty() && seen_dhashes.get(&dhash).copied().unwrap_or(0) >= 3;

                if is_repeated_dhash {
                    verdicts.push(ExtractedImage {
                        page_num: pno + 1,
                        image_index: img_idx + 1,
                        ext: extracted.ext,
                        data: extracted.data,
                        width,
                        height,
                        filename: fname,
                        category: "template_backgrounds".to_string(),
                        y_pos,
                        is_full_page: false,
                        is_cover: false,
                        is_kept: false,
                        reason: format!(
                            "Recurrent scanlation template graphic pattern (dHash: {})",
                            dhash
                        ),
                    });
                    continue;
                }

                if aspect > 3.2 && (height < 250 || width < 800) {
                    verdicts.push(ExtractedImage {
                        page_num: pno + 1,
                        image_index: img_idx + 1,
                        ext: extracted.ext,
                        data: extracted.data,
                        width,
                        height,
                        filename: fname,
                        category: "banners_and_badges".to_string(),
                        y_pos,
                        is_full_page: false,
                        is_cover: false,
                        is_kept: false,
                        reason: format!(
                            "Extreme aspect ratio ribbon or banner ({:.1}:1, {}x{})",
                            aspect, width, height
                        ),
                    });
                    continue;
                }

                let is_full = width >= 400 && height >= 500 && text_len < 200;
                let is_cov = pno == 0 && cover_image.is_empty();
                let is_insert = pno <= 6 && is_full && !is_cov;

                let cat_name = if is_cov {
                    "covers"
                } else if is_insert {
                    "color_inserts"
                } else {
                    "story_illustrations"
                };

                let item = ExtractedImage {
                    page_num: pno + 1,
                    image_index: img_idx + 1,
                    ext: extracted.ext,
                    data: extracted.data,
                    width,
                    height,
                    filename: fname,
                    category: cat_name.to_string(),
                    y_pos,
                    is_full_page: is_full,
                    is_cover: is_cov,
                    is_kept: true,
                    reason: format!(
                        "Legitimate publication graphic ({}, {}x{})",
                        cat_name, width, height
                    ),
                };

                if is_cov {
                    cover_image.push(item.clone());
                } else if is_insert {
                    color_inserts.push(item.clone());
                } else {
                    page_images.entry(pno + 1).or_default().push(item.clone());
                }

                verdicts.push(item);
            }
        }

        (cover_image, color_inserts, page_images, verdicts)
    }
}

fn extract_page_plain_text(page: &PdfPageData) -> String {
    let mut out = String::new();
    for b in &page.blocks {
        if b.block_type != 0 {
            continue;
        }
        for l in &b.lines {
            for s in &l.spans {
                out.push_str(&s.text);
            }
            out.push('\n');
        }
    }
    out
}
