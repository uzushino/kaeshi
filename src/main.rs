use std::collections::BTreeMap;
use structopt::StructOpt;
use tokio::sync::mpsc;

use kaeshi::{output, App, AppConfig, InputToken, TokenExpr, DB, OutputType};

#[derive(Debug, StructOpt)]
struct Opt {
    pub file: Option<String>,

    #[structopt(short, long)]
    pub tags: Vec<String>,

    #[structopt(short, long)]
    pub query: Option<String>,

    #[structopt(long)]
    pub table_name: Option<String>,

    #[structopt(possible_values = &OutputType::variants(), case_insensitive = true)]
    pub output_type: Option<OutputType>,

    pub timestamp: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let opt = Opt::from_args();

    let config: AppConfig = {
        let mut config = AppConfig::default();
        let mut tokens = opt
            .tags
            .iter()
            .map(|tag| TokenExpr::new_with_tag(tag))
            .collect::<Vec<_>>();

        config.table = opt.table_name;
        config.timestamp = opt.timestamp;
        config.templates.append(&mut tokens);
        config
    };

    let (tx, mut rx): (
        mpsc::UnboundedSender<InputToken>,
        mpsc::UnboundedReceiver<InputToken>,
    ) = mpsc::unbounded_channel();
    let templates = config.templates.clone();
    let app = App::new_with_config(tx, config).await?;
    let _ret = tokio::join!(app.input_handler(), app.parse_handler(&mut rx, templates));
    let query = opt
        .query
        .unwrap_or(format!("SELECT * FROM {};", app.table_name()));
    let result = app.execute(query.as_str()).await?;

    match result {
        Some(gluesql::Payload::Select {
            labels: l,
            rows: row,
        }) => {
            let f = |r: &gluesql::data::Value| match r {
                gluesql::data::Value::Str(s) => (*s).clone(),
                _ => String::default(),
            };

            let records: DB = row
                .iter()
                .map(|r| {
                    l.clone()
                        .into_iter()
                        .zip(r.iter().map(f).collect::<Vec<String>>())
                        .collect::<BTreeMap<_, _>>()
                })
                .collect::<Vec<_>>();
                
            output::print(std::io::stdout(), &records, opt.output_type.unwrap())?;
        }
        _ => {}
    };

    Ok(())
}
