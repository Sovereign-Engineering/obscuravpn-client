use std::collections::HashMap;
use std::path::Path;
use std::{fs, process};

pub fn check_duplicates(dir: &str) {
    let mut seen: HashMap<String, Vec<String>> = HashMap::new();

    for path in crate::find_rs_files(Path::new(dir)) {
        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };
        for (line, id) in crate::extract_message_ids(&content) {
            seen.entry(id.to_string())
                .or_default()
                .push(format!("{}:{line}", path.display()));
        }
    }

    let mut duplicates: Vec<_> = seen.iter().filter(|(_, locs)| locs.len() > 1).collect();
    duplicates.sort_unstable();

    if duplicates.is_empty() {
        return;
    }

    eprintln!("DUPLICATE message_ids:");
    for (id, locations) in duplicates {
        eprintln!("  {id}");
        let mut locations = locations.clone();
        locations.sort_unstable();
        for loc in &locations {
            eprintln!("    {loc}");
        }
    }
    process::exit(1);
}
