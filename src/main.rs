use log::{ debug, error };
use structopt::StructOpt;
use std::io::{ BufRead };

mod parser;
mod table;
mod db;
mod app;
mod storage;

#[derive(Debug, StructOpt)]
struct Opt {
    pub file: Option<String>,

    #[structopt(short, long)]
    pub tags: Vec<String>,

    #[structopt(short, long)]
    pub manies: Vec<String>,

    #[structopt(short, long)]
    pub filters: Vec<String>,

    #[structopt(short, long)]
    pub output: Option<String>,
    
    #[structopt(short, long)]
    pub query: Option<String>,
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

        let mut tokens = opt.manies
            .iter()
            .map(|tag| {
                let mut tag = app::TokenExpr::new_with_tag(tag);
                tag.many = Some(true);
                tag
            })
            .collect::<Vec<_>>();

        config.templates.append(&mut tokens);

        config
    };

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

    app.handler.unwrap()
        .join()
        .expect("Couldn't join on the associated thread");

    if let Some(query) = opt.query {
        let mut db = app.db.lock()
            .unwrap();

        if let Ok(result) = db.execute(query.as_str()) {
            dbg!(&result);
        }
    }         

    Ok(())
}
