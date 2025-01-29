use clap::{arg, Parser};
use serde_json::{to_string, Value};
use std::time::Duration;
use reqwest::Error;
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

    #[arg(short, long, default_value = "false")]
    all_logs: bool,

    #[arg(short, long, default_value = "0")]
    expected_records: u64,

    #[arg(short, long, default_value = "30")]
    delay: u64,

    #[arg(long, default_value = "-1000")]
    delta: i64,
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
    message: String
}

impl AlgoliaLog {
    fn from_json(json: &Value) -> AlgoliaLog {
        AlgoliaLog {
            timestamp: json["timestamp"].as_str().unwrap().to_string(),
            message: to_string(json).unwrap(),
        }
    }

    fn is_newer(&self, timestamp: &String) -> bool {
        self.timestamp.gt(timestamp)
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

    if !args.all_logs {
        eprintln!(
            "Monitoring for record count changes, started with expected value of {expected_records}"
        );
    }

    loop {
        if args.all_logs {
            print_all_logs(&client, &mut last_log_timestamp).await?;
        } else {
            print_logs_when_records_change(&client, expected_records, args.delta, &mut last_log_timestamp).await?;
        }
        sleep(Duration::from_secs(args.delay)).await;
    }
}

async fn print_logs_when_records_change(
    client: &AlgoliaClient,
    expected_records: u64,
    delta: i64,
    last_log_timestamp: &mut String,
) -> Result<(), Error> {
    let total_records = client.total_records().await?;
    let changed_records = total_records as i64 - expected_records as i64;
    if (delta < 0 && changed_records < delta) || (delta > 0 && changed_records > delta) {
        eprintln!(
            "Records count difference is more than {} ({}), waiting for logs...",
            delta,
            changed_records
        );
        print_algolia_logs(client, last_log_timestamp).await?;
    }

    Ok(())
}


async fn print_all_logs(
    client: &AlgoliaClient,
    last_log_timestamp: &mut String,
) -> Result<(), reqwest::Error> {
    print_algolia_logs(client, last_log_timestamp).await?;
    Ok(())
}

async fn print_algolia_logs(client: &AlgoliaClient, last_log_timestamp: &mut String) -> Result<(), Error> {
    let logs = client.get_logs().await?;
    for log in &logs {
        if log.is_newer(last_log_timestamp) {
            println!("{}", log.message);
        }
    }
    for log in &logs {
        if log.is_newer(last_log_timestamp) {
            let _ = std::mem::replace(last_log_timestamp, log.timestamp.clone());
        }
    }
    Ok(())
}
