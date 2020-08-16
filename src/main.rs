use log::{ debug, error };
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use structopt::StructOpt;
use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};

mod app;

#[derive(Debug, StructOpt)]
struct Opt {
    pub file: Option<String>,

    #[structopt(short, long)]
    pub tags: Vec<String>,

    #[structopt(short, long)]
    pub filters: Vec<String>
}

fn main() -> anyhow::Result<()> {
    env_logger::init();
    let opt = Opt::from_args();

    let config: app::AppConfig = if let Some(file) = opt.file { 
        let contents = std::fs::read_to_string(file)?;
        debug!("{}", contents);
        serde_yaml::from_str(&contents)?
    } else {
        let mut config = app::AppConfig::default();
        let mut tokens = opt.tags
            .iter()
            .map(|tag| app::TokenExpr::new_with_tag(tag))
            .collect::<Vec<_>>();
        config.templates.append(&mut tokens);
        config
    };

    debug!("{:?}", config);

    let app = app::App::new_with_config(&config)?; 
    let stdin = std::io::stdin();

    loop {
        let mut buf = Vec::with_capacity(1024usize);

        match stdin.lock().read_until(b'\n', &mut buf) {
            Ok(n) => {
                let line = String::from_utf8_lossy(&buf).to_string();
                debug!("input line: {}", line);

                if n == 0 {
                    debug!("eof");
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

    app.handler.unwrap().join().expect("Couldn't join on the associated thread");
    
    Ok(())
}