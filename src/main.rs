use anyhow::Context;
use atharu::{app, cli, config, db, dump};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let (cmd, cfg) =
        config::Config::from_env_and_args(std::env::args().skip(1)).map_err(anyhow::Error::msg)?;
    let mut conn = db::open(&cfg.db).with_context(|| format!("open {}", cfg.db.display()))?;

    match cmd {
        config::Command::Cli(args) => {
            let agent = std::env::var("ATHARU_AGENT").ok();
            let today = chrono::Local::now().date_naive();
            let code = cli::run(
                &mut conn,
                today,
                &args,
                agent.as_deref(),
                &mut std::io::stdout(),
                &mut std::io::stderr(),
            );
            std::process::exit(code);
        }
        config::Command::Dump => {
            let out = cfg.data_dir().join("dump.sql");
            dump::write(&conn, &out).with_context(|| format!("dump to {}", out.display()))?;
            println!("wrote {}", out.display());
        }
        config::Command::Serve => {
            let mut state = app::AppState::new(conn);
            state.settings = std::sync::Arc::new(config::FileConfig::load(
                &cfg.data_dir().join("config.toml"),
            ));
            app::spawn_refreshers(&state);
            let addr = format!("{}:{}", cfg.host, cfg.port);
            let listener = tokio::net::TcpListener::bind(&addr)
                .await
                .with_context(|| format!("bind {addr}"))?;
            println!("atharu on http://{addr}");
            axum::serve(listener, app::router(state)).await?;
        }
    }
    Ok(())
}
