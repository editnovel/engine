use editnovel_engine::epub_builder::EpubBuilder;
use editnovel_engine::models::VolumeInfo;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Cursor, Read};
use std::path::PathBuf;
use zip::ZipArchive;

fn read_epub_entries(bytes: &[u8]) -> (BTreeMap<String, String>, BTreeMap<String, Vec<u8>>) {
    let reader = Cursor::new(bytes);
    let mut zip = ZipArchive::new(reader).unwrap();
    let mut text_map = BTreeMap::new();
    let mut bin_map = BTreeMap::new();

    for i in 0..zip.len() {
        let mut file = zip.by_index(i).unwrap();
        let name = file.name().unwrap().to_string();
        let mut buf = Vec::new();
        file.read_to_end(&mut buf).unwrap();
        if name.ends_with(".xhtml")
            || name.ends_with(".xml")
            || name.ends_with(".opf")
            || name.ends_with(".ncx")
            || name.ends_with(".css")
            || name == "mimetype"
        {
            let s = String::from_utf8_lossy(&buf).to_string();
            text_map.insert(name, s);
        } else {
            bin_map.insert(name, buf);
        }
    }

    (text_map, bin_map)
}

fn normalize_uuid(s: &str) -> String {
    let re = regex::Regex::new(r"urn:uuid:\d+").unwrap();
    let s = re.replace_all(s, "urn:uuid:NORMALIZED").to_string();
    let pub_re = regex::Regex::new(r"\s*<dc:publisher>Agnostic EPUB Converter</dc:publisher>").unwrap();
    pub_re.replace_all(&s, "").to_string()
}

#[test]
fn test_vol07_parity() {
    let Ok(fixtures_dir) = std::env::var("EDITNOVEL_FIXTURES_DIR") else {
        eprintln!("Skipping test_vol07_parity: EDITNOVEL_FIXTURES_DIR not set");
        return;
    };
    let base = PathBuf::from(fixtures_dir);
    let pdf_path_buf = base.join("pdfs").join("sample_vol07.pdf");
    let ref_epub_path_buf = base.join("output_epubs").join("sample_vol07.epub");
    if !pdf_path_buf.exists() || !ref_epub_path_buf.exists() {
        eprintln!("Skipping test_vol07_parity: fixture files not found");
        return;
    }
    let pdf_path = pdf_path_buf.to_str().unwrap();

    let pdf_bytes = fs::read(&pdf_path_buf).expect("Failed to read Vol 07 PDF");
    let ref_epub_bytes = fs::read(&ref_epub_path_buf).expect("Failed to read reference Vol 07 EPUB");

    let mut builder = EpubBuilder::new();
    let vol_info = builder
        .process_pdf_bytes(&pdf_bytes, pdf_path, true)
        .expect("Failed to process Vol 07 PDF");
    let rust_epub_bytes = builder
        .build_epub_bytes(&vol_info)
        .expect("Failed to build Vol 07 EPUB");

    let (ref_txt, ref_bin) = read_epub_entries(&ref_epub_bytes);
    let (rust_txt, rust_bin) = read_epub_entries(&rust_epub_bytes);

    let ref_img_keys: BTreeSet<_> = ref_bin.keys().collect();
    let rust_img_keys: BTreeSet<_> = rust_bin.keys().collect();
    assert_eq!(ref_img_keys, rust_img_keys, "Image keys mismatch in Vol 07");

    let ref_txt_keys: BTreeSet<_> = ref_txt.keys().collect();
    let rust_txt_keys: BTreeSet<_> = rust_txt.keys().collect();
    assert_eq!(ref_txt_keys, rust_txt_keys, "Text keys mismatch in Vol 07");

    for key in ref_txt_keys {
        let r_str = normalize_uuid(&ref_txt[key]);
        let u_str = normalize_uuid(&rust_txt[key]);
        assert_eq!(r_str, u_str, "Mismatch in Vol 07 entry {}", key);
    }
}

