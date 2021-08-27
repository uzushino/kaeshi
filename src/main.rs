use structopt::StructOpt;
use tokio::sync::mpsc;
use std::collections::BTreeMap;

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
    pub query: Option<String>,

    #[structopt(short, long)]
    pub dump: Option<String>,
    
    #[structopt(short, long)]
    pub restore: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let opt = Opt::from_args();

    let config: app::AppConfig = {
        let mut config = app::AppConfig::default();
        let mut tokens = opt.tags
            .iter()
            .map(|tag| app::TokenExpr::new_with_tag(tag))
            .collect::<Vec<_>>();
        config.templates.append(&mut tokens);
        config
    };

    let (tx, mut rx): (mpsc::UnboundedSender<app::InputToken>, mpsc::UnboundedReceiver<app::InputToken>) = mpsc::unbounded_channel();
    let templates = config.templates.clone();
    let app = app::App::new_with_config(tx, config).await?;
            
    if let Some(restore) = opt.restore {
        let content = std::fs::read_to_string(restore)?;
        let records: app::DB = serde_json::from_str(content.as_str())?;

        for record in records {
            app.db.borrow_mut().insert(&record).await?;
        }
    }

    let _ret = tokio::join!(
        app.input_handler(),
        app.parse_handler(&mut rx, templates)
    );
    let query = opt.query.unwrap_or("SELECT * FROM main".to_string());
    let result = app.execute(query.as_str()).await?;

    match result {
        Some(gluesql::Payload::Select { labels: l, rows: row}) => {
            let f = |r: &gluesql::data::Value| { 
                match r {
                    gluesql::data::Value::Str(s) => (*s).clone(),
                    _ => String::default()
                }
            };

            let records: app::DB = row
                .iter()
                .map(|r| l.clone().into_iter().zip(r.0.iter().map(f).collect::<Vec<String>>()).collect::<BTreeMap<String, String>>())
                .collect::<Vec<_>>();
            table::printstd(std::io::stdout(), &records)?;

            if let Some(dump) = opt.dump {
                std::fs::write(dump, serde_json::to_string(&records)?)?;
            }
        },
        _ => {}
    };

    Ok(())
}
