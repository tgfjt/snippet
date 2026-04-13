use std::fs;
use std::path::PathBuf;

use crate::snippet::Snippet;

pub fn snippets_path() -> PathBuf {
    if let Ok(path) = std::env::var("SNIPPET_FILE") {
        PathBuf::from(path)
    } else {
        dirs::home_dir()
            .expect("ホームディレクトリが見つかりません")
            .join(".snippets.yml")
    }
}

pub fn load_snippets() -> Result<Vec<Snippet>, String> {
    let path = snippets_path();
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(&path)
        .map_err(|e| format!("ファイルの読み込みに失敗: {}: {}", path.display(), e))?;
    if content.trim().is_empty() {
        return Ok(Vec::new());
    }
    let snippets: Vec<Snippet> =
        serde_yaml::from_str(&content).map_err(|e| format!("YAMLのパースに失敗: {}", e))?;
    Ok(snippets)
}

/// ファイル末尾に生テキストで追記する（既存コメントを保持するため）
pub fn append_snippet(snippet: &Snippet) -> Result<(), String> {
    let path = snippets_path();
    let mut content = if path.exists() {
        fs::read_to_string(&path).map_err(|e| format!("ファイルの読み込みに失敗: {}", e))?
    } else {
        String::new()
    };

    // 末尾の改行を整える
    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }
    if !content.is_empty() {
        content.push('\n');
    }

    content.push_str(&format!("- name: {}\n", yaml_scalar(&snippet.name)));
    content.push_str(&format!(
        "  description: {}\n",
        yaml_scalar(&snippet.description)
    ));

    if let Some(ref cmd) = snippet.command {
        content.push_str(&format!("  command: {}\n", yaml_scalar(cmd)));
    }
    if let Some(ref tags) = snippet.tags {
        if !tags.is_empty() {
            let items: Vec<String> = tags.iter().map(|t| yaml_scalar(t)).collect();
            content.push_str(&format!("  tags: [{}]\n", items.join(", ")));
        }
    }
    if let Some(ref body) = snippet.body {
        content.push_str("  body: |\n");
        for line in body.lines() {
            if line.is_empty() {
                content.push('\n');
            } else {
                content.push_str(&format!("    {}\n", line));
            }
        }
    }

    fs::write(&path, content).map_err(|e| format!("ファイルの書き込みに失敗: {}", e))?;
    Ok(())
}

/// YAML スカラー値として安全にフォーマットする
fn yaml_scalar(s: &str) -> String {
    if s.is_empty()
        || s.contains(':')
        || s.contains('#')
        || s.contains('{')
        || s.contains('}')
        || s.contains('[')
        || s.contains(']')
        || s.contains('\'')
        || s.contains('"')
        || s.contains('\n')
        || s.starts_with(' ')
        || s.ends_with(' ')
        || s.starts_with('*')
        || s.starts_with('&')
        || s.starts_with('!')
        || s.starts_with('|')
        || s.starts_with('>')
        || s.starts_with('%')
        || s.starts_with('@')
        || s.starts_with('`')
        || s.starts_with(',')
        || s.starts_with('?')
        || s == "true"
        || s == "false"
        || s == "null"
        || s == "yes"
        || s == "no"
    {
        format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        s.to_string()
    }
}
