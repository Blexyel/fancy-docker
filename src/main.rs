use chrono::TimeZone;
use clap::{Parser, arg};
use reqwest::Client;
use serde::Deserialize;
use tabled::{Table, Tabled, settings::Style};

#[derive(Parser, Debug)]
struct Args {
    #[arg(short, long, help = "Do not truncate output")]
    no_truncate: bool,
}

#[derive(Tabled, Debug)]
struct Docker {
    id: String,
    image: String,
    name: String,
    command: String,
    created: String,
    status: String,
    ports: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[allow(non_snake_case)]
struct DockerOutput {
    #[serde(rename = "Id")]
    id: String,
    #[serde(rename = "Image")]
    image: String,
    #[serde(rename = "Names")]
    names: Vec<String>,
    #[serde(rename = "Command")]
    command: String,
    #[serde(rename = "Created")]
    created_at: i64,
    #[serde(rename = "Status")]
    status: String,
    #[serde(rename = "Ports")]
    ports: Vec<Ports>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct Ports {
    #[serde(rename = "IP")]
    ip: Option<String>,
    #[serde(rename = "PrivatePort")]
    private_port: Option<u16>,
    #[serde(rename = "PublicPort")]
    public_port: Option<u16>,
    #[serde(rename = "Type")]
    port_type: Option<String>,
}

fn convert_date_thingi(created_at: i64) -> String {
    let secs = if created_at.abs() > 1_000_000_000_000 {
        created_at / 1000
    } else {
        created_at
    };

    match chrono::Utc.timestamp_opt(secs, 0) {
        chrono::LocalResult::Single(dt) => chrono_humanize::HumanTime::from(dt).to_string(),
        _ => created_at.to_string(),
    }
}

async fn get_containers(truncate: bool) -> Vec<Docker> {
    let builder = Client::builder();
    let output: Vec<DockerOutput>;
    let url = dotenvy::var("DOCKER_URL").unwrap_or("http://localhost".to_string());
    let unix = dotenvy::var("DOCKER_UNIX").unwrap_or("/var/run/docker.sock".to_string());
    if unix.is_empty() {
        let http = builder.http1_only().build().expect("Failed to build client");

        let res = http
            .get(format!("{}/containers/json", url))
            .send()
            .await
            .expect("Failed to send request")
            .json::<Vec<DockerOutput>>()
            .await
            .expect("Failed to parse JSON response (are you sure the Docker daemon is running?)");

        output = res;
    } else {
        let unix = builder
            .unix_socket(dotenvy::var("DOCKER_UNIX").unwrap_or("/var/run/docker.sock".to_string()))
            .build()
            .expect("Failed to build client");

        let res = unix
            .get(format!("{}/containers/json", url))
            .send()
            .await
            .expect("Failed to send request")
            .json::<Vec<DockerOutput>>()
            .await
            .expect("Failed to parse JSON response (are you sure the Docker daemon is running?)");

        output = res;
    }

    let mut vec = Vec::new();

    for d in &output {
        let mut ports = String::new();
        for p in &d.ports {
            let port_entry = if truncate {
                #[allow(clippy::if_same_then_else)]
                if p.private_port.unwrap_or_default() == p.public_port.unwrap_or_default() {
                    format!(
                        "{}/{}",
                        p.private_port.unwrap_or_default(),
                        p.port_type.as_deref().unwrap_or_default()
                    )
                } else if p.private_port.is_some() && p.public_port.is_none() {
                    format!(
                        "{}/{}",
                        p.private_port.unwrap_or_default(),
                        p.port_type.as_deref().unwrap_or_default()
                    )
                } else {
                    format!(
                        "{}->{}/{}",
                        p.private_port.unwrap_or_default(),
                        p.public_port.unwrap_or_default(),
                        p.port_type.as_deref().unwrap_or_default()
                    )
                }
            } else {
                format!(
                    "{}:{}->{}/{}",
                    p.ip.as_deref().unwrap_or_default(),
                    p.private_port.unwrap_or_default(),
                    p.public_port.unwrap_or_default(),
                    p.port_type.as_deref().unwrap_or_default()
                )
            };
            if !ports.is_empty() {
                ports.push_str(", ");
            }
            ports.push_str(&port_entry);
        }

        let image = if !truncate {
            d.image.clone()
        } else {
            d.image.split('@').next().unwrap_or(&d.image).to_string()
        };

        let docker = Docker {
            id: truncate_string(d.id.clone(), 12, truncate),
            image: truncate_string(
                image,
                37,
                truncate,
            ),
            name: truncate_string(d.names[0].clone(), 20, truncate),
            command: truncate_string(d.command.clone(), 30, truncate),
            created: convert_date_thingi(d.created_at),
            status: d.status.clone(),
            ports,
        };
        vec.push(docker);
    }

    vec
}

fn truncate_string(string: String, length: usize, apply: bool) -> String {
    if apply && string.len() > length {
        string.chars().take(length).collect::<String>()
    } else {
        string
    }
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let mut table = Table::new(get_containers(!args.no_truncate).await);
    table.with(Style::rounded());

    println!("{}", table);
}
