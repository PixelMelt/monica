use clap::{Parser, Subcommand};
use monica::config::Config;
use monica::http::state::AppState;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(version, about = "Monica server")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Run the server (the default): applies pending migrations, then listens on `BIND`.
    Serve,
    /// Apply pending database migrations.
    Migrate,
    /// Print the OpenAPI document as JSON.
    Openapi,
    /// Print a new random `APP_KEY` value.
    KeyGenerate,
    /// Print the schema of a database as JSON.
    SchemaDump {
        /// Database URL.
        #[arg(long)]
        database_url: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();
    match Cli::parse().command.unwrap_or(Command::Serve) {
        Command::Serve => {
            let config = Config::from_env()?;
            if let Some(dir) =
                config.database_url.strip_prefix("sqlite://").and_then(|p| std::path::Path::new(p).parent())
            {
                std::fs::create_dir_all(dir)?;
            }
            let db = monica::db::connect(&config.database_url).await?;
            monica::db::migrate(&db).await?;
            let bind = config.bind;
            let app = monica::http::app(AppState::new(db, config)?);
            let listener = tokio::net::TcpListener::bind(bind).await?;
            tracing::info!("listening on {bind}");
            axum::serve(listener, app.into_make_service_with_connect_info::<std::net::SocketAddr>()).await?;
        }
        Command::Migrate => {
            let config = Config::from_env()?;
            let db = monica::db::connect(&config.database_url).await?;
            monica::db::migrate(&db).await?;
        }
        Command::Openapi => println!("{}", monica::api::openapi().to_pretty_json()?),
        Command::KeyGenerate => println!("{}", monica::config::generate_key()),
        Command::SchemaDump { database_url } => {
            let db = monica::db::connect(&database_url).await?;
            let schema = monica::db::schema_dump::dump(&db).await?;
            println!("{}", serde_json::to_string_pretty(&schema)?);
        }
    }
    Ok(())
}
