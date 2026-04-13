use std::collections::HashMap;
use std::io::{self, BufRead};
use std::process::Command;

use dialoguer::{Confirm, Input};

use crate::snippet::Snippet;
use crate::store;

// ─── search ──────────────────────────────────────────────

pub fn filter_snippets<'a>(snippets: &'a [Snippet], query: Option<&str>) -> Vec<&'a Snippet> {
    match query {
        Some(q) => {
            let q = q.to_lowercase();
            snippets
                .iter()
                .filter(|s| {
                    s.name.to_lowercase().contains(&q)
                        || s.description.to_lowercase().contains(&q)
                        || s.tags.iter().any(|t| t.to_lowercase().contains(&q))
                        || s.command
                            .as_ref()
                            .is_some_and(|c| c.to_lowercase().contains(&q))
                        || s.body
                            .as_ref()
                            .is_some_and(|b| b.to_lowercase().contains(&q))
                })
                .collect()
        }
        None => snippets.iter().collect(),
    }
}

pub fn search(query: Option<&str>, full: bool) -> Result<(), String> {
    let snippets = store::load_snippets()?;

    let matches = filter_snippets(&snippets, query);

    if matches.is_empty() {
        if query.is_some() {
            eprintln!("マッチするエントリが見つかりません");
        } else {
            eprintln!("エントリがありません");
        }
        return Ok(());
    }

    for s in &matches {
        let tags_str = if s.tags.is_empty() {
            String::new()
        } else {
            format!(" [{}]", s.tags.join(", "))
        };
        println!("{} \u{2014} {}{}", s.name, s.description, tags_str);

        if full {
            if let Some(ref cmd) = s.command {
                println!("  command: {}", cmd);
            }
            if let Some(ref body) = s.body {
                for line in body.lines() {
                    println!("  {}", line);
                }
            }
            println!();
        }
    }

    Ok(())
}

// ─── get ─────────────────────────────────────────────────

pub fn get(name: &str) -> Result<(), String> {
    let snippets = store::load_snippets()?;

    let s = snippets
        .iter()
        .find(|s| s.name == name)
        .ok_or_else(|| format!("エントリが見つかりません: {}", name))?;

    let yaml = serde_yaml::to_string(&vec![s]).map_err(|e| format!("YAML出力に失敗: {}", e))?;
    print!("{}", yaml);
    Ok(())
}

// ─── add ─────────────────────────────────────────────────

pub fn add() -> Result<(), String> {
    let snippets = store::load_snippets()?;

    let name: String = Input::new()
        .with_prompt("name")
        .interact_text()
        .map_err(|e| format!("入力エラー: {}", e))?;

    if snippets.iter().any(|s| s.name == name) {
        return Err(format!("name が重複しています: {}", name));
    }

    let description: String = Input::new()
        .with_prompt("description")
        .interact_text()
        .map_err(|e| format!("入力エラー: {}", e))?;

    let command_input: String = Input::new()
        .with_prompt("command (空エンターでスキップ)")
        .allow_empty(true)
        .interact_text()
        .map_err(|e| format!("入力エラー: {}", e))?;
    let command = if command_input.is_empty() {
        None
    } else {
        Some(command_input)
    };

    let tags_input: String = Input::new()
        .with_prompt("tags (カンマ区切り、空エンターでスキップ)")
        .allow_empty(true)
        .interact_text()
        .map_err(|e| format!("入力エラー: {}", e))?;
    let tags: Vec<String> = tags_input
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    // body: 複数行入力。空行2つで終了、最初の空エンターでスキップ
    eprintln!("body (空エンターでスキップ、複数行は空行2つで終了):");
    let body = read_body()?;

    let snippet = Snippet {
        name: name.clone(),
        description,
        command,
        tags,
        body,
    };

    store::append_snippet(&snippet)?;
    eprintln!("保存しました: {}", name);
    Ok(())
}

fn read_body() -> Result<Option<String>, String> {
    let stdin = io::stdin();
    let mut lines = Vec::new();
    let mut empty_count = 0;

    for line in stdin.lock().lines() {
        let line = line.map_err(|e| format!("入力エラー: {}", e))?;
        if line.is_empty() {
            empty_count += 1;
            if lines.is_empty() {
                // 最初の空エンター → スキップ
                return Ok(None);
            }
            if empty_count >= 2 {
                break;
            }
            lines.push(String::new());
        } else {
            empty_count = 0;
            lines.push(line);
        }
    }

    // 末尾の空行を除去
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }

    if lines.is_empty() {
        Ok(None)
    } else {
        Ok(Some(lines.join("\n")))
    }
}

// ─── run ─────────────────────────────────────────────────

