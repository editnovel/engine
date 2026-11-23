use editnovel_engine::cleaner::TextCleaner;
use editnovel_engine::config::{
    DelimiterLevel, LanguageProfile, ParseOptions, PresetProfile, resolve_language,
};
use editnovel_engine::epub_builder::{EpubBuilder, sanitize_html_for_xhtml};
use editnovel_engine::models::{Paragraph, Section, VolumeInfo};
use editnovel_engine::text_utils::detect_language_from_text;
use editnovel_engine::{
    FrontendEditorialRule, FrontendGlossaryReplacement, FrontendRegexRule,
    TransformSectionResult, WasmExportRequest, WasmParsedBookPayload, apply_transforms_internal,
    build_epub_from_json, compress_brotli, compute_rules_signature, decompress_brotli,
    filter_discarded_images_html, init_dictionary, inject_ocr_pages_json,
    parse_book_file_with_options_json, read_zip_entries_rc, transform_section_blocks_json,
};
use lopdf::{Dictionary, Document, Object, Stream, content::Content, content::Operation};
use std::collections::HashSet;
use std::fs;
use std::path::Path;

fn build_synthetic_scanned_pdf() -> Vec<u8> {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let img_id = doc.new_object_id();
    let content_id = doc.new_object_id();
    let page_id = doc.new_object_id();

    let mut img_dict = Dictionary::new();
    img_dict.set("Type", Object::Name(b"XObject".to_vec()));
    img_dict.set("Subtype", Object::Name(b"Image".to_vec()));
    img_dict.set("Width", Object::Integer(400));
    img_dict.set("Height", Object::Integer(600));
    img_dict.set("ColorSpace", Object::Name(b"DeviceGray".to_vec()));
    img_dict.set("BitsPerComponent", Object::Integer(8));
    let raw_pixels = vec![200u8; 400 * 600];
    doc.objects
        .insert(img_id, Object::Stream(Stream::new(img_dict, raw_pixels)));

    let content = Content {
        operations: vec![
            Operation::new("q", vec![]),
            Operation::new(
                "cm",
                vec![
                    400.into(),
                    0.into(),
                    0.into(),
                    600.into(),
                    50.into(),
                    100.into(),
                ],
            ),
            Operation::new("Do", vec![Object::Name(b"Im1".to_vec())]),
            Operation::new("Q", vec![]),
        ],
    };
    let content_bytes = content.encode().unwrap();
    doc.objects.insert(
        content_id,
        Object::Stream(Stream::new(Dictionary::new(), content_bytes)),
    );

    let mut xobj_dict = Dictionary::new();
    xobj_dict.set("Im1", Object::Reference(img_id));
    let mut res_dict = Dictionary::new();
    res_dict.set("XObject", Object::Dictionary(xobj_dict));

    let mut page_dict = Dictionary::new();
    page_dict.set("Type", Object::Name(b"Page".to_vec()));
    page_dict.set("Parent", Object::Reference(pages_id));
    page_dict.set(
        "MediaBox",
        Object::Array(vec![0.into(), 0.into(), 595.into(), 842.into()]),
    );
    page_dict.set("Contents", Object::Reference(content_id));
    page_dict.set("Resources", Object::Dictionary(res_dict));
    doc.objects.insert(page_id, Object::Dictionary(page_dict));

    let mut pages_dict = Dictionary::new();
    pages_dict.set("Type", Object::Name(b"Pages".to_vec()));
    pages_dict.set("Kids", Object::Array(vec![Object::Reference(page_id)]));
    pages_dict.set("Count", Object::Integer(1));
    doc.objects.insert(pages_id, Object::Dictionary(pages_dict));

    let catalog_id = doc.add_object(Dictionary::from_iter(vec![
        ("Type", Object::Name(b"Catalog".to_vec())),
        ("Pages", Object::Reference(pages_id)),
    ]));
    doc.trailer.set("Root", Object::Reference(catalog_id));

    let mut out = Vec::new();
    doc.save_to(&mut out).unwrap();
    out
}

