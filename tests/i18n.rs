//! Verifies `locales/*.yml`, generated TypeScript declarations in
//! `src/i18n/translations.generated.ts`, and `t(...)` usage across `App.tsx`
//! and `src/`. Pure stdlib so `cargo test` and `nsh test` run it with no
//! extra dependencies.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

fn strip_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut quote: Option<u8> = None;
    let mut escaped = false;
    for (i, &b) in bytes.iter().enumerate() {
        if escaped {
            escaped = false;
            continue;
        }
        if quote == Some(b'"') && b == b'\\' {
            escaped = true;
            continue;
        }
        if let Some(q) = quote {
            if b == q {
                quote = None;
            }
            continue;
        }
        if b == b'"' || b == b'\'' {
            quote = Some(b);
            continue;
        }
        if b == b'#' && (i == 0 || bytes[i - 1].is_ascii_whitespace()) {
            return &line[..i];
        }
    }
    line
}

fn find_unquoted_colon(line: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    let mut quote: Option<u8> = None;
    let mut escaped = false;
    for (i, &b) in bytes.iter().enumerate() {
        if escaped {
            escaped = false;
            continue;
        }
        if quote == Some(b'"') && b == b'\\' {
            escaped = true;
            continue;
        }
        if let Some(q) = quote {
            if b == q {
                quote = None;
            }
            continue;
        }
        if b == b'"' || b == b'\'' {
            quote = Some(b);
            continue;
        }
        if b == b':' && (i + 1 == bytes.len() || bytes[i + 1].is_ascii_whitespace()) {
            return Some(i);
        }
    }
    None
}

fn unquote_scalar(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.len() >= 2 && trimmed.starts_with('"') && trimmed.ends_with('"') {
        let inner = &trimmed[1..trimmed.len() - 1];
        let mut out = String::with_capacity(inner.len());
        let mut chars = inner.chars();
        while let Some(c) = chars.next() {
            if c == '\\' {
                match chars.next() {
                    Some('n') => out.push('\n'),
                    Some('t') => out.push('\t'),
                    Some('"') => out.push('"'),
                    Some('\\') => out.push('\\'),
                    Some(other) => {
                        out.push('\\');
                        out.push(other);
                    }
                    None => out.push('\\'),
                }
            } else {
                out.push(c);
            }
        }
        return out;
    }
    if trimmed.len() >= 2 && trimmed.starts_with('\'') && trimmed.ends_with('\'') {
        return trimmed[1..trimmed.len() - 1].replace("''", "'");
    }
    trimmed.to_string()
}

fn parse_yaml_locale(path: &Path) -> BTreeMap<String, String> {
    let content = fs::read_to_string(path).unwrap();
    let mut stack: Vec<(isize, Vec<String>)> = vec![(-1, Vec::new())];
    let mut entries = BTreeMap::new();

    for (line_idx, raw) in content.lines().enumerate() {
        let no_comment = strip_comment(raw);
        let trimmed = no_comment.trim();
        if trimmed.is_empty() || trimmed == "---" {
            continue;
        }
        assert!(!raw.contains('\t'), "{}:{}: tab indentation unsupported", path.display(), line_idx + 1);
        let indent = no_comment.bytes().take_while(|&b| b == b' ').count() as isize;
        let part = &no_comment[indent as usize..];
        let colon = find_unquoted_colon(part)
            .unwrap_or_else(|| panic!("{}:{}: expected key: value", path.display(), line_idx + 1));
        let key = unquote_scalar(&part[..colon]);
        let rest = part[colon + 1..].trim();

        while stack.len() > 1 && indent <= stack.last().unwrap().0 {
            stack.pop();
        }
        let mut prefix = stack.last().unwrap().1.clone();
        prefix.push(key);
        if rest.is_empty() {
            stack.push((indent, prefix));
        } else {
            let joined = prefix.join(".");
            assert!(
                entries.insert(joined.clone(), unquote_scalar(rest)).is_none(),
                "{}:{}: duplicate key {joined:?}",
                path.display(),
                line_idx + 1
            );
        }
    }
    entries
}

fn extract_params(text: &str) -> BTreeSet<String> {
    let mut params = BTreeSet::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i + 2 < bytes.len() {
        if bytes[i] == b'%' && bytes[i + 1] == b'{' {
            let start = i + 2;
            let mut end = start;
            while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
                end += 1;
            }
            if end > start && end < bytes.len() && bytes[end] == b'}' {
                params.insert(text[start..end].to_string());
                i = end + 1;
                continue;
            }
        }
        i += 1;
    }
    params
}

