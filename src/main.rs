use log::{ debug, error };
use structopt::StructOpt;
use std::io::{ BufRead };
use tokio::sync::mpsc;
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
    pub source: Option<String>,
    
    #[structopt(short, long)]
    pub query: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
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

    let (tx, mut rx): (mpsc::UnboundedSender<app::InputToken>, mpsc::UnboundedReceiver<app::InputToken>) = mpsc::unbounded_channel();
    let templates = config.templates.clone();

    let mutex_app = std::sync::Arc::new(
        tokio::sync::Mutex::new(app::App::new_with_config(tx, config).await?)
    ); 

    let app1 = mutex_app.clone();
    let handler1 = async move {
        app1.lock_owned().await.handler(&mut rx, templates).await;
    };

    debug!("aaa");

    let app2 = mutex_app.clone();
    let handler2 = async move {
        let app2 = app2.lock_owned().await;
        handler(&app2).await;
    };

    debug!("bbb");
    let res = tokio::join!(
        handler2, 
        handler1
    );
    debug!("Res: {:?}", res);

    /*
    app.handler.unwrap()
        .join()
        .expect("Couldn't join on the associated thread");
    */
    if let Some(query) = opt.query {
        debug!("q {:?}", query);
        let mut app = mutex_app.lock_owned().await;
        let result = app.db.execute(query.as_str());
        debug!("Result: {:?}", result);
    }         

    Ok(())
}

async fn handler(app: &app::App) -> anyhow::Result<()> { 
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

    Ok(())
}
