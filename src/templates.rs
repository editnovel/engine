pub const EPUB_CSS: &str = r#"@charset "utf-8";

@page {
    margin: 5pt;
}

body {
    margin: 0;
    padding: 0 5%;
    font-family: "Merriweather", "Charter", "Source Serif Pro", "Palatino Linotype", "Georgia", serif;
    font-size: 1.05rem;
    line-height: 1.75;
    text-rendering: optimizeLegibility;
    -webkit-font-smoothing: antialiased;
}

h1.chapter-title {
    font-size: 1.7rem;
    font-weight: 700;
    text-align: center;
    margin-top: 2.5em;
    margin-bottom: 0.5em;
    line-height: 1.3;
    letter-spacing: 0.03em;
    page-break-before: always;
}

h2.chapter-subtitle {
    font-size: 1.25rem;
    font-weight: 600;
    text-align: center;
    margin-top: 0;
    margin-bottom: 2em;
    opacity: 0.9;
    font-style: italic;
}

h3.part-heading {
    font-size: 1.25rem;
    font-weight: 700;
    text-align: center;
    margin-top: 3.5rem;
    margin-bottom: 1.5rem;
    letter-spacing: 0.08em;
    page-break-after: avoid;
    break-after: avoid;
    clear: both;
}

p {
    margin: 0;
    text-align: justify;
    text-indent: 1.6em;
    hyphens: auto;
    -webkit-hyphens: auto;
    clear: both;
}

p.drop-cap,
p.first-paragraph.drop-cap {
    text-indent: 0 !important;
}

p.drop-cap::first-letter,
p.first-paragraph.drop-cap::first-letter {
    font-size: 3.2em;
    float: left;
    margin-right: 0.12em;
    line-height: 0.82;
    font-weight: 700;
}

p.first-paragraph,
h1 + p,
h2 + p,
h3 + p,
hr.scene-break + p,
div.scene-break + p,
div.inline-image + p,
div.full-illustration-wrap + p,
aside.footnote-card + p {
    text-indent: 0;
}

p.dialogue {
    text-indent: 0;
    margin-bottom: 0.25em;
}

p.bracket-dialogue {
    text-indent: 0;
    font-style: italic;
    margin: 0.85em 0;
    padding-left: 0.85em;
    border-left: 2.5px solid rgba(128, 128, 128, 0.4);
}

p.thought {
    font-style: italic;
    text-indent: 1.6em;
}

div.scene-break {
    text-align: center;
    margin: 1.8em 0;
    font-size: 0.95rem;
    letter-spacing: 0.35em;
    opacity: 0.75;
    user-select: none;
    page-break-inside: avoid;
    break-inside: avoid;
    clear: both;
}

div.inline-image {
    text-align: center;
    margin: 1.8em auto;
    page-break-inside: avoid;
    break-inside: avoid;
    clear: both;
}

div.inline-image img {
    max-width: 100%;
    max-height: 90vh;
    height: auto;
    display: block;
    margin: 0 auto;
    border-radius: 2px;
}

div.full-illustration-wrap {
    page-break-before: always;
    page-break-after: always;
    page-break-inside: avoid;
    break-before: page;
    break-after: page;
    break-inside: avoid;
    display: block;
    width: 100%;
    margin: 1em auto;
    padding: 0;
    text-align: center;
    clear: both;
}

div.full-illustration-wrap img.full-illustration-img {
    max-width: 100%;
    max-height: 90vh;
    width: auto;
    height: auto;
    object-fit: contain;
    display: block;
    margin: 0 auto;
}

div.full-page-wrap {
    width: 100%;
    height: 100vh;
    margin: 0;
    padding: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    page-break-inside: avoid;
}

svg.full-svg {
    width: 100%;
    height: 100%;
    display: block;
}

aside.footnote-card {
    margin: 1.1em 0;
    padding: 0.65em 0.95em;
    background-color: rgba(128, 128, 128, 0.08);
    border-left: 3.5px solid rgba(128, 128, 128, 0.6);
    border-radius: 4px;
    font-size: 0.88rem;
    line-height: 1.55;
    page-break-inside: avoid;
    break-inside: avoid;
    clear: both;
}

aside.footnote-card .footnote-card-tag {
    font-weight: 700;
    text-transform: uppercase;
    font-size: 0.72rem;
    letter-spacing: 0.08em;
    opacity: 0.85;
    margin-bottom: 0.25em;
    color: inherit;
}

aside.footnote-card .footnote-card-body {
    text-indent: 0;
    margin: 0;
    text-align: left;
}

a.noteref {
    text-decoration: none;
    color: inherit;
    font-weight: 600;
    vertical-align: super;
    font-size: 0.75em;
    line-height: 0;
    padding: 0 0.15em;
}

s, del {
    text-decoration: line-through;
}

u {
    text-decoration: underline;
    text-underline-position: under;
}

sup.ordinal-sup {
    font-size: 0.72em;
    vertical-align: super;
    line-height: 0;
}

