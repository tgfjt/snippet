use std::fs;
use std::path::{Path, PathBuf};

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
    load_snippets_from(&snippets_path())
}

pub fn load_snippets_from(path: &Path) -> Result<Vec<Snippet>, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(path)
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
    append_snippet_to(snippet, &snippets_path())
}

pub fn append_snippet_to(snippet: &Snippet, path: &Path) -> Result<(), String> {
    let mut content = if path.exists() {
        fs::read_to_string(path).map_err(|e| format!("ファイルの読み込みに失敗: {}", e))?
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
    if !snippet.tags.is_empty() {
        let items: Vec<String> = snippet.tags.iter().map(|t| yaml_scalar(t)).collect();
        content.push_str(&format!("  tags: [{}]\n", items.join(", ")));
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

    fs::write(path, content).map_err(|e| format!("ファイルの書き込みに失敗: {}", e))?;
    Ok(())
}

/// YAML スカラー値として安全にフォーマットする
pub(crate) fn yaml_scalar(s: &str) -> String {
    if needs_quoting(s) {
        format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        s.to_string()
    }
}

fn needs_quoting(s: &str) -> bool {
    s.is_empty()
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
        || matches!(s, "true" | "false" | "null" | "yes" | "no")
        || looks_numeric(s)
}

/// YAML パーサが数値として解釈しうる文字列を検出する
fn looks_numeric(s: &str) -> bool {
    // 整数、浮動小数点、八進、十六進、無限大、NaN
    s.parse::<f64>().is_ok()
        || s.starts_with("0x")
        || s.starts_with("0o")
        || s.eq_ignore_ascii_case(".inf")
        || s.eq_ignore_ascii_case("-.inf")
        || s.eq_ignore_ascii_case(".nan")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::NamedTempFile;

    // ─── yaml_scalar 契約テスト ──────────────────────────

    #[test]
    fn 普通の文字列はそのまま返す() {
        // Arrange
        let input = "hello";

        // Act
        let result = yaml_scalar(input);

        // Assert
        assert_eq!(result, "hello");
    }

    #[test]
    fn コロンを含む文字列はクォートされる() {
        // Arrange
        let input = "key: value";

        // Act
        let result = yaml_scalar(input);

        // Assert
        assert_eq!(result, "\"key: value\"");
    }

    #[test]
    fn 波括弧を含むコマンドはクォートされる() {
        // Arrange
        let input = "echo {{name}}";

        // Act
        let result = yaml_scalar(input);

        // Assert
        assert!(result.starts_with('"'));
        assert!(result.ends_with('"'));
    }

    #[test]
    fn yaml_boolean_リテラルはクォートされる() {
        // Arrange / Act / Assert
        for word in &["true", "false", "yes", "no", "null"] {
            let result = yaml_scalar(word);
            assert_eq!(
                result,
                format!("\"{}\"", word),
                "{} がクォートされていない",
                word
            );
        }
    }

    #[test]
    fn 空文字列はクォートされる() {
        // Arrange
        let input = "";

        // Act
        let result = yaml_scalar(input);

        // Assert
        assert_eq!(result, "\"\"");
    }

    #[test]
    fn ダブルクォートを含む文字列はエスケープされる() {
        // Arrange
        let input = r#"say "hello""#;

        // Act
        let result = yaml_scalar(input);

        // Assert
        assert_eq!(result, r#""say \"hello\"""#);
    }

    #[test]
    fn 数値文字列はクォートされる() {
        // Arrange / Act / Assert
        for s in &["123", "3.14", "0xFF", "0o77", ".inf", ".nan"] {
            let result = yaml_scalar(s);
            assert!(
                result.starts_with('"'),
                "{} がクォートされていない: {}",
                s,
                result
            );
        }
    }

    // ─── 往復テスト ─────────────────────────────────────

    #[test]
    fn 最小構成のスニペットが往復する() {
        // Arrange
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path();
        let snippet = Snippet::minimal("memo", "ただのメモ");

        // Act
        append_snippet_to(&snippet, path).unwrap();
        let loaded = load_snippets_from(path).unwrap();

        // Assert
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "memo");
        assert_eq!(loaded[0].description, "ただのメモ");
        assert!(loaded[0].command.is_none());
        assert!(loaded[0].tags.is_empty());
        assert!(loaded[0].body.is_none());
    }

    #[test]
    fn 全フィールドありのスニペットが往復する() {
        // Arrange
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path();
        let snippet = Snippet::full(
            "loudnorm",
            "音量ノーマライズ",
            Some("ffmpeg -i {{input}} -af loudnorm {{output}}"),
            &["ffmpeg", "audio"],
            Some("EBU R128準拠\nTP=-1で制限"),
        );

        // Act
        append_snippet_to(&snippet, path).unwrap();
        let loaded = load_snippets_from(path).unwrap();

        // Assert
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "loudnorm");
        assert_eq!(
            loaded[0].command.as_deref().unwrap(),
            "ffmpeg -i {{input}} -af loudnorm {{output}}"
        );
        assert_eq!(loaded[0].tags, vec!["ffmpeg", "audio"]);
        assert!(loaded[0].body.as_ref().unwrap().contains("EBU R128準拠"));
    }

    #[test]
    fn 複数エントリを追記しても全部読み戻せる() {
        // Arrange
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path();
        let s1 = Snippet::minimal("first", "1番目");
        let s2 = Snippet::full("second", "2番目", Some("echo hello"), &["test"], None);
        let s3 = Snippet::minimal("third", "3番目");

        // Act
        append_snippet_to(&s1, path).unwrap();
        append_snippet_to(&s2, path).unwrap();
        append_snippet_to(&s3, path).unwrap();
        let loaded = load_snippets_from(path).unwrap();

        // Assert
        assert_eq!(loaded.len(), 3);
        assert_eq!(loaded[0].name, "first");
        assert_eq!(loaded[1].name, "second");
        assert_eq!(loaded[2].name, "third");
    }

    // ─── ファイル操作テスト ─────────────────────────────

    #[test]
    fn 存在しないファイルからは空のリストが返る() {
        // Arrange
        let path = Path::new("/tmp/snipl_test_nonexistent_12345.yml");

        // Act
        let result = load_snippets_from(path).unwrap();

        // Assert
        assert!(result.is_empty());
    }

    #[test]
    fn 空ファイルからは空のリストが返る() {
        // Arrange
        let tmp = NamedTempFile::new().unwrap();
        fs::write(tmp.path(), "").unwrap();

        // Act
        let result = load_snippets_from(tmp.path()).unwrap();

        // Assert
        assert!(result.is_empty());
    }

    #[test]
    fn 追記しても既存エントリが壊れない() {
        // Arrange
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path();
        let existing = Snippet::full("original", "元のエントリ", Some("echo hi"), &["test"], None);
        append_snippet_to(&existing, path).unwrap();
        let before = load_snippets_from(path).unwrap();

        // Act
        let new_entry = Snippet::minimal("added", "追加エントリ");
        append_snippet_to(&new_entry, path).unwrap();
        let after = load_snippets_from(path).unwrap();

        // Assert
        assert_eq!(after.len(), 2);
        assert_eq!(after[0].name, before[0].name);
        assert_eq!(after[0].description, before[0].description);
        assert_eq!(after[0].command, before[0].command);
    }

    #[test]
    fn 追記してもファイル内の既存コメントが残る() {
        // Arrange
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path();
        let initial_content =
            "# このファイルは手動で編集できます\n- name: first\n  description: 最初のエントリ\n";
        fs::write(path, initial_content).unwrap();

        // Act
        let snippet = Snippet::minimal("second", "追加分");
        append_snippet_to(&snippet, path).unwrap();
        let raw = fs::read_to_string(path).unwrap();

        // Assert
        assert!(raw.contains("# このファイルは手動で編集できます"));
        let loaded = load_snippets_from(path).unwrap();
        assert_eq!(loaded.len(), 2);
    }
}
