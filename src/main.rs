use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use structopt::StructOpt;

mod app;
mod parser;
mod table;

#[derive(Debug, StructOpt)]
struct Opt {
    pub file: String,
}

fn parse_input(templates: Vec<app::Token>) -> Option<String> {
    let mut input = String::default();
    let _ = io::stdin().read_line(&mut input).ok();
    let mut result = String::default();
    let head = 
        app::App::build(vec![templates[0].clone()]);

    match head(input.as_str()) {
        Ok((_rest, _rows)) =>  {
            result = input.clone();
            if templates.len() < 2 {
                return Some(result);
            }

            loop {
                let mut buf = String::default();
                let _ = io::stdin().read_line(&mut buf);
                let l = templates.last()?;
                let combinator = app::App::build(vec![l.clone()]);
                let r = combinator(buf.as_str());
                
                result = format!("{}{}", result, buf);
                if let Ok((_rest, _rows)) = r {
                    break;
                }
            }
        },
        _ => {}
    }

    Some(result)
}

fn main() -> anyhow::Result<()> {
    let opt = Opt::from_args();
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    let app = app::App::load_from_file(opt.file.as_str())?; 

    let thandle = thread::spawn(move || {
        while running.load(Ordering::Relaxed) {
            for (_k, ap) in app.iter() {
                let templates = ap.templates.clone();
                let input = parse_input(templates.clone());
                let combinate = app::App::build(templates.clone());
               
                match combinate(&input.unwrap()) {
                    Ok((_rest, rows)) => table::printstd(&rows),
                    _ => {}
                }
            }
        }
    });

    ctrlc::set_handler(move || {
        r.store(false, Ordering::Relaxed);
    })
    .expect("Error setting Ctrl-C handler");

    let _ = thandle.join();

    Ok(())
}