use editnovel_engine::contracts::{engine_manifest_json, generate_typescript_contracts};
use std::env;
use std::fs;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let mut manifest_out: Option<PathBuf> = None;
    let mut ts_out: Option<PathBuf> = None;

    let mut i = 1usize;
    while i < args.len() {
        match args[i].as_str() {
            "--manifest-out" if i + 1 < args.len() => {
                manifest_out = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--ts-out" if i + 1 < args.len() => {
                ts_out = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            _ => {
                i += 1;
            }
        }
    }

    if let Some(path) = manifest_out {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, format!("{}\n", engine_manifest_json()))?;
    }

    if let Some(path) = ts_out {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, generate_typescript_contracts())?;
    }

    Ok(())
}