fn collect_ts_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_ts_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "ts" || e == "tsx") {
            out.push(path);
        }
    }
}

fn extract_quoted_literals(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let q = bytes[i];
        if q == b'"' || q == b'\'' {
            let start = i + 1;
            let mut j = start;
            let mut closed = false;
            while j < bytes.len() {
                if bytes[j] == b'\n' || bytes[j] == b'\r' {
                    break;
                }
                if bytes[j] == b'\\' {
                    j += 2;
                    continue;
                }
                if bytes[j] == q {
                    closed = true;
                    break;
                }
                j += 1;
            }
            if closed {
                if let Ok(s) = std::str::from_utf8(&bytes[start..j]) {
                    let looks_like_key = s.contains('.')
                        && s.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'));
                    if looks_like_key || q == b'"' {
                        out.push(s.to_string());
                        i = j + 1;
                        continue;
                    }
                }
            }
            i += 1;
        } else {
            i += 1;
        }
    }
    out
}

fn extract_template_prefixes(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(idx) = rest.find("t(`") {
        let after = &rest[idx + 3..];
        if let Some(interp) = after.find("${") {
            let prefix = &after[..interp];
            if !prefix.contains('`') && !prefix.contains('\n') {
                out.push(prefix.to_string());
            }
        }
        rest = &rest[idx + 3..];
    }
    out
}

#[test]
fn locales_generated_types_and_usages_are_consistent() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let en = parse_yaml_locale(&root.join("locales/en.yml"));
    let zh = parse_yaml_locale(&root.join("locales/zh.yml"));
    assert!(!en.is_empty(), "locales/en.yml must not be empty");

    for (key, value) in &en {
        assert!(!value.trim().is_empty(), "en key {key:?} has empty value");
        let zh_val = zh.get(key).unwrap_or_else(|| panic!("zh missing key {key:?}"));
        assert!(!zh_val.trim().is_empty(), "zh key {key:?} has empty value");
        assert_eq!(
            extract_params(value),
            extract_params(zh_val),
            "placeholder mismatch for key {key:?}"
        );
    }
    for key in zh.keys() {
        assert!(en.contains_key(key), "zh has extra key {key:?} missing from en");
    }

    let generated = fs::read_to_string(root.join("src/i18n/translations.generated.ts")).unwrap();
    for (key, value) in &en {
        let line_prefix = format!("\"{key}\":");
        assert!(
            generated.contains(&line_prefix),
            "translations.generated.ts is missing key {key:?}"
        );
        for param in extract_params(value) {
            let needle = format!("{param}: string | number");
            let line = generated
                .lines()
                .find(|l| l.trim_start().starts_with(&line_prefix))
                .unwrap();
            assert!(
                line.contains(&needle),
                "translations.generated.ts line for {key:?} missing param {param:?}"
            );
        }
    }

    let mut files = vec![root.join("App.tsx")];
    collect_ts_files(&root.join("src"), &mut files);
    let wrapper = root.join("src/i18n/index.ts");
    let generated_path = root.join("src/i18n/translations.generated.ts");

    let mut used_literals = BTreeSet::new();
    let mut template_prefixes = BTreeSet::new();

    for file in files {
        if file == generated_path {
            continue;
        }
        let name = file.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if name.ends_with(".test.ts") || name.ends_with(".test.tsx") {
            continue;
        }
        let text = fs::read_to_string(&file).unwrap();
        if file != wrapper {
            assert!(
                !text.contains("\"nsh/i18n\"") && !text.contains("'nsh/i18n'"),
                "{}: import from src/i18n instead of nsh/i18n",
                file.display()
            );
        }
        for lit in extract_quoted_literals(&text) {
            used_literals.insert(lit);
        }
        for prefix in extract_template_prefixes(&text) {
            assert!(
                en.keys().any(|k| k.starts_with(&prefix)),
                "{}: template prefix {prefix:?} matches no translation key",
                file.display()
            );
            template_prefixes.insert(prefix);
        }
    }

    for key in en.keys() {
        let is_used = used_literals.contains(key) || template_prefixes.iter().any(|p| key.starts_with(p));
        assert!(is_used, "unused translation key {key:?} in locales/");
    }
}