#[test]
fn test_brotli_dictionaries_generation_and_dynamic_init() {
    let out_dir = Path::new("data/dicts_br");
    fs::create_dir_all(out_dir).expect("Failed to create data/dicts_br");

    let pairs = [
        ("es", "data/dictionaries/es_frequency.txt", "data/dicts_br/es_frequency.txt.br"),
        ("en", "data/dictionaries/en_frequency.txt", "data/dicts_br/en_frequency.txt.br"),
        ("ja", "data/dictionaries/ja_romaji_common.tsv", "data/dicts_br/ja_romaji_common.tsv.br"),
    ];

    for (lang, src_path, dst_path) in pairs {
        let raw = fs::read(src_path).expect("Missing source dictionary");
        let compressed = compress_brotli(&raw).expect("Brotli compression failed");
        assert!(!compressed.is_empty());
        assert!(compressed.len() < raw.len());
        fs::write(dst_path, &compressed).expect("Failed to write .br dictionary");

        let decompressed = decompress_brotli(&compressed).expect("Brotli decompression failed");
        assert_eq!(decompressed, raw);

        init_dictionary(lang, &compressed).expect("Failed to init dictionary from brotli bytes");
        init_dictionary(lang, &raw).expect("Failed to init dictionary from raw bytes");
    }
}

#[test]
fn test_rc_zip_epub_ingestion_and_entries() {
    let builder = EpubBuilder::new();
    let mut sec = Section::new("Capítulo 1: El Despertar".to_string(), None, 1, 2);
    sec.paragraphs.push(Paragraph::with_style(
        "El caballero caminaba por el bosque mientras pensaba en la batalla que había dejado atrás.",
        "narrative",
    ));
    sec.paragraphs
        .push(Paragraph::with_style("—Continuemos adelante.", "dialogue"));

    let vol = VolumeInfo {
        pdf_path: "novela_vol01.pdf".to_string(),
        volume_number: 1,
        title: "Crónicas - Volumen 01".to_string(),
        total_pages: 2,
        cover_image: None,
        color_images: Vec::new(),
        sections: vec![sec],
        all_verdicts: Vec::new(),
        spell_transformations: Vec::new(),
        publisher: Some("Editorial Test".to_string()),
        description: Some("Una sinopsis de prueba suficientemente larga para el test.".to_string()),
    };

    let epub_bytes = builder.build_epub_bytes(&vol).expect("EPUB build failed");
    let entries = read_zip_entries_rc(&epub_bytes).expect("rc-zip read failed");
    assert_eq!(
        entries.get("mimetype").map(|v| v.as_slice()),
        Some(&b"application/epub+zip"[..])
    );
    assert!(entries.contains_key("OEBPS/content.opf"));
    assert!(entries.contains_key("OEBPS/nav.xhtml"));

    let parsed_json = parse_book_file_with_options_json(
        &epub_bytes,
        "cronicas_vol01.epub",
        r#"{"language":"auto","presetId":"visual-novel"}"#,
    )
    .expect("Failed to parse EPUB via rc-zip");

    let payload: WasmParsedBookPayload = serde_json::from_str(&parsed_json).unwrap();
    assert_eq!(payload.metadata.title, "Crónicas - Volumen 01");
    assert_eq!(payload.metadata.language, "es");
    assert!(!payload.needs_ocr);
    assert!(payload.scanned_page_numbers.is_empty());
    assert_eq!(payload.groups.len(), 1);
    assert_eq!(payload.groups[0].sections.len(), 1);
}

