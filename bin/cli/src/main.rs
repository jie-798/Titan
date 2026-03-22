mod commands;

use clap::{Parser, Subcommand};

const DEFAULT_CONFIG_PATH: &str = "data/config.yaml";

#[derive(Parser)]
#[command(name = "titan", version, about = "Titan proxy toolkit")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Subscribe {
        #[arg(short, long)]
        url: String,

        #[arg(short, long, default_value = DEFAULT_CONFIG_PATH)]
        output: String,
    },
    GeoipUpdate {
        #[arg(long, default_value = titan_rules::geoip::DEFAULT_GEOIP_DB_PATH)]
        output: String,

        #[arg(
            long,
            default_value = "https://ftp.apnic.net/stats/apnic/delegated-apnic-latest"
        )]
        url: String,

        #[arg(long)]
        interval_hours: Option<u64>,
    },
    GeositeUpdate {
        #[arg(short, long, default_value = DEFAULT_CONFIG_PATH)]
        config: String,

        #[arg(long, default_value = titan_rules::geosite::DEFAULT_GEOSITE_DIR)]
        output_dir: String,

        #[arg(
            long,
            default_value = "https://raw.githubusercontent.com/v2fly/domain-list-community/master/data"
        )]
        base_url: String,

        #[arg(long)]
        category: Option<String>,

        #[arg(long)]
        interval_hours: Option<u64>,
    },
    Run {
        #[arg(short, long, default_value = DEFAULT_CONFIG_PATH)]
        config: String,

        #[arg(short, long, default_value = "127.0.0.1")]
        bind: String,

        #[arg(short, long, default_value = "7890")]
        port: u16,

        #[arg(long)]
        api_port: Option<u16>,

        #[arg(long, default_value = "127.0.0.1")]
        api_bind: String,

        #[arg(long)]
        set_system_proxy: bool,
    },
    Test {
        #[arg(short, long, default_value = DEFAULT_CONFIG_PATH)]
        config: String,
    },
    Info {
        #[arg(short, long, default_value = DEFAULT_CONFIG_PATH)]
        config: String,
    },
    Select {
        #[arg(short, long, default_value = DEFAULT_CONFIG_PATH)]
        config: String,

        #[arg(short, long)]
        group: String,

        #[arg(short, long)]
        proxy: String,
    },
    SystemProxy {
        #[command(subcommand)]
        command: SystemProxyCommands,
    },
}

#[derive(Subcommand)]
enum SystemProxyCommands {
    Set {
        #[arg(long, default_value = "127.0.0.1")]
        host: String,

        #[arg(long, default_value = "7890")]
        port: u16,
    },
    Unset,
    Status,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "titan=info".into()),
        )
        .init();

    titan_protocols::init_crypto_provider();

    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Subscribe { url, output }) => commands::subscribe(&url, &output).await,
        Some(Commands::GeoipUpdate {
            output,
            url,
            interval_hours,
        }) => commands::geoip_update(&url, &output, interval_hours).await,
        Some(Commands::GeositeUpdate {
            config,
            output_dir,
            base_url,
            category,
            interval_hours,
        }) => {
            commands::geosite_update(
                &config,
                &base_url,
                &output_dir,
                category.as_deref(),
                interval_hours,
            )
            .await
        }
        Some(Commands::Run {
            config,
            bind,
            port,
            api_port,
            api_bind,
            set_system_proxy,
        }) => commands::run(
            &config,
            &bind,
            port,
            api_port,
            &api_bind,
            set_system_proxy,
        )
        .await,
        Some(Commands::Test { config }) => commands::test(&config).await,
        Some(Commands::Info { config }) => commands::info(&config).await,
        Some(Commands::Select {
            config,
            group,
            proxy,
        }) => commands::select(&config, &group, &proxy).await,
        Some(Commands::SystemProxy { command }) => match command {
            SystemProxyCommands::Set { host, port } => commands::system_proxy_set(&host, port),
            SystemProxyCommands::Unset => commands::system_proxy_unset(),
            SystemProxyCommands::Status => commands::system_proxy_status(),
        },
        None => {
            println!("Titan v{}", env!("CARGO_PKG_VERSION"));
            println!("Use --help for available commands");
            Ok(())
        }
    }
}
