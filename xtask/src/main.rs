mod check_duplicates;
mod fix_placeholders;

use std::path::Path;
use std::{env, fs, process};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() > 3 {
        eprintln!("usage: xtask <check-duplicates|fix> [dir]");
        process::exit(1);
    }
    let cmd = args.get(1).map(|s| s.as_str());
    let dir = args.get(2).map_or(".", |s| s.as_str());

    match cmd {
        Some("check-duplicates") => check_duplicates::check_duplicates(dir),
        Some("fix") => fix_placeholders::fix_placeholders(dir),
        _ => {
            eprintln!("usage: xtask <check-duplicates|fix> [dir]");
            process::exit(1);
        }
    }
}

fn find_rs_files(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    let entries = fs::read_dir(dir).unwrap_or_else(|e| {
        eprintln!("error reading directory {}: {e}", dir.display());
        process::exit(1);
    });
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let excluded_paths = path.file_name().is_some_and(|n| {
                n.to_str()
                    .is_some_and(|s| s.starts_with('.') || s == "target")
            });
            if excluded_paths {
                continue;
            }
            files.extend(find_rs_files(&path));
        } else if path.extension().is_some_and(|e| e == "rs") {
            files.push(path);
        }
    }
    files
}

/// Extract IDs from message_id fields and erased_error macro calls.
/// Scans the whole file content to handle multi-line erased_error! invocations.
fn extract_message_ids(content: &str) -> Vec<(usize, &str)> {
    let mut results = Vec::new();

    for needle in ["message_id = \"", "message_id=\""] {
        let mut start = 0;
        while let Some(idx) = content[start..].find(needle) {
            let abs = start + idx;
            let after = abs + needle.len();
            let Some(end) = content[after..].find('"') else {
                break;
            };
            let line = content[..abs].bytes().filter(|&b| b == b'\n').count() + 1;
            results.push((line, &content[after..after + end]));
            start = after + end + 1;
        }
    }

    // erased_error!( possibly whitespace/newlines then "..."
    {
        let needle = "erased_error!(";
        let mut start = 0;
        while let Some(idx) = content[start..].find(needle) {
            let abs = start + idx;
            let after = abs + needle.len();
            let rest = &content[after..];
            let trimmed = rest.trim_start();
            if let Some(stripped) = trimmed.strip_prefix('"') {
                let Some(end) = stripped.find('"') else {
                    start = after;
                    continue;
                };
                let quote_abs = content.len() - trimmed.len() + 1;
                let line = content[..quote_abs].bytes().filter(|&b| b == b'\n').count() + 1;
                results.push((line, &stripped[..end]));
                start = quote_abs + end + 1;
            } else {
                start = after;
            }
        }
    }

    // erased_ensure!(condition, "id", ...) — ID is the second arg after the condition
    {
        let needle = "erased_ensure!(";
        let mut start = 0;
        while let Some(idx) = content[start..].find(needle) {
            let abs = start + idx;
            let after = abs + needle.len();
            if let Some(id) = skip_first_arg_then_string(&content[after..]) {
                let id_abs = content.len() - content[after..].len() + id.0;
                let line = content[..id_abs].bytes().filter(|&b| b == b'\n').count() + 1;
                results.push((line, id.1));
                start = id_abs + id.1.len() + 1;
            } else {
                start = after;
            }
        }
    }

    results
}

/// Skip the first macro argument (tracking paren depth), then read the next quoted string.
/// Returns (byte offset of the opening quote relative to input, the ID slice).
fn skip_first_arg_then_string(s: &str) -> Option<(usize, &str)> {
    let mut depth = 0u32;
    let mut chars = s.char_indices();
    // Find the first top-level comma (skip nested parens/brackets)
    let comma_pos = loop {
        let (i, c) = chars.next()?;
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => {
                if depth == 0 {
                    return None;
                }
                depth -= 1;
            }
            ',' if depth == 0 => break i,
            _ => {}
        }
    };
    let after_comma = &s[comma_pos + 1..];
    let trimmed = after_comma.trim_start();
    let stripped = trimmed.strip_prefix('"')?;
    let end = stripped.find('"')?;
    let quote_offset = comma_pos + 1 + (after_comma.len() - trimmed.len());
    Some((quote_offset + 1, &stripped[..end]))
}
