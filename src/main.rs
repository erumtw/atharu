use anyhow::Context;
use worklog::{app, config, db, dump};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let (cmd, cfg) =
        config::Config::from_env_and_args(std::env::args().skip(1)).map_err(anyhow::Error::msg)?;
    let conn = db::open(&cfg.db).with_context(|| format!("open {}", cfg.db.display()))?;

    match cmd {
        config::Command::Dump => {
            let out = cfg.data_dir().join("dump.sql");
            dump::write(&conn, &out).with_context(|| format!("dump to {}", out.display()))?;
            println!("wrote {}", out.display());
        }
        config::Command::Serve => {
            let state = app::AppState::new(conn);
            let addr = format!("{}:{}", cfg.host, cfg.port);
            let listener = tokio::net::TcpListener::bind(&addr)
                .await
                .with_context(|| format!("bind {addr}"))?;
            println!("worklog on http://{addr}");
            axum::serve(listener, app::router(state)).await?;
        }
    }
    Ok(())
}
