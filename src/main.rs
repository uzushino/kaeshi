use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

mod app;
mod parser;
mod table;

fn main() {
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    let thandle = thread::spawn(move || {
        let app = app::App::load_from_file("sample.yml");
            
        while running.load(Ordering::Relaxed) {
            let mut input = String::default();
            let _ = io::stdin().read_line(&mut input);
            
            let combinators = app
                .iter()
                .map(|(_, ap)| {
                    app::App::combinator(ap.templates.clone())
                })
                .collect::<Vec<_>>();

            for combinator in combinators.iter() {
                let _ = combinator(&input)
                    .and_then(|(_rest, rows)| {
                        table::printstd(&rows);
                        Ok(())    
                    }) ;
            }
        }
    });

    ctrlc::set_handler(move || {
        r.store(false, Ordering::Relaxed);
    })
    .expect("Error setting Ctrl-C handler");

    let _ = thandle.join();
}