pub fn run(name: &str, args: Vec<String>) -> Result<(), String> {
    let snippets = store::load_snippets()?;

    let snippet = snippets
        .iter()
        .find(|s| s.name == name)
        .ok_or_else(|| format!("エントリが見つかりません: {}", name))?;

    let command = match &snippet.command {
        Some(cmd) => cmd.clone(),
        None => {
            // command がなければ body を表示するだけ
            if let Some(ref body) = snippet.body {
                println!("{}", body);
            } else {
                println!("{} \u{2014} {}", snippet.name, snippet.description);
            }
            return Ok(());
        }
    };

    // key=value 引数をパース
    let mut arg_map: HashMap<String, String> = HashMap::new();
    for arg in &args {
        if let Some((key, value)) = arg.split_once('=') {
            arg_map.insert(key.to_string(), value.to_string());
        }
    }

    // プレースホルダを置換
    let mut result = command.clone();

    for ph in parse_placeholders(&command) {
        let value = if let Some(v) = arg_map.get(&ph.name) {
            v.clone()
        } else if let Some(ref d) = ph.default {
            d.clone()
        } else {
            // 対話的に聞く
            Input::new()
                .with_prompt(&ph.name)
                .interact_text()
                .map_err(|e| format!("入力エラー: {}", e))?
        };

        result = result.replace(&ph.raw, &value);
    }

    println!("$ {}", result);

    let confirm = Confirm::new()
        .with_prompt("実行しますか?")
        .default(true)
        .interact()
        .map_err(|e| format!("入力エラー: {}", e))?;

    if !confirm {
        eprintln!("キャンセルしました");
        return Ok(());
    }

    // stdout/stderr をそのまま端末に流す
    let status = Command::new("sh")
        .arg("-c")
        .arg(&result)
        .status()
        .map_err(|e| format!("コマンド実行に失敗: {}", e))?;

    if !status.success() {
        return Err(format!(
            "コマンドが終了コード {} で失敗",
            status.code().unwrap_or(-1)
        ));
    }

    Ok(())
}

#[derive(Debug, PartialEq)]
struct Placeholder {
    name: String,
    default: Option<String>,
    raw: String, // 元の "{{name}}" や "{{name:default}}" をそのまま保持
}

/// `{{name}}` や `{{name:default}}` を抽出する（出現順、重複なし）
fn parse_placeholders(command: &str) -> Vec<Placeholder> {
    let mut result = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let bytes = command.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i + 3 < len {
        if bytes[i] == b'{' && bytes[i + 1] == b'{' {
            if let Some(end) = command[i + 2..].find("}}") {
                let raw = &command[i..i + 2 + end + 2];
                let inner = &command[i + 2..i + 2 + end];
                if !inner.is_empty() {
                    let (name, default) = match inner.split_once(':') {
                        Some((n, d)) => (n.to_string(), Some(d.to_string())),
                        None => (inner.to_string(), None),
                    };
                    if seen.insert(raw.to_string()) {
                        result.push(Placeholder {
                            name,
                            default,
                            raw: raw.to_string(),
                        });
                    }
                }
                i = i + 2 + end + 2;
                continue;
            }
        }
        i += 1;
    }

    result
}

// ─── tags ────────────────────────────────────────────────

pub fn tags() -> Result<(), String> {
    let snippets = store::load_snippets()?;

    let mut counts: HashMap<String, usize> = HashMap::new();
    for s in &snippets {
        for tag in &s.tags {
            *counts.entry(tag.clone()).or_insert(0) += 1;
        }
    }

    if counts.is_empty() {
        eprintln!("タグがありません");
        return Ok(());
    }

    let mut sorted: Vec<_> = counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    for (tag, count) in &sorted {
        println!("{} ({})", tag, count);
    }

    Ok(())
}

// ─── edit ────────────────────────────────────────────────

