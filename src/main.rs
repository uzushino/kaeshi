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

    let template = r#"
    abc
    a{{ b0 }}c
    a {{ b1 }} c
    "#;

    let thandle = thread::spawn(move || {
        let syn = parser::Syntax::default();
        let rows = Vec::default();
        let templates: Vec<&str> = template.trim().split("\n").collect();

        while running.load(Ordering::Relaxed) {
            let mut lines: Vec<String> = Vec::default();
            for _ in templates.clone() {
                let mut input = String::new();
                let _ = io::stdin().read_line(&mut input);
                lines.push(input.trim().to_string());
            }

            let tokens = parser::parse(template.trim(), &syn);
            /*
            let f = app::make_combinator(&tokens);
            match f(lines.join("\n").trim()) {
                Ok((_rest, rebind)) => rows.push(rebind),
                Err(_) => {}
            };
            */ 
            table::printstd(&rows);
        }
    });

    ctrlc::set_handler(move || {
        r.store(false, Ordering::Relaxed);
    })
    .expect("Error setting Ctrl-C handler");

    let _ = thandle.join();
}