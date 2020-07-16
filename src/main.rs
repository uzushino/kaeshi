use log::{ debug, error };
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use structopt::StructOpt;
use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};

mod app;

#[derive(Debug, StructOpt)]
struct Opt {
    pub file: String,
}

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let opt = Opt::from_args();
    let contents = std::fs::read_to_string(opt.file)?;
    debug!("{}", contents);
    let config: app::AppConfig = serde_yaml::from_str(&contents)?;
    let app = app::App::new_with_config(&config)?; 
    let stdin = std::io::stdin();

    loop {
        let mut buf = Vec::with_capacity(1024usize);

        match stdin.lock().read_until(b'\n', &mut buf) {
            Ok(n) => {
                let line = String::from_utf8_lossy(&buf).to_string();
                debug!("input line: {}", line);

                if n == 0 {
                    app.send_byte(b'\0')?;
                    break;
                }
                
                app.send_string(line.to_string())?;
            }
            Err(e) => {
                error!("{}", e.to_string());
                break;
            },
        }
    }

    Ok(())
}