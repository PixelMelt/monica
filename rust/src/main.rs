use clap::{Parser, Subcommand};
use monica::config::Config;
use monica::http::state::AppState;
use tracing::Level;
use tracing_subscriber::filter::Targets;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{EnvFilter, fmt};

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
    /// Send every scheduled reminder now (not in production; `test:send-reminders`).
    TestReminders,
    /// Prepare an account with fake data (`monica:dummy`). Deletes ALL data
    /// unless `--migrate` is given.
    Dummy {
        /// Run the pending migrations instead of dropping every table first.
        #[arg(long)]
        migrate: bool,
        /// Don't ask for confirmation.
        #[arg(long)]
        force: bool,
    },
    /// Add a CardDAV address book subscription to a vault
    /// (`monica:newaddressbooksubscription`).
    NewAddressBookSubscription {
        /// Monica account to add subscription to.
        #[arg(long)]
        email: Option<String>,
        /// Id of the vault to add subscription to.
        #[arg(long = "vaultId")]
        vault_id: Option<String>,
        /// CardDAV url of the address book.
        #[arg(long)]
        url: Option<String>,
        /// Login.
        #[arg(long)]
        login: Option<String>,
        /// Password of the account.
        #[arg(long)]
        password: Option<String>,
        /// Set only push way.
        #[arg(long)]
        pushonly: bool,
        /// Set only get way.
        #[arg(long)]
        getonly: bool,
    },
    /// Synchronize one address book subscription now
    /// (`monica:updateaddressbooksubscription`).
    UpdateAddressBookSubscription {
        /// The subscription's id.
        #[arg(long = "subscriptionId")]
        subscription_id: String,
        /// Force a full sync.
        #[arg(long)]
        force: bool,
    },
    /// Print the schema of a database as JSON.
    SchemaDump {
        /// Database URL.
        #[arg(long)]
        database_url: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            fmt::layer()
                .with_writer(std::io::stderr)
                .with_ansi(std::io::IsTerminal::is_terminal(&std::io::stderr()))
                .with_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into())),
        )
        .with(monica::logging::layer().with_filter(Targets::new().with_target(monica::logging::TARGET, Level::DEBUG)))
        .init();
    match Cli::parse().command.unwrap_or(Command::Serve) {
        Command::Serve => {
            let config = Config::from_env()?;
            let db = monica::commands::wait_for_db(&config.database_url).await?;
            monica::db::migrate(&db).await?;
            monica::logging::start(db.clone(), config.env.name().into());
            let bind = config.bind;
            let state = AppState::new(db, config)?;
            tokio::spawn(monica::scheduler::run_forever(state.clone()));
            let app = monica::http::app(state);
            let listener = tokio::net::TcpListener::bind(bind).await?;
            tracing::info!("listening on {bind}");
            axum::serve(listener, app.into_make_service_with_connect_info::<std::net::SocketAddr>()).await?;
        }
        Command::Migrate => {
            let config = Config::from_env()?;
            let db = monica::commands::wait_for_db(&config.database_url).await?;
            monica::db::migrate(&db).await?;
        }
        Command::TestReminders => {
            let config = Config::from_env()?;
            let db = monica::commands::wait_for_db(&config.database_url).await?;
            let state = AppState::new(db, config.clone())?;
            let sent = monica::commands::test_reminders(&state, &config).await?;
            println!("{sent} reminders sent.");
        }
        Command::Dummy { migrate, force } => {
            let config = Config::from_env()?;
            let db = monica::commands::wait_for_db(&config.database_url).await?;
            let mailer = monica::mail::Mailer::new(&config)?;
            let confirmed = force || {
                println!("Are you sure you want to proceed? This will delete ALL data in your environment.");
                println!("Are you sure you want to run this command? (yes/no) [no]");
                let mut answer = String::new();
                std::io::stdin().read_line(&mut answer)?;
                matches!(answer.trim(), "y" | "yes")
            };
            if !confirmed {
                println!("Command cancelled.");
                return Ok(());
            }
            monica::dummy::setup_dummy_account(&db, &config, &mailer, !migrate, true).await?;
        }
        Command::NewAddressBookSubscription { email, vault_id, url, login, password, pushonly, getonly } => {
            let config = Config::from_env()?;
            let db = monica::commands::wait_for_db(&config.database_url).await?;
            let state = AppState::new(db, config)?;
            let options = monica::domains::contact::dav_client::NewAddressBookSubscriptionOptions {
                email,
                vault_id,
                url,
                login,
                password,
                pushonly,
                getonly,
            };
            let code = monica::domains::contact::dav_client::new_address_book_subscription(&state, options).await;
            std::process::exit(code);
        }
        Command::UpdateAddressBookSubscription { subscription_id, force } => {
            let config = Config::from_env()?;
            let db = monica::commands::wait_for_db(&config.database_url).await?;
            let state = AppState::new(db, config)?;
            monica::domains::contact::dav_client::update_address_book_subscription(&state, &subscription_id, force)
                .await?;
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
