use std::error::Error;
use std::io::{self, Write};
use std::path::PathBuf;
use tokio;
use tokio::signal::unix::{signal, SignalKind};
use tokio::time::{sleep, Duration};



// Create a custom writer that writes to both file and stdout
struct MultiWriter {
    file: std::fs::File,
    stdout: std::io::Stderr,
}

impl Write for MultiWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.file.write_all(buf)?;
        self.stdout.write_all(buf)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()?;
        self.stdout.flush()?;
        Ok(())
    }
}


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
    // Create necessary directories
    let log_dir = PathBuf::from("var/log/daemon");
    std::fs::create_dir_all(&log_dir);

    // Set up logging with absolute paths
    {
        use env_logger::{Builder, Target};
        use log::LevelFilter;
        let log_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_dir.join("daemon.log"))?;

        let multi_writer = MultiWriter {
            file: log_file,
            stdout: std::io::stderr(),
        };

        Builder::from_default_env()
            .target(Target::Pipe(Box::new(multi_writer)))
            .filter_level(LevelFilter::Debug)
            .init();
        log::debug!("logger initialized with log dir {:?}", &log_dir);
    }

    log::info!("starting");

    #[cfg(unix)]
    {
        use std::env;
        let args: Vec<String> = env::args().collect();
        let foreground = args.contains(&"--foreground".to_string());

        if foreground {
            daemon_main().await;
            Ok(())
        }
        else {
            use daemonize::Daemonize;
            let run_dir = PathBuf::from("var/run/daemon");
            std::fs::create_dir_all(&run_dir);
            log::debug!("run dir is {:?}", &run_dir);
            log::debug!("Daemonizing");
            let d = Daemonize::new()
                .working_directory(&run_dir)
                .pid_file("daemon.pid")
                .chown_pid_file(true);
            match d.start() {
                Ok(_) => daemon_main().await,
                Err(e) => {
                    log::error!("error daemonizing: {}", e);
                    Ok(())
                }
            }
        }
    }
}
