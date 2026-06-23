use rand::RngExt;
use std::path::Path;
use std::{fs, process};

pub fn fix_placeholders(dir: &str) {
    let files = crate::find_rs_files(Path::new(dir));

    let mut rng = rand::rng();
    let chars: Vec<char> = ('a'..='z').chain('A'..='Z').chain('0'..='9').collect();
    let mut count = 0;

    for path in &files {
        let Ok(content) = fs::read_to_string(path) else {
            continue;
        };

        let mut result = String::with_capacity(content.len());
        let mut rest = content.as_str();
        let mut modified = false;
        let needle = format!("message_id = {}", "()");

        while let Some(idx) = rest.find(needle.as_str()) {
            modified = true;
            result.push_str(&rest[..idx]);
            let id: String = (0..8)
                .map(|_| chars[rng.random_range(0..chars.len())])
                .collect();
            result.push_str(&format!("message_id = \"{id}\""));
            rest = &rest[idx + needle.len()..];
            count += 1;
        }
        result.push_str(rest);

        if modified {
            fs::write(path, result).unwrap_or_else(|e| {
                eprintln!("error writing {}: {e}", path.display());
                process::exit(1);
            });
        }
    }

    println!("fixed {count} placeholder(s)");
}