#[test]
#[ignore]
fn test_omnibus_19_volumes_parity() {
    use rayon::prelude::*;

    let Ok(fixtures_dir) = std::env::var("EDITNOVEL_FIXTURES_DIR") else {
        eprintln!("Skipping test_omnibus_19_volumes_parity: EDITNOVEL_FIXTURES_DIR not set");
        return;
    };
    let base = PathBuf::from(fixtures_dir);
    let pdfs_dir = base.join("pdfs");
    let ref_omnibus_path = base.join("output_epubs").join("sample_omnibus.epub");

    let ref_omnibus_bytes =
        fs::read(ref_omnibus_path).expect("Failed to read reference Omnibus EPUB");
    let (ref_txt, ref_bin) = read_epub_entries(&ref_omnibus_bytes);

    let mut pdf_files: Vec<PathBuf> = fs::read_dir(&pdfs_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("pdf"))
        .collect();
    pdf_files.sort();

    assert_eq!(pdf_files.len(), 19, "Expected 19 PDFs");

    let volumes: Vec<VolumeInfo> = pdf_files
        .par_iter()
        .map(|pdf_path| {
            let mut builder = EpubBuilder::new();
            let path_str = pdf_path.to_str().unwrap();
            let bytes = fs::read(pdf_path).unwrap();
            let vol = builder
                .process_pdf_bytes(&bytes, path_str, true)
                .unwrap_or_else(|e| panic!("Failed to process {}: {}", path_str, e));
            println!(
                "Processed Vol {:02}: {} sections, {} color images, cover={}",
                vol.volume_number,
                vol.sections.len(),
                vol.color_images.len(),
                vol.cover_image.is_some()
            );
            vol
        })
        .collect();

    let builder = EpubBuilder::new();
    let rust_omnibus_bytes = builder
        .build_omnibus_bytes(&volumes)
        .expect("Failed to build omnibus EPUB");
    let (rust_txt, rust_bin) = read_epub_entries(&rust_omnibus_bytes);

    let ref_img_keys: BTreeSet<_> = ref_bin.keys().collect();
    let rust_img_keys: BTreeSet<_> = rust_bin.keys().collect();
    if ref_img_keys != rust_img_keys {
        println!("OMNIBUS IMAGE KEYS MISMATCH:");
        println!(
            "  Missing in Rust ({}): {:?}",
            ref_img_keys.difference(&rust_img_keys).count(),
            ref_img_keys.difference(&rust_img_keys)
        );
        println!(
            "  Extra in Rust ({}):   {:?}",
            rust_img_keys.difference(&ref_img_keys).count(),
            rust_img_keys.difference(&ref_img_keys)
        );
    } else {
        println!("OMNIBUS IMAGE KEYS MATCH: {} images", ref_img_keys.len());
    }

    let ref_txt_keys: BTreeSet<_> = ref_txt.keys().collect();
    let rust_txt_keys: BTreeSet<_> = rust_txt.keys().collect();
    if ref_txt_keys != rust_txt_keys {
        println!("OMNIBUS TEXT KEYS MISMATCH:");
        println!(
            "  Missing in Rust ({}): {:?}",
            ref_txt_keys.difference(&rust_txt_keys).count(),
            ref_txt_keys.difference(&rust_txt_keys)
        );
        println!(
            "  Extra in Rust ({}):   {:?}",
            rust_txt_keys.difference(&ref_txt_keys).count(),
            rust_txt_keys.difference(&ref_txt_keys)
        );
    } else {
        println!("OMNIBUS TEXT KEYS MATCH: {} text files", ref_txt_keys.len());
    }

    let mut diff_files_count = 0usize;
    let mut total_line_diffs = 0usize;
    for key in ref_txt_keys.intersection(&rust_txt_keys) {
        let r_str = normalize_uuid(&ref_txt[*key]);
        let u_str = normalize_uuid(&rust_txt[*key]);
        if r_str != u_str {
            diff_files_count += 1;
            let r_lines: Vec<&str> = r_str.lines().collect();
            let u_lines: Vec<&str> = u_str.lines().collect();
            let max_l = r_lines.len().max(u_lines.len());
            let mut file_diffs = 0usize;
            println!(
                "\n--- DIFF in {} (ref {} lines vs rust {} lines) ---",
                key,
                r_lines.len(),
                u_lines.len()
            );
            for idx in 0..max_l {
                let rl = r_lines.get(idx).copied().unwrap_or("<EOF>");
                let ul = u_lines.get(idx).copied().unwrap_or("<EOF>");
                if rl != ul {
                    file_diffs += 1;
                    total_line_diffs += 1;
                    if file_diffs <= 5 {
                        println!("  Line {}:", idx + 1);
                        println!("    PY: {}", rl);
                        println!("    RS: {}", ul);
                    }
                }
            }
            println!("  (Total line diffs in {}: {})", key, file_diffs);
        }
    }

    println!(
        "\n=== OMNIBUS SUMMARY: {} diff files, {} total line diffs ===",
        diff_files_count, total_line_diffs
    );
    assert_eq!(ref_img_keys, rust_img_keys, "Omnibus image keys mismatch");
    assert_eq!(ref_txt_keys, rust_txt_keys, "Omnibus text keys mismatch");
    assert_eq!(
        total_line_diffs, 0,
        "Omnibus has {} line differences across {} files",
        total_line_diffs, diff_files_count
    );
}
