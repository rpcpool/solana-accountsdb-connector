use {
    clap::Parser,
    futures::stream::StreamExt,
    solana_geyser_grpc_public::grpc::proto::{geyser_client::GeyserClient, SubscribeRequest},
    tonic::Request,
};

#[derive(Debug, Parser)]
#[clap(author, version, about)]
struct Args {
    /// Service endpoint
    #[clap(short, long, default_value_t = String::from("http://127.0.0.1:10000"))]
    endpoint: String,

    /// Filter by Account Pubkey
    #[clap(short, long)]
    accounts: Vec<String>,

    /// Filter by Owner Pubkey
    #[clap(short, long)]
    owner: Vec<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let mut client = GeyserClient::connect(args.endpoint).await?;
    let request = Request::new(SubscribeRequest {
        accounts: args.accounts,
        owners: args.owner,
    });
    let response = client.subscribe(request).await?;
    let mut stream = response.into_inner();

    println!("stream opened");
    while let Some(message) = stream.next().await {
        println!("new message: {:?}", message);
    }
    println!("stream closed");

    Ok(())
}
