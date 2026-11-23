use crate::config::PresetProfile;
use crate::{
    FrontendBookMetadata, FrontendEditorialRule, FrontendGlossaryReplacement, FrontendRegexRule,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const ARCHIVE_FORMAT: &str = "editnovel";
pub const ARCHIVE_SCHEMA_VERSION: u32 = 2;

pub const SUPPORTED_PRESET_IDS: &[&str] = &[
    "visual-novel",
    "generic",
    "webnovel",
    "readme",
    "paper",
];

pub const SUPPORTED_LANGUAGE_CODES: &[&str] = &["auto", "es", "en"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalizedLabel {
    pub es: String,
    pub en: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetDescriptor {
    pub id: String,
    pub aliases: Vec<String>,
    pub profile: PresetProfile,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageDescriptor {
    pub code: String,
    pub labels: LocalizedLabel,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryAsset {
    pub lang: String,
    pub cache_name: String,
    pub br_file_name: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineManifest {
    pub schema_version: u32,
    pub engine_version: String,
    pub archive_format: String,
    pub archive_schema_version: u32,
    pub presets: Vec<PresetDescriptor>,
    pub preset_aliases: BTreeMap<String, String>,
    pub languages: Vec<LanguageDescriptor>,
    pub dictionaries: Vec<DictionaryAsset>,
    pub default_editorial_rules: Vec<FrontendEditorialRule>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditNovelV2Provenance {
    pub source_app: String,
    pub exported_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditNovelV2ValidationReport {
    pub valid: bool,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditNovelV2SourceEntry {
    pub id: String,
    pub name: String,
    pub extension: String,
    pub active: bool,
    pub section_count: usize,
    pub image_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditNovelV2Manifest {
    pub format: String,
    pub version: u32,
    pub app: String,
    pub created_at: String,
    pub timestamp: u64,
    pub created_by: String,
    pub profile: String,
    pub metadata: FrontendBookMetadata,
    pub provenance: EditNovelV2Provenance,
    pub validation_report: EditNovelV2ValidationReport,
    pub sources: Vec<EditNovelV2SourceEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditNovelV2TreeSection {
    pub id: String,
    pub file_id: String,
    pub original_title: String,
    pub title: String,
    #[serde(rename = "type")]
    pub section_type: String,
    pub order: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_page: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_page: Option<usize>,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditNovelV2TreeGroup {
    pub file_id: String,
    pub file_name: String,
    pub collapsed: bool,
    pub sections: Vec<EditNovelV2TreeSection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditNovelV2Tree {
    pub groups: Vec<EditNovelV2TreeGroup>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditNovelV2MediaItem {
    pub id: String,
    pub file_id: String,
    pub name: String,
    pub path: String,
    pub mime_type: String,
    pub resolution: String,
    pub is_discarded: bool,
    pub status: String,
    #[serde(rename = "type")]
    pub image_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alt_text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decorative: Option<bool>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditNovelV2EditorialRules {
    pub editorial_rules: Vec<FrontendEditorialRule>,
    pub regex_rules: Vec<FrontendRegexRule>,
    pub glossary: Vec<FrontendGlossaryReplacement>,
    pub spell_ignored_words: Vec<String>,
    pub spell_language_mode: String,
    pub spell_ignore_japanese: bool,
    pub spell_ignore_foreign: bool,
    pub spell_ignore_proper_nouns: bool,
    pub unify_breaks: bool,
    pub unified_break_symbol: String,
    pub unify_dialogue: bool,
    pub unified_dialogue_symbol: String,
    pub unify_quotes: bool,
    pub unified_quote_symbol: String,
    pub unify_ellipsis: bool,
    pub unified_ellipsis_symbol: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub global_spell_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spell_volume_settings: Option<BTreeMap<String, bool>>,
}

pub fn preset_aliases_for(id: &str) -> &'static [&'static str] {
    match id {
        "visual-novel" => &["en-light-novel", "es-editorial", "light-novel"],
        "generic" => &["en-fiction", "fiction"],
        "webnovel" => &["en-litrpg", "litrpg"],
        _ => &[],
    }
}

pub fn resolve_preset_id(raw: &str) -> Option<&'static str> {
    let norm = raw.trim().to_lowercase();
    for &id in SUPPORTED_PRESET_IDS {
        if norm == id || preset_aliases_for(id).contains(&norm.as_str()) {
            return Some(id);
        }
    }
    None
}

pub fn build_preset_descriptors() -> Vec<PresetDescriptor> {
    SUPPORTED_PRESET_IDS
        .iter()
        .map(|&id| PresetDescriptor {
            id: id.to_string(),
            aliases: preset_aliases_for(id)
                .iter()
                .map(|a| (*a).to_string())
                .collect(),
            profile: PresetProfile::from_id(id),
        })
        .collect()
}

pub fn build_preset_alias_map() -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for &id in SUPPORTED_PRESET_IDS {
        map.insert(id.to_string(), id.to_string());
        for &alias in preset_aliases_for(id) {
            map.insert(alias.to_string(), id.to_string());
        }
    }
    map
}

pub fn build_language_descriptors() -> Vec<LanguageDescriptor> {
    vec![
        LanguageDescriptor {
            code: "auto".to_string(),
            labels: LocalizedLabel {
                es: "Automático".to_string(),
                en: "Auto".to_string(),
            },
        },
        LanguageDescriptor {
            code: "es".to_string(),
            labels: LocalizedLabel {
                es: "Español".to_string(),
                en: "Spanish".to_string(),
            },
        },
        LanguageDescriptor {
            code: "en".to_string(),
            labels: LocalizedLabel {
                es: "Inglés".to_string(),
                en: "English".to_string(),
            },
        },
    ]
}

pub fn build_dictionary_assets() -> Vec<DictionaryAsset> {
    vec![
        DictionaryAsset {
            lang: "es".to_string(),
            cache_name: "es_frequency.txt".to_string(),
            br_file_name: "es_frequency.txt.br".to_string(),
            url: "/dicts/es_frequency.txt.br".to_string(),
        },
        DictionaryAsset {
            lang: "en".to_string(),
            cache_name: "en_frequency.txt".to_string(),
            br_file_name: "en_frequency.txt.br".to_string(),
            url: "/dicts/en_frequency.txt.br".to_string(),
        },
        DictionaryAsset {
            lang: "ja".to_string(),
            cache_name: "ja_romaji_common.tsv".to_string(),
            br_file_name: "ja_romaji_common.tsv.br".to_string(),
            url: "/dicts/ja_romaji_common.tsv.br".to_string(),
        },
    ]
}

pub fn default_editorial_rules_with_counts(
    dash_count: usize,
    quote_count: usize,
    ellipsis_count: usize,
    break_count: usize,
    header_and_spell_count: usize,
) -> Vec<FrontendEditorialRule> {
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

pub fn build_engine_manifest() -> EngineManifest {
    EngineManifest {
        schema_version: 1,
        engine_version: crate::engine_version().to_string(),
        archive_format: ARCHIVE_FORMAT.to_string(),
        archive_schema_version: ARCHIVE_SCHEMA_VERSION,
        presets: build_preset_descriptors(),
        preset_aliases: build_preset_alias_map(),
        languages: build_language_descriptors(),
        dictionaries: build_dictionary_assets(),
        default_editorial_rules: default_editorial_rules_with_counts(0, 0, 0, 0, 0),
    }
}

pub fn engine_manifest_json() -> String {
    serde_json::to_string_pretty(&build_engine_manifest()).unwrap_or_else(|_| "{}".to_string())
}

pub fn generate_typescript_contracts() -> String {
    let manifest = build_engine_manifest();
    let manifest_json =
        serde_json::to_string_pretty(&manifest).unwrap_or_else(|_| "{}".to_string());
    let preset_union = SUPPORTED_PRESET_IDS
        .iter()
        .map(|id| format!("\"{}\"", id))
        .collect::<Vec<_>>()
        .join(" | ");
    let lang_union = SUPPORTED_LANGUAGE_CODES
        .iter()
        .map(|code| format!("\"{}\"", code))
        .collect::<Vec<_>>()
        .join(" | ");

    format!(
        r#"export type PresetId = {preset_union};
export type LanguageProfile = {lang_union};
export type DictionaryLanguageCode = "es" | "en" | "ja";

export interface LocalizedLabel {{
  es: string;
  en: string;
}}

export interface PresetProfileContract {{
  id: PresetId;
  enableDropCaps: boolean;
  enableLitRpgBrackets: boolean;
  enableSceneBreaks: boolean;
  footnoteStyle: string;
  fullPageIllustrations: boolean;
}}

export interface PresetDescriptor {{
  id: PresetId;
  aliases: string[];
  profile: PresetProfileContract;
}}

export interface LanguageDescriptor {{
  code: LanguageProfile;
  labels: LocalizedLabel;
}}

export interface DictionaryAsset {{
  lang: DictionaryLanguageCode;
  cacheName: string;
  brFileName: string;
  url: string;
}}

export interface EngineEditorialRuleContract {{
  id: string;
  label: string;
  description: string;
  enabled: boolean;
  affectedCount: number;
}}

export interface EngineManifestContract {{
  schemaVersion: number;
  engineVersion: string;
  archiveFormat: "editnovel";
  archiveSchemaVersion: 2;
  presets: PresetDescriptor[];
  presetAliases: Record<string, PresetId>;
  languages: LanguageDescriptor[];
  dictionaries: DictionaryAsset[];
  defaultEditorialRules: EngineEditorialRuleContract[];
}}

export interface EngineBookMetadataContract {{
  title: string;
  author: string;
  illustrator: string;
  series: string;
  volume: string;
  language: string;
  publisher: string;
  description: string;
  isbn?: string;
  publishedYear?: string;
  copyrightNotice?: string;
  translator?: string;
}}

export interface EngineRegexRuleContract {{
  id: string;
  pattern: string;
  replacement: string;
  enabled: boolean;
  flags: string;
  affectedCount: number;
}}

export interface EngineGlossaryReplacementContract {{
  id: string;
  find: string;
  replace: string;
  enabled: boolean;
}}

export interface EditNovelV2SourceEntry {{
  id: string;
  name: string;
  extension: "pdf" | "epub";
  active: boolean;
  sectionCount: number;
  imageCount: number;
  path?: string;
}}

export interface EditNovelV2Provenance {{
  sourceApp: string;
  exportedAt: string;
}}

export interface EditNovelV2ValidationReport {{
  valid: boolean;
  warnings: string[];
  errors: string[];
}}

export interface EditNovelV2Manifest<TMetadata = EngineBookMetadataContract> {{
  format: "editnovel";
  version: 2;
  app: "editnovel";
  createdAt: string;
  timestamp: number;
  createdBy: "human" | "agent" | "hybrid";
  profile: string;
  metadata: TMetadata;
  provenance: EditNovelV2Provenance;
  validationReport: EditNovelV2ValidationReport;
  sources: EditNovelV2SourceEntry[];
}}

export interface EditNovelV2TreeSection<
  TSectionType extends string = "prologue" | "chapter" | "interlude" | "epilogue" | "afterword" | "cover" | "color",
> {{
  id: string;
  fileId: string;
  originalTitle: string;
  title: string;
  type: TSectionType;
  order: number;
  startPage?: number;
  endPage?: number;
  path: string;
}}

export interface EditNovelV2TreeGroup<
  TSectionType extends string = "prologue" | "chapter" | "interlude" | "epilogue" | "afterword" | "cover" | "color",
> {{
  fileId: string;
  fileName: string;
  collapsed: boolean;
  sections: EditNovelV2TreeSection<TSectionType>[];
}}

export interface EditNovelV2Tree<
  TSectionType extends string = "prologue" | "chapter" | "interlude" | "epilogue" | "afterword" | "cover" | "color",
> {{
  groups: EditNovelV2TreeGroup<TSectionType>[];
}}

export interface EditNovelV2MediaItem<
  TImageType extends string = "cover" | "color" | "bw" | "tiled",
> {{
  id: string;
  fileId: string;
  name: string;
  path: string;
  mimeType: string;
  resolution: string;
  isDiscarded: boolean;
  status: "keep" | "discard" | "uncertain";
  type: TImageType;
  reason?: string;
  altText?: string;
  decorative?: boolean;
}}

export interface EditNovelV2EditorialRules {{
  editorialRules: EngineEditorialRuleContract[];
  regexRules: EngineRegexRuleContract[];
  glossary: EngineGlossaryReplacementContract[];
  spellIgnoredWords: string[];
  spellLanguageMode: "hybrid" | "es" | "en";
  spellIgnoreJapanese: boolean;
  spellIgnoreForeign: boolean;
  spellIgnoreProperNouns: boolean;
  unifyBreaks: boolean;
  unifiedBreakSymbol: string;
  unifyDialogue: boolean;
  unifiedDialogueSymbol: string;
  unifyQuotes: boolean;
  unifiedQuoteSymbol: string;
  unifyEllipsis: boolean;
  unifiedEllipsisSymbol: string;
  globalSpellEnabled?: boolean;
  spellVolumeSettings?: Record<string, boolean>;
}}

export const ENGINE_MANIFEST: EngineManifestContract = {manifest_json};
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_manifest_and_aliases() {
        let json_str = engine_manifest_json();
        let manifest: EngineManifest =
            serde_json::from_str(&json_str).expect("manifest json should parse");

        assert_eq!(manifest.archive_format, "editnovel");
        assert_eq!(manifest.archive_schema_version, 2);
        assert_eq!(manifest.presets.len(), SUPPORTED_PRESET_IDS.len());
        assert_eq!(manifest.languages.len(), SUPPORTED_LANGUAGE_CODES.len());
        assert_eq!(manifest.dictionaries.len(), 3);
        assert_eq!(manifest.default_editorial_rules.len(), 5);
        assert_eq!(resolve_preset_id("en-light-novel"), Some("visual-novel"));
        assert_eq!(resolve_preset_id("es-editorial"), Some("visual-novel"));
        assert_eq!(resolve_preset_id("en-litrpg"), Some("webnovel"));
        assert_eq!(resolve_preset_id("en-fiction"), Some("generic"));
        assert_eq!(resolve_preset_id("unknown-preset"), None);
    }

    #[test]
    fn test_generate_typescript_contracts_contains_core_exports() {
        let ts = generate_typescript_contracts();
        assert!(ts.contains("export type PresetId ="));
        assert!(ts.contains("export type LanguageProfile ="));
        assert!(ts.contains("export interface EditNovelV2Manifest"));
        assert!(ts.contains("export const ENGINE_MANIFEST: EngineManifestContract ="));
    }
}
