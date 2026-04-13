use clap::{Parser, Subcommand};

mod commands;
mod snippet;
mod store;

#[derive(Parser)]
#[command(name = "snippet", about = "コマンドや手順を保存・検索・実行するCLI")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// キーワードでスニペットを検索（省略で全件表示）
    Search {
        /// 検索キーワード
        query: Option<String>,
        /// command / body も含めて詳細表示
        #[arg(long)]
        full: bool,
    },
    /// 指定したスニペットを YAML 形式で出力
    Get {
        /// エントリの name
        name: String,
    },
    /// 対話的にスニペットを追加
    Add,
    /// スニペットのコマンドを実行（プレースホルダを置換）
    Run {
        /// エントリの name
        name: String,
        /// key=value 形式のパラメータ（-- の後に指定）
        #[arg(last = true)]
        args: Vec<String>,
    },
    /// タグ一覧と使用数を表示
    Tags,
    /// $EDITOR でスニペットを編集
    Edit {
        /// エントリの name
        name: String,
    },
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Search { query, full } => commands::search(query.as_deref(), full),
        Commands::Get { name } => commands::get(&name),
        Commands::Add => commands::add(),
        Commands::Run { name, args } => commands::run(&name, args),
        Commands::Tags => commands::tags(),
        Commands::Edit { name } => commands::edit(&name),
    };

    if let Err(e) = result {
        eprintln!("エラー: {}", e);
        std::process::exit(2);
    }
}