#[test]
fn test_lol_html_text_node_transforms_and_xhtml_sanitization() {
    let html_input = r#"<div class="card--main" data-hero="Rimuru..."><p class="line--1">Rimuru said -- "Wait..."</p><img src="../Images/Rimuru--01.jpg" alt="Rimuru..."></div>"#;
    let rules = vec![
        FrontendEditorialRule {
            id: "rule-dialogue-dashes".to_string(),
            label: String::new(),
            description: String::new(),
            enabled: true,
            affected_count: 1,
        },
        FrontendEditorialRule {
            id: "rule-curly-quotes".to_string(),
            label: String::new(),
            description: String::new(),
            enabled: true,
            affected_count: 1,
        },
        FrontendEditorialRule {
            id: "rule-ellipsis".to_string(),
            label: String::new(),
            description: String::new(),
            enabled: true,
            affected_count: 1,
        },
    ];
    let glossary = vec![FrontendGlossaryReplacement {
        id: "g1".to_string(),
        find: "Rimuru".to_string(),
        replace: "Lord Rimuru".to_string(),
        enabled: true,
    }];

    let out = apply_transforms_internal(html_input, &rules, &[], &glossary);
    assert!(out.contains(r#"class="card--main""#));
    assert!(out.contains(r#"data-hero="Rimuru...""#));
    assert!(out.contains(r#"src="../Images/Rimuru--01.jpg""#));
    assert!(out.contains(r#"alt="Rimuru...""#));
    assert!(out.contains("Lord Rimuru said — “Wait…”"));

    let img_html = r#"<div class="inline-image"><img src="../Images/keep.jpg" alt="Keep"></div><div class="inline-image"><img src="../Images/discard.jpg" alt="Discard"></div>"#;
    let mut discarded = HashSet::new();
    discarded.insert("discard.jpg".to_string());
    let filtered = filter_discarded_images_html(img_html, &discarded);
    assert!(filtered.contains("keep.jpg"));
    assert!(!filtered.contains("discard.jpg"));
    assert_eq!(filtered.matches("inline-image").count(), 1);

    let dirty_xhtml = r#"<p>Hello&nbsp;world<span class="noteref" data-fn-id="fn-7"><sup>[7]</sup></span><br><hr class="sep"><img src="../Images/pic.png" alt="Pic"></p>"#;
    let clean_xhtml = sanitize_html_for_xhtml(dirty_xhtml);
    assert!(clean_xhtml.contains("&#160;"));
    assert!(clean_xhtml.contains(r##"<a href="#fn-7" id="fnref-7" class="noteref" epub:type="noteref" role="doc-noteref"><sup>[7]</sup></a>"##));
    assert!(clean_xhtml.contains("<br />"));
    assert!(clean_xhtml.contains(r#"<hr class="sep" />"#));
    assert!(clean_xhtml.contains(r#"<img src="../Images/pic.png" alt="Pic" />"#));
}

#[test]
fn test_aho_corasick_leftmost_longest_and_block_transform_cache() {
    let glossary = vec![
        FrontendGlossaryReplacement {
            id: "g1".to_string(),
            find: "Rimuru".to_string(),
            replace: "Sir Rimuru".to_string(),
            enabled: true,
        },
        FrontendGlossaryReplacement {
            id: "g2".to_string(),
            find: "Rimuru Tempest".to_string(),
            replace: "Demon Lord Rimuru Tempest".to_string(),
            enabled: true,
        },
    ];
    let regex_rules = vec![FrontendRegexRule {
        id: "r1".to_string(),
        pattern: r"\bLVL\s*(\d+)\b".to_string(),
        replacement: "Level $1".to_string(),
        enabled: true,
        flags: "gi".to_string(),
        affected_count: 1,
    }];

    let sig1 = compute_rules_signature(&[], &regex_rules, &glossary);
    let sig2 = compute_rules_signature(&[], &regex_rules, &glossary);
    assert_eq!(sig1.len(), 16);
    assert_eq!(sig1, sig2);

    let req_json = serde_json::json!({
        "requestId": 42,
        "sectionId": "sec-1",
        "html": "<p>Rimuru Tempest greeted Rimuru at LVL 99.</p>\n<p>Second paragraph with Rimuru.</p>",
        "editorialRules": [],
        "regexRules": regex_rules,
        "glossary": glossary
    })
    .to_string();

    let res_str = transform_section_blocks_json(&req_json).expect("Transform failed");
    let res: TransformSectionResult = serde_json::from_str(&res_str).unwrap();
    assert_eq!(res.request_id, 42);
    assert_eq!(res.section_id, "sec-1");
    assert_eq!(res.rules_signature, sig1);
    assert!(res.html.contains("Demon Lord Rimuru Tempest greeted Sir Rimuru at Level 99."));
    assert!(res.html.contains("Second paragraph with Sir Rimuru."));
}

#[test]
fn test_language_profile_and_preset_profile_en_es() {
    let en_sample = "The knight walked through the dark forest after the battle because he had to find the hidden dungeon before nightfall.";
    let es_sample = "El caballero caminaba por el bosque oscuro después de la batalla porque tenía que encontrar el calabozo antes del anochecer.";

    assert_eq!(detect_language_from_text(en_sample), "en");
    assert_eq!(detect_language_from_text(es_sample), "es");
    assert_eq!(resolve_language("auto", en_sample, "webnovel"), "en");
    assert_eq!(resolve_language("auto", es_sample, "visual-novel"), "es");

    let en_cleaner = TextCleaner::with_profiles(
        LanguageProfile::for_language("en"),
        PresetProfile::from_id("webnovel"),
    );
    assert_eq!(
        en_cleaner.canonical_delimiters_for_level(DelimiterLevel::Level1SpokenDialogue),
        ("“", "”")
    );
    let en_diag = en_cleaner.standardize_paragraph_text("「We must enter the dungeon now!」", "narrative");
    assert_eq!(en_diag, "“We must enter the dungeon now!”");
    assert_eq!(en_cleaner.classify_paragraph_style(&en_diag, None), "dialogue");

    let en_sys = en_cleaner.standardize_paragraph_text("【Skill Acquired: Shadow Step】", "narrative");
    assert_eq!(en_sys, "[Skill Acquired: Shadow Step]");
    assert_eq!(en_cleaner.classify_paragraph_style(&en_sys, None), "bracket_speech");

    let en_skill = en_cleaner.standardize_paragraph_text("He activated≪ Shadow Step ≫immediately.", "narrative");
    assert_eq!(en_skill, "He activated 《Shadow Step》 immediately.");

    let paper_cleaner = TextCleaner::with_profiles(
        LanguageProfile::for_language("en"),
        PresetProfile::from_id("paper"),
    );
    assert!(!paper_cleaner.is_scene_break("* * *"));
    assert_eq!(
        paper_cleaner.classify_paragraph_style("[Citation 12]", None),
        "narrative"
    );

    let opts = ParseOptions {
        language: "en".to_string(),
        preset_id: "webnovel".to_string(),
    };
    let builder = EpubBuilder::with_options(&opts);
    assert_eq!(builder.language, "en");
    assert!(!builder.preset.enable_drop_caps);
    assert_eq!(builder.preset.footnote_style, "callout_card");
}

#[test]
fn test_needs_ocr_and_inject_ocr_pages_pipeline() {
    let scanned_pdf = build_synthetic_scanned_pdf();
    let parsed_json = parse_book_file_with_options_json(
        &scanned_pdf,
        "scanned_vol01.pdf",
        r#"{"language":"auto","presetId":"webnovel"}"#,
    )
    .expect("Failed to parse synthetic scanned PDF");

    let initial_payload: WasmParsedBookPayload = serde_json::from_str(&parsed_json).unwrap();
    assert!(initial_payload.needs_ocr);
    assert_eq!(initial_payload.scanned_page_numbers, vec![1]);

    let ocr_pages_json = serde_json::json!([
        {
            "pageNumber": 1,
            "width": 595.0,
            "height": 842.0,
            "lines": [
                { "text": "Chapter 1: The Awakening", "bbox": [72.0, 80.0, 350.0, 102.0], "size": 18.0, "isBold": true },
                { "text": "The young warrior opened his eyes inside the ancient labyrinth and looked at the glowing crystal above him.", "bbox": [72.0, 130.0, 510.0, 145.0] },
                { "text": "\"Where am I right now, and why is this dungeon shaking?\" he asked while drawing his blade.", "bbox": [72.0, 175.0, 510.0, 190.0] },
                { "text": "[System Notification: Quest Started]", "bbox": [72.0, 220.0, 410.0, 235.0] }
            ]
        }
    ])
    .to_string();

    let injected_json = inject_ocr_pages_json(
        "scanned_vol01.pdf",
        &ocr_pages_json,
        r#"{"language":"auto","presetId":"webnovel"}"#,
    )
    .expect("OCR injection failed");

    let updated_payload: WasmParsedBookPayload = serde_json::from_str(&injected_json).unwrap();
    assert!(!updated_payload.needs_ocr);
    assert!(updated_payload.scanned_page_numbers.is_empty());
    assert_eq!(updated_payload.metadata.language, "en");
    assert!(!updated_payload.groups.is_empty());
    let secs = &updated_payload.groups[0].sections;
    assert!(!secs.is_empty());
    let chap_sec = secs
        .iter()
        .find(|s| s.title.contains("Chapter 1") || s.title.contains("Capítulo 1"))
        .expect("Missing Chapter 1 section after OCR injection");
    assert!(chap_sec.html_content.contains("The young warrior opened his eyes"));
    assert!(chap_sec.html_content.contains("“Where am I right now"));
    assert!(chap_sec.html_content.contains("[System Notification: Quest Started]"));

    let export_req = serde_json::json!({
        "metadata": updated_payload.metadata,
        "groups": updated_payload.groups,
        "images": updated_payload.images,
        "editorialRules": updated_payload.detected_rules,
        "regexRules": [],
        "glossary": []
    })
    .to_string();
    let _req_check: WasmExportRequest = serde_json::from_str(&export_req).unwrap();
    let epub_out = build_epub_from_json(&export_req).expect("Failed to export EPUB after OCR");
    let entries = read_zip_entries_rc(&epub_out).expect("Failed to read exported EPUB");
    let opf = String::from_utf8_lossy(entries.get("OEBPS/content.opf").unwrap());
    assert!(opf.contains("<dc:language>en</dc:language>"));
}
