use std::path::Path;

pub fn process_transactions(filename: impl AsRef<Path>) -> anyhow::Result<String> {

    eprintln!("Processing file {}", filename.as_ref().display());

    Ok("".to_string())
}