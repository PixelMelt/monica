use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version, about = "Monica server")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print the schema of a database as JSON.
    SchemaDump {
        /// Database URL.
        #[arg(long)]
        database_url: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    match Cli::parse().command {
        Command::SchemaDump { database_url } => {
            let db = monica::db::connect(&database_url).await?;
            let schema = monica::db::schema_dump::dump(&db).await?;
            println!("{}", serde_json::to_string_pretty(&schema)?);
        }
    }
    Ok(())
}