section.chapter-footnotes {
    margin-top: 3.5em;
    padding-top: 1.5em;
    border-top: 1px solid rgba(128, 128, 128, 0.35);
    font-size: 0.88rem;
    line-height: 1.55;
}

hr.footnotes-divider {
    display: none;
}

h4.footnotes-heading {
    font-size: 0.95rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    margin: 0 0 1em 0;
    opacity: 0.75;
}

aside.footnote {
    margin-bottom: 0.8em;
}

aside.footnote p {
    text-indent: 0;
    margin: 0;
    text-align: left;
}

a.footnote-backref {
    text-decoration: none;
    font-weight: bold;
    color: inherit;
    margin-right: 0.4em;
}

div.footnote-block {
    margin-top: 2.5em;
    padding-top: 1em;
    border-top: 1px solid rgba(128, 128, 128, 0.3);
    font-size: 0.85rem;
    line-height: 1.5;
}

div.footnote-item {
    margin-bottom: 0.5em;
    text-indent: 0;
}

sup.footnote-ref {
    font-size: 0.75em;
    vertical-align: super;
    line-height: 0;
}

div.volume-title-card {
    text-align: center;
    margin-top: 30vh;
    page-break-before: always;
}

h1.vol-main-title {
    font-size: 2.1rem;
    margin-bottom: 0.3em;
    font-weight: 700;
}

h2.vol-sub-title {
    font-size: 1.4rem;
    opacity: 0.8;
    font-weight: 500;
}

div.verse-block {
    margin: 1.8em auto;
    text-align: center;
    font-style: italic;
    page-break-inside: avoid;
}

p.verse-line {
    text-indent: 0;
    margin: 0.3em 0;
    line-height: 1.65;
    text-align: center;
}

blockquote.document-card {
    margin: 1.6em 1em;
    padding: 1.1em 1.4em;
    border-left: 3px solid rgba(59, 130, 246, 0.7);
    background-color: rgba(128, 128, 128, 0.05);
    border-radius: 3px;
    font-size: 0.94rem;
    page-break-inside: avoid;
}

blockquote.document-card p {
    text-indent: 0;
    margin: 0.4em 0;
}

section.colophon {
    margin-top: 4em;
    padding-top: 2em;
    border-top: 1px solid rgba(128, 128, 128, 0.2);
    text-align: center;
    font-size: 0.82rem;
    opacity: 0.8;
    page-break-before: always;
}

section.colophon p {
    text-indent: 0;
    margin: 0.4em 0;
    text-align: center;
}

"#;

pub fn render_title_page(language: &str, volume_number: &str, series_title: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" xml:lang="{language}">
<head>
  <title>Parte {volume_number}</title>
  <link rel="stylesheet" type="text/css" href="../Styles/stylesheet.css"/>
</head>
<body>
  <section epub:type="titlepage" role="doc-titlepage" aria-label="Página de título">
    <div class="volume-title-card">
      <h1 class="vol-main-title">{series_title}</h1>
      <h2 class="vol-sub-title">Parte {volume_number}</h2>
    </div>
  </section>
</body>
</html>
"#
    )
}

pub fn render_full_image(
    language: &str,
    title: &str,
    width: u32,
    height: u32,
    filename: &str,
) -> String {
    let is_cover = title.to_lowercase().contains("portada") || title.to_lowercase().contains("cover");
    let (sec_open, sec_close) = if is_cover {
        (format!(r#"<section epub:type="cover" role="doc-cover" aria-label="{}">"#, title), "</section>")
    } else {
        (r#"<section class="illustration-page">"#.to_string(), "</section>")
    };
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" xml:lang="{language}">
<head>
  <title>{title}</title>
  <link rel="stylesheet" type="text/css" href="../Styles/stylesheet.css"/>
  <meta name="viewport" content="width={width}, height={height}"/>
</head>
<body style="margin: 0; padding: 0; background-color: #111111;">
  {sec_open}
    <div class="full-page-wrap">
      <svg class="full-svg" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" version="1.1" viewBox="0 0 {width} {height}" preserveAspectRatio="xMidYMid meet">
        <image width="{width}" height="{height}" xlink:href="../Images/{filename}" href="../Images/{filename}"/>
      </svg>
    </div>
  {sec_close}
</body>
</html>
"#
    )
}

pub fn classify_section_type(title: &str) -> &'static str {
    let lower = title.trim().to_lowercase();
    if lower.starts_with("prólogo")
        || lower.starts_with("prologo")
        || lower.starts_with("prologue")
    {
        "prologue"
    } else if lower.starts_with("epílogo")
        || lower.starts_with("epilogo")
        || lower.starts_with("epilogue")
    {
        "epilogue"
    } else if lower.starts_with("interludio")
        || lower.starts_with("intermisión")
        || lower.starts_with("intermision")
        || lower.starts_with("interlude")
    {
        "interlude"
    } else if lower.starts_with("palabras")
        || lower.starts_with("posfacio")
        || lower.starts_with("afterword")
        || lower.starts_with("notas")
        || lower.starts_with("resumen")
    {
        "afterword"
    } else if lower.starts_with("parte")
        || lower.starts_with("part")
    {
        "part"
    } else {
        "chapter"
    }
}

pub fn get_section_epub_semantics(sec_type: &str) -> (&'static str, &'static str, &'static str) {
    match sec_type {
        "prologue" => ("prologue", "doc-prologue", "prologue-section"),
        "epilogue" => ("epilogue", "doc-epilogue", "epilogue-section"),
        "afterword" => ("afterword", "doc-afterword", "afterword-section"),
        "part" => ("part", "doc-part", "part-section"),
        _ => ("chapter", "doc-chapter", "chapter-section"),
    }
}

pub fn render_section(
    language: &str,
    title: &str,
    main_heading: &str,
    sub_heading_block: &str,
    body_content: &str,
    sec_type: &str,
) -> String {
    let (epub_type, doc_role, sec_class) = get_section_epub_semantics(sec_type);
    let sub_block = if !sub_heading_block.is_empty() {
        format!("{}\n", sub_heading_block)
    } else {
        String::new()
    };

    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" xml:lang="{language}" lang="{language}">
<head>
  <title>{title}</title>
  <link rel="stylesheet" type="text/css" href="../Styles/stylesheet.css"/>
</head>
<body class="chapter-body" epub:type="{epub_type}" role="{doc_role}">
  <section epub:type="{epub_type}" role="{doc_role}" class="{sec_class}">
    <h1 class="chapter-title">{main_heading}</h1>
{sub_block}{body_content}
  </section>
</body>
</html>
"#
    )
}

