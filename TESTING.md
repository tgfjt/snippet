# Testing

想定の確立。実装確認ではなく振る舞いの契約を書く。

## 3A

Arrange → Act → Assert。Act は 1 テスト 1 操作。Assert は戻り値かファイルの中身。

```rust
#[test]
fn 検索はタグにもマッチする() {
    // Arrange
    let snippets = vec![snippet_with_tags("deploy", &["aws", "prod"])];
    // Act
    let results = filter_snippets(&snippets, "aws");
    // Assert
    assert_eq!(results.len(), 1);
}
```

## 命名

日本語で「何が・どうなる」。`test_` prefix や関数名の繰り返しはしない。

## 分類

**契約** — 純粋関数の入出力（`parse_placeholders`, `yaml_scalar`, `filter_snippets`）

**往復** — `append_snippet_to` → `load_snippets_from` で同じものが返る

**ファイル** — 一時ファイルで境界を検証。空ファイル、追記後の既存エントリ維持、コメント保持

**対象外** — dialoguer の I/O、`sh -c`、`process::exit`

## テスタビリティ

I/O とロジックが混ざったら、ロジックを純粋関数に切り出す。
