use crate::{catalog::discover, query::compile, Catalog, Query, Result};
use data_model::document::{Document, Record};
use postgres::{Client, NoTls};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    time::Duration,
};

pub const DEFAULT_URL: &str = "postgresql://ultorg:ultorg@localhost:55432/ultorg";

pub fn connect(url: &str) -> Result<Client> {
    let mut config: postgres::Config = url.parse()?;
    config.connect_timeout(Duration::from_secs(5));
    let mut client = config.connect(NoTls)?;
    client.batch_execute(
        "SET statement_timeout='10s'; SET default_transaction_read_only=on; SET timezone='UTC'",
    )?;
    Ok(client)
}

pub fn execute(client: &mut Client, query: &Query, catalog: &Catalog) -> Result<Document> {
    let compiled = compile(query, catalog)?;
    let params: Vec<&(dyn postgres::types::ToSql + Sync)> = compiled
        .params
        .iter()
        .map(|s| s as &(dyn postgres::types::ToSql + Sync))
        .collect();
    let mut rows = vec![];
    for row in client.query(&compiled.sql, &params)? {
        let json: serde_json::Value = row.get(0);
        rows.push(serde_json::from_value::<Record>(json)?);
    }
    fn trim(rows: &mut Vec<Record>, limit: usize) -> bool {
        let mut truncated = rows.len() > limit;
        rows.truncate(limit);
        for row in rows {
            for children in row.children.values_mut() {
                truncated |= trim(children, limit);
            }
        }
        truncated
    }
    let truncated = trim(&mut rows, query.limit);
    Ok(Document {
        fields: query.root.fields(),
        rows,
        truncated,
    })
}

pub enum Response {
    Catalog(Catalog),
    Data(Document),
}
struct Running {
    cancelled: AtomicBool,
    token: Mutex<Option<postgres::CancelToken>>,
}
pub struct Worker {
    url: String,
    generation: u64,
    tx: mpsc::Sender<(u64, std::result::Result<Response, String>)>,
    rx: mpsc::Receiver<(u64, std::result::Result<Response, String>)>,
    running: Option<Arc<Running>>,
}
impl Worker {
    pub fn new(url: String) -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            url,
            generation: 0,
            tx,
            rx,
            running: None,
        }
    }
    pub fn cancel(&mut self) {
        self.generation += 1;
        if let Some(running) = self.running.take() {
            running.cancelled.store(true, Ordering::SeqCst);
            if let Some(token) = running.token.lock().unwrap().take() {
                // Each request owns a distinct connection: cancellation cannot hit a newer query.
                std::thread::spawn(move || {
                    let _ = token.cancel_query(NoTls);
                });
            }
        }
    }
    pub fn submit(&mut self, query: Option<(Query, Catalog)>) {
        self.cancel();
        let generation = self.generation;
        let running = Arc::new(Running {
            cancelled: AtomicBool::new(false),
            token: Mutex::new(None),
        });
        self.running = Some(running.clone());
        let tx = self.tx.clone();
        let url = self.url.clone();
        std::thread::spawn(move || {
            let result = (|| -> Result<Response> {
                let mut client = connect(&url)?;
                {
                    let mut token = running.token.lock().unwrap();
                    if running.cancelled.load(Ordering::SeqCst) {
                        return Err(crate::Error::Invalid("Cancelled".into()));
                    }
                    *token = Some(client.cancel_token());
                }
                if running.cancelled.load(Ordering::SeqCst) {
                    return Err(crate::Error::Invalid("Cancelled".into()));
                }
                match query {
                    Some((q, c)) => execute(&mut client, &q, &c).map(Response::Data),
                    None => discover(&mut client).map(Response::Catalog),
                }
            })()
            .map_err(|e| match &e {
                crate::Error::Postgres(error) => error
                    .as_db_error()
                    .map(|db| db.message().to_string())
                    .unwrap_or_else(|| error.to_string()),
                _ => e.to_string(),
            });
            let _ = tx.send((generation, result));
        });
    }
    pub fn poll(&mut self) -> Option<std::result::Result<Response, String>> {
        while let Ok((generation, result)) = self.rx.try_recv() {
            if generation == self.generation {
                self.running = None;
                return Some(result);
            }
        }
        None
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.cancel();
    }
}