pub fn render_custom_section_xhtml(
    language: &str,
    title: &str,
    body_content: &str,
    sec_type: &str,
) -> String {
    let (epub_type, doc_role, sec_class) = get_section_epub_semantics(sec_type);
    let trimmed = body_content.trim();
    let inner_content = if trimmed.starts_with("<section") {
        trimmed.to_string()
    } else {
        format!(
            "  <section epub:type=\"{}\" role=\"{}\" class=\"{}\">\n{}\n  </section>",
            epub_type, doc_role, sec_class, body_content
        )
    };

    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" xml:lang="{language}" lang="{language}">
<head>
  <meta charset="utf-8"/>
  <title>{title}</title>
  <link rel="stylesheet" type="text/css" href="../Styles/stylesheet.css"/>
</head>
<body class="chapter-body" epub:type="{epub_type}" role="{doc_role}">
{inner_content}
</body>
</html>
"#
    )
}

pub fn render_nav(language: &str, title: &str, items: &str, landmarks: &str) -> String {
    let landmarks_section = if !landmarks.trim().is_empty() {
        format!(
            "\n  <nav epub:type=\"landmarks\" id=\"landmarks\" hidden=\"\">\n    <h2>Guía</h2>\n    <ol>\n{}\n    </ol>\n  </nav>",
            landmarks
        )
    } else {
        String::new()
    };
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" xml:lang="{language}">
<head>
  <title>Tabla de Contenidos</title>
  <link rel="stylesheet" type="text/css" href="Styles/stylesheet.css"/>
</head>
<body>
  <nav epub:type="toc" id="toc">
    <h1>{title}</h1>
    <ol>
{items}
    </ol>
  </nav>{landmarks_section}
</body>
</html>
"#
    )
}

pub fn render_colophon(
    language: &str,
    title: &str,
    author: &str,
    illustrator: &str,
    translator: &str,
    publisher: &str,
    published_year: &str,
    isbn: &str,
    copyright_notice: &str,
) -> String {
    let mut details = Vec::new();
    if !author.is_empty() {
        details.push(format!("    <p><strong>Autor:</strong> {}</p>", author));
    }
    if !illustrator.is_empty() && illustrator != "N/A" {
        details.push(format!("    <p><strong>Ilustrador:</strong> {}</p>", illustrator));
    }
    if !translator.is_empty() {
        details.push(format!("    <p><strong>Traductor:</strong> {}</p>", translator));
    }
    if !publisher.is_empty() {
        details.push(format!("    <p><strong>Editorial:</strong> {}</p>", publisher));
    }
    if !published_year.is_empty() {
        details.push(format!("    <p><strong>Año de publicación:</strong> {}</p>", published_year));
    }
    if !isbn.is_empty() {
        details.push(format!("    <p><strong>ISBN:</strong> {}</p>", isbn));
    }
    if !copyright_notice.is_empty() {
        details.push(format!("    <p class=\"copyright-notice\">{}</p>", copyright_notice));
    }
    let details_html = details.join("\n");

    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" xml:lang="{language}">
<head>
  <meta charset="utf-8" />
  <title>Página de derechos</title>
  <link rel="stylesheet" type="text/css" href="../Styles/stylesheet.css" />
</head>
<body>
  <section class="copyright-page" epub:type="copyright-page" role="doc-credit" aria-label="Página de derechos de autor">
    <h2>{title}</h2>
{details_html}
    <p class="colophon-engine">Edición digital maquetada con EditNovel.</p>
  </section>
</body>
</html>
"#
    )
}
