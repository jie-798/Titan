mod commands;

use std::time::Duration;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "titan", version, about = "Titan proxy toolkit")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Subscribe {
        #[arg(short, long, default_value = commands::DEFAULT_CONFIG_PATH)]
        output: String,

        #[arg(short, long)]
        url: Option<String>,

        #[arg(long)]
        interval_hours: Option<u64>,
    },
    GeoipUpdate {
        #[arg(long, default_value = commands::DEFAULT_GEOIP_PATH)]
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
        #[arg(short, long, default_value = commands::DEFAULT_CONFIG_PATH)]
        config: String,

        #[arg(long, default_value = commands::DEFAULT_GEOSITE_DIR)]
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
        #[arg(short, long, default_value = commands::DEFAULT_CONFIG_PATH)]
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
        #[arg(short, long, default_value = commands::DEFAULT_CONFIG_PATH)]
        config: String,

        #[arg(long)]
        proxy: Option<String>,

        #[arg(long)]
        group: Option<String>,

        #[arg(long, default_value = "http://www.gstatic.com/generate_204")]
        url: String,

        #[arg(long, default_value_t = 5)]
        timeout_secs: u64,
    },
    Info {
        #[arg(short, long, default_value = commands::DEFAULT_CONFIG_PATH)]
        config: String,
    },
    Select {
        #[arg(short, long, default_value = commands::DEFAULT_CONFIG_PATH)]
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
    Logs {
        #[arg(short = 'n', long, default_value_t = 80)]
        lines: usize,
    },
    Doctor {
        #[arg(short, long, default_value = commands::DEFAULT_CONFIG_PATH)]
        config: String,
    },
    Runtime {
        #[arg(long, default_value = "http://127.0.0.1:9090")]
        api: String,
    },
    Sessions {
        #[arg(long, default_value = "http://127.0.0.1:9090")]
        api: String,

        #[arg(long, default_value = "active")]
        state: String,

        #[arg(long, default_value_t = 100)]
        limit: usize,

        #[arg(long)]
        close_all: bool,

        #[arg(long)]
        clear_history: bool,
    },
    Tun {
        #[command(subcommand)]
        command: TunCommands,
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

#[derive(Subcommand)]
enum TunCommands {
    Init {
        #[arg(long, default_value = commands::DEFAULT_TUN_CONFIG_PATH)]
        config: String,

        #[arg(long)]
        force: bool,
    },
    Status {
        #[arg(long, default_value = commands::DEFAULT_TUN_STATE_PATH)]
        state: String,

        #[arg(long)]
        config: Option<String>,
    },
    Up {
        #[arg(long, default_value = commands::DEFAULT_TUN_CONFIG_PATH)]
        config: String,

        #[arg(long, default_value = commands::DEFAULT_TUN_STATE_PATH)]
        state: String,

        #[arg(long)]
        preview: bool,
    },
    Hold {
        #[arg(long, default_value = commands::DEFAULT_TUN_CONFIG_PATH)]
        config: String,

        #[arg(long, default_value = commands::DEFAULT_TUN_STATE_PATH)]
        state: String,
    },
    Run {
        #[arg(long, default_value = commands::DEFAULT_TUN_CONFIG_PATH)]
        config: String,

        #[arg(long, default_value = commands::DEFAULT_TUN_STATE_PATH)]
        state: String,

        #[arg(long, default_value = commands::DEFAULT_CONFIG_PATH)]
        proxy_config: String,
    },
    Inspect {
        #[arg(long, default_value = commands::DEFAULT_TUN_CONFIG_PATH)]
        config: String,

        #[arg(long, default_value = commands::DEFAULT_TUN_STATE_PATH)]
        state: String,

        #[arg(long, default_value = commands::DEFAULT_CONFIG_PATH)]
        proxy_config: String,

        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    Down {
        #[arg(long, default_value = commands::DEFAULT_TUN_STATE_PATH)]
        state: String,
    },
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
        Some(Commands::Subscribe {
            url,
            output,
            interval_hours,
        }) => commands::subscribe(url.as_deref(), &output, interval_hours).await,
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
        }) => commands::run(&config, &bind, port, api_port, &api_bind, set_system_proxy).await,
        Some(Commands::Test {
            config,
            proxy,
            group,
            url,
            timeout_secs,
        }) => {
            commands::test(
                &config,
                proxy.as_deref(),
                group.as_deref(),
                &url,
                Duration::from_secs(timeout_secs),
            )
            .await
        }
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
        Some(Commands::Logs { lines }) => commands::logs(lines).await,
        Some(Commands::Doctor { config }) => commands::doctor(&config).await,
        Some(Commands::Runtime { api }) => commands::runtime(&api).await,
        Some(Commands::Sessions { api, state, limit, close_all, clear_history }) => {
            commands::sessions(&api, Some(state.as_str()), limit, close_all, clear_history).await
        },
        Some(Commands::Tun { command }) => match command {
            TunCommands::Init { config, force } => commands::tun_init(config.as_str(), force),
            TunCommands::Status { state, config } => {
                commands::tun_status(state.as_str(), config.as_deref())
            }
            TunCommands::Up {
                config,
                state,
                preview,
            } => {
                if preview {
                    commands::tun_prepare(config.as_str(), state.as_str())
                } else {
                    commands::tun_up(config.as_str(), state.as_str())
                }
            }
            TunCommands::Hold { config, state } => {
                commands::tun_hold(config.as_str(), state.as_str()).await
            }
            TunCommands::Run {
                config,
                state,
                proxy_config,
            } => {
                commands::tun_run(config.as_str(), proxy_config.as_str(), state.as_str()).await
            }
            TunCommands::Inspect {
                config,
                state,
                proxy_config,
                limit,
            } => {
                commands::tun_inspect(
                    config.as_str(),
                    proxy_config.as_str(),
                    state.as_str(),
                    limit,
                )
                .await
            }
            TunCommands::Down { state } => commands::tun_down(state.as_str()),
        },
        None => {
            println!("Titan v{}", env!("CARGO_PKG_VERSION"));
            println!("Workspace: {}", commands::display_path(commands::workspace_root()));
            println!("Default config: {}", commands::display_path(commands::default_config_path()));
            println!("Use --help for available commands");
            Ok(())
        }
    }
}
