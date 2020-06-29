use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use structopt::StructOpt;

mod app;

#[derive(Debug, StructOpt)]
struct Opt {
    pub file: String,
}

fn parse_input(ap: &app::App) -> Option<String> {
    let mut input = String::default();
    let _ = io::stdin().read_line(&mut input).ok();
    let mut result = String::default();
    let head = app::App::build(vec![ap.conditions.start.clone()]);

    if head(input.as_str()).is_ok() {
        result = input.clone();
        loop {
            let mut buf = String::default();
            let _ = io::stdin().read_line(&mut buf);
            let combinator = app::App::build(vec![ap.conditions.end.clone()]);
            let r = combinator(buf.as_str());
            
            result = format!("{}{}", result, buf);
            if r.is_ok() {
                break;
            }
        }
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
                let input = parse_input(&ap);
                let combinate = app::App::build(ap.templates.clone());
               
                match combinate(&input.unwrap()) {
                    Ok((_rest, rows)) => ap.print(&rows),
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