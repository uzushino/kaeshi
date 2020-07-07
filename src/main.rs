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

/*
fn parse_input(ap: &app::App) -> Option<String> {
    let mut input = String::default();
    let _ = io::stdin().read_line(&mut input).ok();
    let mut result = String::default();
    let head = if let Some(ref condition) = ap.conditions {
        app::App::build(vec![condition.start.clone()])
    } else {
        app::App::build(vec![ap.templates[0].clone()])
    };

    if head(input.as_str()).is_ok() {
        if ap.templates.len() == 1 {
            return Some(input.clone());
        }

        result = input.clone();

        loop {
            let mut buf = String::default();
            let _ = io::stdin().read_line(&mut buf);
            let combinator = if let Some(ref condition) = ap.conditions {
                app::App::build(vec![condition.end.clone()])
            } else {
                app::App::build(vec![ap.templates[0].clone()])
            };
            let r = combinator(buf.as_str());
            
            result = format!("{}{}", result, buf);
            if r.is_ok() {
                break;
            }
        }
    }

    Some(result)
}
*/

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