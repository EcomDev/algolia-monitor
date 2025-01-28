use clap::{arg, Parser};
use serde_json::{to_string, Value};
use std::time::Duration;
use tokio::time::sleep;

/// Algolia index size monitor
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Application ID
    app_id: String,

    /// Algolia API key
    key: String,

    /// Name of the index to monitor
    index_name: String,

    #[arg(short, long, default_value = "0")]
    expected_records: u64,

    #[arg(short, long, default_value = "30")]
    delay: u64,
}

impl Args {
    fn create_client(&self) -> AlgoliaClient {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("x-algolia-application-id", self.app_id.parse().unwrap());
        headers.insert("x-algolia-api-key", self.key.parse().unwrap());
        headers.insert("content-type", "application/json".parse().unwrap());
        headers.insert("accept", "application/json".parse().unwrap());

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .unwrap();

        AlgoliaClient {
            client,
            base_url: format!("https://{}-dsn.algolia.net/1/", self.app_id),
            index_name: self.index_name.clone(),
        }
    }
}

struct AlgoliaClient {
    client: reqwest::Client,
    base_url: String,
    index_name: String,
}

struct AlgoliaLog {
    timestamp: String,
    message: String,
    method: String,
}

impl AlgoliaLog {
    fn from_json(json: &Value) -> AlgoliaLog {
        AlgoliaLog {
            timestamp: json["timestamp"].as_str().unwrap().to_string(),
            message: to_string(json).unwrap(),
            method: json["method"].as_str().unwrap().to_string(),
        }
    }

    fn is_update(&self) -> bool {
        self.method != "GET"
    }
}

impl AlgoliaClient {
    async fn total_records(&self) -> Result<u64, reqwest::Error> {
        let request = self
            .client
            .post(format!(
                "{}indexes/{}/query",
                self.base_url, self.index_name
            ))
            .body(r#"{"params":"hitsPerPage=0&getRankingInfo=0&query=*"}"#)
            .build()?;

        let response = self.client.execute(request).await?;
        let response: Value = response.json().await?;

        let value = response
            .get("nbHits")
            .map(|v| v.as_u64().unwrap_or(0))
            .unwrap_or(0);

        Ok(value)
    }

    async fn get_logs(&self) -> Result<Vec<AlgoliaLog>, reqwest::Error> {
        let url = format!(
            "{}logs?indexName={}&type={}&offset=1&length=1000",
            self.base_url, self.index_name, "build"
        );

        let request = self.client.get(url).build()?;

        let response = self.client.execute(request).await?;
        let response: Value = response.json().await?;

        let logs = match response.get("logs") {
            Some(Value::Array(logs)) => logs,
            _ => return Ok(vec![]),
        };

        Ok(logs.iter().map(AlgoliaLog::from_json).collect())
    }
}

#[tokio::main]
async fn main() -> Result<(), reqwest::Error> {
    let args = Args::parse();
    let client = args.create_client();
    let mut last_log_timestamp = "0000-00-00T00:00:00.000Z".to_string();
    let expected_records = match args.expected_records {
        0 => client.total_records().await?,
        _ => args.expected_records,
    };

    eprintln!(
        "Monitoring for record count changes, started with expected value of {expected_records}"
    );

    loop {
        let total_records = client.total_records().await?;
        if (expected_records as i64 - total_records as i64).abs() > 1000 {
            eprintln!("Records count difference is more than 1000, waiting for logs...");
            let logs = client.get_logs().await?;
            for log in &logs {
                if log.timestamp > last_log_timestamp && log.is_update() {
                    if log.message.contains("delete") {
                        println!("DELETE:{}", log.message);
                        continue;
                    }

                    println!("UPDATE:{}", log.message);
                }
            }
            for log in &logs {
                if log.timestamp > last_log_timestamp {
                    last_log_timestamp = log.timestamp.to_string();
                }
            }
        }

        sleep(Duration::from_secs(args.delay)).await;
    }
}
