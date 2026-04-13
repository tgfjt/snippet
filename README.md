# snipl

コマンドや手順を保存・検索・実行する CLI。
「作ったけど存在を忘れる」問題を解決する。

## Install

```bash
cargo install --path .
```

バイナリ名は `snippet`。

## Usage

### エントリを追加する

```bash
snippet add
```

対話的に name, description, command, tags, body を入力する。

### 検索する

```bash
snippet search           # 全件表示
snippet search ffmpeg    # キーワードで絞り込み（name, description, tags, command, body を部分一致）
snippet search --full    # command / body も表示
```

### 実行する

```bash
snippet run loudnorm -- input=voice.wav output=out.wav
```

`command` フィールドの `{{placeholder}}` を引数で置換して実行する。
デフォルト値つき (`{{quality:80}}`) は引数省略可。未指定のプレースホルダは対話的に聞く。
`command` がないエントリは `body` を表示するだけ。

### その他

```bash
snippet get <name>       # YAML 形式で出力（MCP やスクリプト連携用）
snippet tags             # タグ一覧と使用数
snippet edit <name>      # $EDITOR で編集
```

## Data file

`~/.snippets.yml` に YAML で管理。手書き編集 OK。

```yaml
- name: loudnorm
  description: 音量をノーマライズする（EBU R128）
  command: "ffmpeg -i {{input}} -af loudnorm=I=-14:TP=-1:LRA=11 {{output}}"
  tags: [ffmpeg, audio]

- name: git-worktree-agent
  description: エージェント用にworktreeを切る手順
  tags: [git, agent]
  body: |
    1. git worktree add ../project-agent feature/xxx
    2. cd ../project-agent
    3. claude -p "..."
    4. 終わったら git worktree remove ../project-agent
```

環境変数 `SNIPPET_FILE` でファイルパスを上書きできる。

## Design decisions

- **append 方式の書き込み** — `add` はファイル末尾に生テキストで追記する。serde_yaml で全体を再シリアライズしないので、手書きのコメントが消えない
- **`edit` だけ全体再シリアライズ** — エントリの差し替えが必要なため、コメントは失われる
- **regex 非依存** — プレースホルダのパースは手動。依存を減らすため
