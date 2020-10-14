use log::{ debug, error };
use structopt::StructOpt;
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

    let app = app::App::new_with_config(tx, config).await?;
    let _ret = tokio::join!(
        app.input_handler(),
        app.parse_handler(&mut rx, templates)
    );

    if let Some(query) = opt.query {
        let result = app.execute(query.as_str())?;
        debug!("Result: {:?}", result);
    }         

    Ok(())
}
