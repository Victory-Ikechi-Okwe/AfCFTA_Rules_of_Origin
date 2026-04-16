use clap::Parser;
use serde::Serialize;
use std::collections::HashMap;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::UnixStream,
};

#[derive(Serialize)]
struct RtPackage {
    rule_id: String,
    document: HashMap<String, String>,
}

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    #[arg(long)]
    rule_id: String,
    #[arg(long)]
    document: String,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let f = std::fs::File::open(&args.document).expect("error opening document");
    let document: HashMap<String, String> =
        serde_json::from_reader(f).expect("parse failure");

    let pkg = RtPackage {
        rule_id: args.rule_id,
        document,
    };

    let payload = serde_json::to_vec(&pkg).unwrap();

    let mut stream = UnixStream::connect("/tmp/rs.sock")
        .await
        .expect("could not connect to Rule Reserve — is rs running?");

    stream.write_all(&payload).await.unwrap();
    stream.flush().await.unwrap();
    stream.shutdown().await.unwrap();

    let mut response = String::new();
    stream.read_to_string(&mut response).await.unwrap();

    println!("{}", response);
}