pub fn edit(name: &str) -> Result<(), String> {
    let snippets = store::load_snippets()?;
    if !snippets.iter().any(|s| s.name == name) {
        return Err(format!("エントリが見つかりません: {}", name));
    }

    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());

    // エントリだけを一時ファイルに切り出す（NamedTempFile でパストラバーサル回避）
    let yaml = serde_yaml::to_string(&snippets.iter().find(|s| s.name == name).unwrap())
        .map_err(|e| format!("YAML変換に失敗: {}", e))?;

    let tmp = tempfile::Builder::new()
        .prefix("snippet-")
        .suffix(".yml")
        .tempfile()
        .map_err(|e| format!("一時ファイルの作成に失敗: {}", e))?;

    std::fs::write(tmp.path(), &yaml)
        .map_err(|e| format!("一時ファイルの書き込みに失敗: {}", e))?;

    let status = Command::new(&editor)
        .arg(tmp.path())
        .status()
        .map_err(|e| format!("エディタの起動に失敗: {} ({})", editor, e))?;

    if !status.success() {
        return Err("エディタが異常終了しました".to_string());
    }

    let edited = std::fs::read_to_string(tmp.path())
        .map_err(|e| format!("一時ファイルの読み込みに失敗: {}", e))?;

    let edited_snippet: Snippet =
        serde_yaml::from_str(&edited).map_err(|e| format!("編集後のYAMLパースに失敗: {}", e))?;

    // name の変更で重複しないかチェック
    if edited_snippet.name != name && snippets.iter().any(|s| s.name == edited_snippet.name) {
        return Err(format!("name が重複しています: {}", edited_snippet.name));
    }

    // 全エントリを再構築して書き出す（コメントは失われる）
    let new_snippets: Vec<Snippet> = snippets
        .into_iter()
        .map(|s| {
            if s.name == name {
                edited_snippet.clone()
            } else {
                s
            }
        })
        .collect();

    let yaml =
        serde_yaml::to_string(&new_snippets).map_err(|e| format!("YAML変換に失敗: {}", e))?;
    std::fs::write(store::snippets_path(), yaml)
        .map_err(|e| format!("ファイルの書き込みに失敗: {}", e))?;

    eprintln!("保存しました: {}", edited_snippet.name);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── プレースホルダのパース ──────────────────────────

    #[test]
    fn 複数のプレースホルダを出現順に抽出する() {
        // Arrange
        let command = "ffmpeg -i {{input}} -o {{output}}";

        // Act
        let result = parse_placeholders(command);

        // Assert
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].name, "input");
        assert_eq!(result[1].name, "output");
    }

    #[test]
    fn デフォルト値つきプレースホルダを名前と値に分離する() {
        // Arrange
        let command = "convert -quality {{quality:80}} {{input}}";

        // Act
        let result = parse_placeholders(command);

        // Assert
        assert_eq!(result[0].name, "quality");
        assert_eq!(result[0].default, Some("80".to_string()));
        assert_eq!(result[0].raw, "{{quality:80}}");
        assert_eq!(result[1].name, "input");
    }

    #[test]
    fn 同名同形のプレースホルダは最初の1つだけ残る() {
        // Arrange
        let command = "cp {{file}} /backup/{{file}}";

        // Act
        let result = parse_placeholders(command);

        // Assert
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "file");
    }

    #[test]
    fn 同名でデフォルト値が異なるプレースホルダは別々に抽出される() {
        // Arrange
        let command = "convert -q {{quality:80}} -o {{quality:90}}";

        // Act
        let result = parse_placeholders(command);

        // Assert
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].raw, "{{quality:80}}");
        assert_eq!(result[1].raw, "{{quality:90}}");
    }

    #[test]
    fn プレースホルダがなければ空を返す() {
        // Arrange
        let command = "echo hello world";

        // Act
        let result = parse_placeholders(command);

        // Assert
        assert!(result.is_empty());
    }

    #[test]
    fn 閉じ括弧がないプレースホルダは無視する() {
        // Arrange
        let command = "echo {{broken";

        // Act
        let result = parse_placeholders(command);

        // Assert
        assert!(result.is_empty());
    }

    // ─── 検索フィルタリング ─────────────────────────────

    #[test]
    fn クエリなしで全件返る() {
        // Arrange
        let snippets = vec![
            Snippet::minimal("a", "first"),
            Snippet::minimal("b", "second"),
        ];

        // Act
        let results = filter_snippets(&snippets, None);

        // Assert
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn 名前に部分一致する() {
        // Arrange
        let snippets = vec![
            Snippet::minimal("loudnorm", "normalize audio"),
            Snippet::minimal("resize", "resize image"),
        ];

        // Act
        let results = filter_snippets(&snippets, Some("loud"));

        // Assert
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "loudnorm");
    }

    #[test]
    fn 検索は大文字小文字を区別しない() {
        // Arrange
        let snippets = vec![Snippet::minimal("FFmpeg-encode", "Encode video")];

        // Act
        let results = filter_snippets(&snippets, Some("ffmpeg"));

        // Assert
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn タグにもマッチする() {
        // Arrange
        let snippets = vec![Snippet::full(
            "deploy",
            "deploy to prod",
            None,
            &["aws", "prod"],
            None,
        )];

        // Act
        let results = filter_snippets(&snippets, Some("aws"));

        // Assert
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "deploy");
    }

    #[test]
    fn コマンド文字列にもマッチする() {
        // Arrange
        let snippets = vec![Snippet::full(
            "enc",
            "encode",
            Some("ffmpeg -i {{input}}"),
            &[],
            None,
        )];

        // Act
        let results = filter_snippets(&snippets, Some("ffmpeg"));

        // Assert
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn ボディにもマッチする() {
        // Arrange
        let snippets = vec![Snippet::full(
            "setup",
            "setup steps",
            None,
            &[],
            Some("run docker compose up"),
        )];

        // Act
        let results = filter_snippets(&snippets, Some("docker"));

        // Assert
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn マッチしなければ空を返す() {
        // Arrange
        let snippets = vec![Snippet::minimal("hello", "world")];

        // Act
        let results = filter_snippets(&snippets, Some("zzz"));

        // Assert
        assert!(results.is_empty());
    }
}
