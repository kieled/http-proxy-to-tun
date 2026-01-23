#[tokio::main(flavor = "multi_thread")]
async fn main() {
    if let Err(err) = proxyvpn_app::run().await {
        eprintln!("error: {:#}", err);
        std::process::exit(1);
    }
}
