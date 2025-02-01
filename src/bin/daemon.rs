use std::error::Error;
use std::io::{self, Write};
use std::path::PathBuf;
use tokio;
use tokio::signal::unix::{signal, SignalKind};
use tokio::time::{sleep, Duration};

async fn work() {
    sleep(Duration::from_secs(10)).await;
    log::info!("doing work")
}

async fn daemon_main() -> Result<(), Box<dyn Error>> {
    let mut sig_term = signal(SignalKind::terminate())?;
    let mut sig_int = signal(SignalKind::interrupt())?;

    loop {
        log::info!("select");
        tokio::select! {
            _ = sig_term.recv() => {
                log::info!("SIGTERM");
                log::logger().flush();
                break;
            }
            _ = sig_int.recv() => {
                log::info!("SIGINT");
                break;
            }
            _ = work() => {
                log::info!("completed work");
            }
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Create necessary directories with absolute paths
    let run_dir = PathBuf::from("var/run/daemon");
    let log_dir = PathBuf::from("var/log/daemon");
    std::fs::create_dir_all(&run_dir)?;
    std::fs::create_dir_all(&log_dir)?;

    // Set up logging with absolute paths
    {
        use env_logger::{Builder, Target};
        use log::LevelFilter;
        let log_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_dir.join("daemon.log"))?;

        Builder::from_default_env()
            .target(Target::Pipe(Box::new(log_file)))
            .filter_level(LevelFilter::Debug)
            .init();
    }

    log::info!("starting");

    #[cfg(unix)]
    {
        use daemonize::Daemonize;
        let d = Daemonize::new()
            .pid_file("var/run/daemon/daemon.pid")
            .chown_pid_file(true)
            .working_directory("var/run/daemon");

        match d.start() {
            Ok(_) => daemon_main().await,
            Err(e) => {
                log::error!("error daemonizing: {}", e);
                Ok(())
            }
        }
    }
}
