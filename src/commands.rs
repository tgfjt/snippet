use std::collections::HashMap;
use std::io::{self, BufRead};
use std::process::Command;

use dialoguer::{Confirm, Input};

use crate::snippet::Snippet;
use crate::store;

// ─── search ──────────────────────────────────────────────

pub fn search(query: Option<&str>, full: bool) -> Result<(), String> {
    let snippets = store::load_snippets()?;

    let matches: Vec<&Snippet> = match query {
        Some(q) => {
            let q = q.to_lowercase();
            snippets
                .iter()
                .filter(|s| {
                    s.name.to_lowercase().contains(&q)
                        || s.description.to_lowercase().contains(&q)
                        || s.tags
                            .as_ref()
                            .map_or(false, |tags| tags.iter().any(|t| t.to_lowercase().contains(&q)))
                        || s.command
                            .as_ref()
                            .map_or(false, |c| c.to_lowercase().contains(&q))
                        || s.body
                            .as_ref()
                            .map_or(false, |b| b.to_lowercase().contains(&q))
                })
                .collect()
        }
        None => snippets.iter().collect(),
    };

    if matches.is_empty() {
        if query.is_some() {
            eprintln!("マッチするエントリが見つかりません");
        } else {
            eprintln!("エントリがありません");
        }
        return Ok(());
    }

    for s in &matches {
        let tags_str = s
            .tags
            .as_ref()
            .filter(|t| !t.is_empty())
            .map(|t| format!(" [{}]", t.join(", ")))
            .unwrap_or_default();
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

    match snippets.iter().find(|s| s.name == name) {
        Some(s) => {
            let yaml = serde_yaml::to_string(&vec![s])
                .map_err(|e| format!("YAML出力に失敗: {}", e))?;
            print!("{}", yaml);
            Ok(())
        }
        None => {
            eprintln!("エントリが見つかりません: {}", name);
            std::process::exit(1);
        }
    }
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
    let tags = if tags_input.is_empty() {
        None
    } else {
        Some(
            tags_input
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
        )
    };

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
    while lines.last().map_or(false, |l| l.is_empty()) {
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

    let snippet = match snippets.iter().find(|s| s.name == name) {
        Some(s) => s,
        None => {
            eprintln!("エントリが見つかりません: {}", name);
            std::process::exit(1);
        }
    };

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
    let placeholders = parse_placeholders(&command);
    let mut result = command.clone();

    for (ph_name, default) in &placeholders {
        let value = if let Some(v) = arg_map.get(ph_name) {
            v.clone()
        } else if let Some(d) = default {
            d.clone()
        } else {
            // 対話的に聞く
            Input::new()
                .with_prompt(ph_name)
                .interact_text()
                .map_err(|e| format!("入力エラー: {}", e))?
        };

        // 元のプレースホルダ文字列を組み立てて置換
        let pattern = match default {
            Some(d) => format!("{{{{{ph_name}:{d}}}}}"),
            None => format!("{{{{{ph_name}}}}}"),
        };
        result = result.replace(&pattern, &value);
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
        std::process::exit(status.code().unwrap_or(2));
    }

    Ok(())
}

/// `{{name}}` や `{{name:default}}` を抽出する（出現順、重複排除）
fn parse_placeholders(command: &str) -> Vec<(String, Option<String>)> {
    let mut result = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let bytes = command.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i + 3 < len {
        if bytes[i] == b'{' && bytes[i + 1] == b'{' {
            if let Some(end) = command[i + 2..].find("}}") {
                let inner = &command[i + 2..i + 2 + end];
                if !inner.is_empty() {
                    let (name, default) = match inner.split_once(':') {
                        Some((n, d)) => (n.to_string(), Some(d.to_string())),
                        None => (inner.to_string(), None),
                    };
                    if seen.insert(name.clone()) {
                        result.push((name, default));
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
        if let Some(ref tags) = s.tags {
            for tag in tags {
                *counts.entry(tag.clone()).or_insert(0) += 1;
            }
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

// ─── edit (v1 では未実装) ────────────────────────────────

pub fn edit(name: &str) -> Result<(), String> {
    let snippets = store::load_snippets()?;
    if !snippets.iter().any(|s| s.name == name) {
        eprintln!("エントリが見つかりません: {}", name);
        std::process::exit(1);
    }

    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());

    // エントリだけを一時ファイルに切り出す
    let yaml =
        serde_yaml::to_string(&snippets.iter().find(|s| s.name == name).unwrap())
            .map_err(|e| format!("YAML変換に失敗: {}", e))?;

    let tmp_dir = std::env::temp_dir();
    let tmp_path = tmp_dir.join(format!("snippet-{}.yml", name));
    std::fs::write(&tmp_path, &yaml)
        .map_err(|e| format!("一時ファイルの作成に失敗: {}", e))?;

    let status = Command::new(&editor)
        .arg(&tmp_path)
        .status()
        .map_err(|e| format!("エディタの起動に失敗: {} ({})", editor, e))?;

    if !status.success() {
        let _ = std::fs::remove_file(&tmp_path);
        return Err("エディタが異常終了しました".to_string());
    }

    let edited = std::fs::read_to_string(&tmp_path)
        .map_err(|e| format!("一時ファイルの読み込みに失敗: {}", e))?;
    let _ = std::fs::remove_file(&tmp_path);

    let edited_snippet: Snippet =
        serde_yaml::from_str(&edited).map_err(|e| format!("編集後のYAMLパースに失敗: {}", e))?;

    // name の変更で重複しないかチェック
    if edited_snippet.name != name
        && snippets.iter().any(|s| s.name == edited_snippet.name)
    {
        return Err(format!(
            "name が重複しています: {}",
            edited_snippet.name
        ));
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

    let yaml = serde_yaml::to_string(&new_snippets)
        .map_err(|e| format!("YAML変換に失敗: {}", e))?;
    std::fs::write(store::snippets_path(), yaml)
        .map_err(|e| format!("ファイルの書き込みに失敗: {}", e))?;

    eprintln!("保存しました: {}", edited_snippet.name);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_placeholders_simple() {
        let result = parse_placeholders("ffmpeg -i {{input}} -o {{output}}");
        assert_eq!(result, vec![
            ("input".to_string(), None),
            ("output".to_string(), None),
        ]);
    }

    #[test]
    fn test_parse_placeholders_with_defaults() {
        let result = parse_placeholders("convert -quality {{quality:80}} {{input}}");
        assert_eq!(result, vec![
            ("quality".to_string(), Some("80".to_string())),
            ("input".to_string(), None),
        ]);
    }

    #[test]
    fn test_parse_placeholders_dedup() {
        let result = parse_placeholders("cp {{file}} /tmp/{{file}}");
        assert_eq!(result, vec![("file".to_string(), None)]);
    }

    #[test]
    fn test_parse_placeholders_empty() {
        let result = parse_placeholders("echo hello");
        assert!(result.is_empty());
    }
}
