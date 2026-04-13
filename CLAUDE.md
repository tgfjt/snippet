# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

snipl — コマンドや手順を保存・検索・実行する CLI。バイナリ名は `snippet`。
データは `~/.snippets.yml`（env `SNIPPET_FILE` で上書き可）に YAML で1ファイル管理。

## Commands

```bash
cargo build              # ビルド
cargo test               # 全テスト実行
cargo test テスト名      # 単一テスト実行（テスト名は部分一致）
cargo run -- search      # 開発中の動作確認（サブコマンドを -- の後に渡す）
```

テスト時はファイルI/Oの分離に `SNIPPET_FILE` 環境変数を使う:
```bash
SNIPPET_FILE=/tmp/test.yml cargo run -- search
```

## Architecture

```
src/
  main.rs      — clap CLI 定義とディスパッチのみ
  snippet.rs   — Snippet 構造体（serde でシリアライズ）
  store.rs     — ファイル I/O（load_snippets, append_snippet）
  commands.rs  — 各サブコマンドの実装 + parse_placeholders
```

- **store の書き込みは append 方式**。serde_yaml で全体を再シリアライズしない。既存ファイルのコメントを壊さないため。`edit` コマンドだけは全体再シリアライズするのでコメントが消える。
- **`yaml_scalar`**（store.rs）は手書きの YAML エスケープ。serde_yaml を通さず生テキストで YAML を組み立てるために必要。
- **`parse_placeholders`**（commands.rs）は `{{name}}` / `{{name:default}}` を手動パースする。regex 非依存。
- 対話的 I/O は `dialoguer` 経由。テストでは I/O 部分を避け、ロジックだけをテストする。

## Testing

TESTING.md にガイドラインがある。要点:

- テストは「想定の確立」。実装確認ではなく振る舞いの契約を書く
- 3A パターン（Arrange / Act / Assert）を守る。Act は1テスト1操作
- テスト名は日本語で「何が・どうなる」（例: `検索はタグにもマッチする`）
- `process::exit` を呼ぶパスはテストしない（テストプロセスが死ぬ）
- ファイル操作テストは一時ファイル + `SNIPPET_FILE` 環境変数で分離

## Data schema

```yaml
- name: string        # 必須、一意
  description: string # 必須
  command: string     # 任意。{{placeholder}} / {{name:default}} でパラメータ化
  tags: [string]      # 任意
  body: string        # 任意。複数行の手順書
```

`command` と `body` は両方なし・片方だけ・両方ありのいずれも有効。
