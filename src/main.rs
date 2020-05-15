use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use structopt::StructOpt;
use std::collections::HashMap;

mod app;
mod parser;
mod table;

#[derive(Debug, StructOpt)]
struct Opt {
    pub file: String,
}

fn parse_input(templates: &Vec<HashMap<String, String>>) {
    let head = 
        app::App::combinator(vec![templates[0].clone()]);
    let mut input = String::default();
    let _ = io::stdin().read_line(&mut input);

    match head(&input.clone()) {
        Ok((_rest, _rows)) =>  {
            loop {
                let mut buf = String::default();
                let _ = io::stdin().read_line(&mut buf);
                let combinator= 
                    app::App::combinator(vec![templates.last().unwrap().clone()]);
                let r = combinator(&buf);
                if let Ok(_) = r {
                    input = format!("{}\n{}", input, buf);
                } else {
                    break;
                }
            }
        },
        _ => {}
    }
}

fn read_in() -> String {
    let mut buf = String::default();
    let _ = io::stdin().read_line(&mut buf);
    buf
}

fn main() -> anyhow::Result<()> {
    let opt = Opt::from_args();

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    let app = app::App::load_from_file(opt.file.as_str())?; 
    let thandle = thread::spawn(move || {
        while running.load(Ordering::Relaxed) {
            let mut input = String::default();
            let _ = io::stdin().read_line(&mut input);
           
            let combinators = app
                .iter()
                .map(|(_, ap)| {
                    let head = 
                        app::App::combinator(vec![ap.templates[0].clone()]);
                    let tail = app::App::combinator(ap.templates[1..].to_vec());
                    (head, tail)
                })
                .collect::<Vec<_>>();

            for (h, t) in combinators.iter() {
                match h(&input) {
                    Ok((_rest, rows)) => {
                        let _ = t(&input)
                            .and_then(|(_rest, rows)| {
                                table::printstd(&rows);
                                Ok(())    
                            }) ;
                    },
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