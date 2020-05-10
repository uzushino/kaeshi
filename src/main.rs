use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

mod app;
mod parser;
mod table;

const CSV: &'static str = r#"id,name,age,email
=============
1,abc,10,abc@example.com
2,def,20,def@example.com
3,def,30,def@example.com
=============
total,30
"#;

fn main() {
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    let thandle = thread::spawn(move || {
        while running.load(Ordering::Relaxed) {
            let mut input = String::default();
            let _ = io::stdin().read_line(&mut input);
            let app = app::App::load_from_file("sample.yml");
            let csv = app.get("csv").unwrap();
            let comb= app::App::combinator(csv.templates.clone());
            let (_rest, rows) = comb(CSV).unwrap();

            table::printstd(&rows);
        }
    });

    ctrlc::set_handler(move || {
        r.store(false, Ordering::Relaxed);
    })
    .expect("Error setting Ctrl-C handler");

    let _ = thandle.join();
}