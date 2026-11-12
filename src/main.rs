use anyhow::Context;
use atharu::{app, cli, config, db, dump};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Exit 2 like the CLI's own usage errors, so scripts can tell "bad call" from "refused".
    let (cmd, cfg) = match config::Config::from_env_and_args(std::env::args().skip(1)) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    if let config::Command::Help = cmd {
        println!("{}", cli::HELP);
        return Ok(());
    }
    if let config::Command::Cli(args) = &cmd
        && matches!(args[0].as_str(), "version" | "update")
    {
        let repo = cli::update::repo();
        let code = cli::update::run(
            args,
            repo.as_ref(),
            &mut std::io::stdout(),
            &mut std::io::stderr(),
        );
        std::process::exit(code);
    }
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
                &mut std::io::stdin(),
                &mut std::io::stdout(),
                &mut std::io::stderr(),
            );
            std::process::exit(code);
        }
        config::Command::Help => unreachable!("handled before opening the DB"),
        config::Command::Dump => {
            let out = cfg.data_dir().join("dump.sql");
            dump::write(&conn, &out).with_context(|| format!("dump to {}", out.display()))?;
            println!("wrote {}", out.display());
        }
        config::Command::Serve => {
            let mut state = app::AppState::new(conn);
            state.bind_host = cfg.host.clone();
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
