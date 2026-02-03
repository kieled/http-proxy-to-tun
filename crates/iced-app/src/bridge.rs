use std::path::PathBuf;
use anyhow::Result;
use tokio::sync::{mpsc, oneshot};

/// Commands sent to the VPN backend
#[derive(Debug)]
pub enum BackendCommand {
    Connect(ConnectArgs),
    Disconnect,
}

/// Events sent from backend to frontend
#[derive(Debug, Clone)]
pub enum BackendEvent {
    Connected,
    Disconnected,
    Error(String),
}

/// Arguments for connection
#[derive(Debug, Clone)]
pub struct ConnectArgs {
    pub proxy_host: String,
    pub proxy_port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    pub tun_name: String,
    pub tun_cidr: String,
    pub killswitch: bool,
}

/// Start the backend task and return command sender and event receiver
pub async fn start_backend() -> Result<(mpsc::Sender<BackendCommand>, mpsc::Receiver<BackendEvent>)> {
    let (cmd_tx, mut cmd_rx) = mpsc::channel::<BackendCommand>(32);
    let (event_tx, event_rx) = mpsc::channel::<BackendEvent>(32);

    tokio::spawn(async move {
        let mut shutdown_tx: Option<oneshot::Sender<()>> = None;

        while let Some(cmd) = cmd_rx.recv().await {
            match cmd {
                BackendCommand::Connect(args) => {
                    tracing::info!("Backend: Connect command received");

                    // Stop any existing VPN
                    if let Some(tx) = shutdown_tx.take() {
                        let _ = tx.send(());
                        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    }

                    let (tx, rx) = oneshot::channel();
                    shutdown_tx = Some(tx);

                    // Use a channel to wait for actual connection status
                    let (ready_tx, ready_rx) = oneshot::channel::<Result<(), String>>();
                    let event_tx_clone = event_tx.clone();
                    tokio::spawn(async move {
                        match run_vpn(args, rx, ready_tx).await {
                            Ok(()) => {
                                tracing::info!("VPN task completed successfully");
                                let _ = event_tx_clone.send(BackendEvent::Disconnected).await;
                            }
                            Err(e) => {
                                tracing::error!("VPN task error: {}", e);
                                let _ = event_tx_clone.send(BackendEvent::Error(e.to_string())).await;
                            }
                        }
                    });

                    // Wait for actual connection confirmation (with timeout)
                    match tokio::time::timeout(
                        tokio::time::Duration::from_secs(30),
                        ready_rx,
                    )
                    .await
                    {
                        Ok(Ok(Ok(()))) => {
                            tracing::info!("VPN setup confirmed");
                            let _ = event_tx.send(BackendEvent::Connected).await;
                        }
                        Ok(Ok(Err(e))) => {
                            tracing::error!("VPN setup failed: {}", e);
                            let _ = event_tx.send(BackendEvent::Error(e)).await;
                        }
                        Ok(Err(_)) => {
                            // Channel closed unexpectedly
                            tracing::error!("VPN setup channel closed");
                            let _ = event_tx
                                .send(BackendEvent::Error("VPN setup failed unexpectedly".to_string()))
                                .await;
                        }
                        Err(_) => {
                            // Timeout
                            tracing::error!("VPN setup timed out");
                            let _ = event_tx
                                .send(BackendEvent::Error("Connection timed out".to_string()))
                                .await;
                        }
                    }
                }

                BackendCommand::Disconnect => {
                    tracing::info!("Backend: Disconnect command received");

                    if let Some(tx) = shutdown_tx.take() {
                        let _ = tx.send(());
                    }
                    let _ = event_tx.send(BackendEvent::Disconnected).await;
                }
            }
        }

        tracing::info!("Backend task exiting");
    });

    Ok((cmd_tx, event_rx))
}

/// Check if the system is ready for TUN device creation
fn check_tun_prerequisites() -> Result<()> {
    use std::path::Path;

    // Check if /dev/net/tun exists
    if !Path::new("/dev/net/tun").exists() {
        return Err(anyhow::anyhow!(
            "TUN device not available. Run: sudo modprobe tun"
        ));
    }

    // Check if we have CAP_NET_ADMIN by trying to read /proc/self/status
    // This is a best-effort check; actual capability verification is complex
    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        // The binary needs CAP_NET_ADMIN (bit 12)
        // If running without caps and not root, warn
        if status.contains("CapEff:") && !proxyvpn_util::is_root() {
            tracing::debug!("Running as non-root user, CAP_NET_ADMIN required");
        }
    }

    Ok(())
}

async fn run_vpn(
    args: ConnectArgs,
    shutdown: oneshot::Receiver<()>,
    ready_tx: oneshot::Sender<Result<(), String>>,
) -> Result<()> {
    use proxyvpn_cli::RunArgs;

    // Check prerequisites before attempting connection
    if let Err(e) = check_tun_prerequisites() {
        let _ = ready_tx.send(Err(e.to_string()));
        return Err(e);
    }

    let run_args = RunArgs {
        state_dir: resolve_state_dir(),
        verbose: false,
        keep_logs: false,
        dry_run: false,
        proxy_url: None,
        proxy_host: Some(args.proxy_host),
        proxy_port: Some(args.proxy_port),
        username: args.username,
        password: args.password,
        password_file: None,
        proxy_ip: vec![],
        tun_name: args.tun_name,
        tun_cidr: args.tun_cidr,
        dns: None,
        allow_dns: vec![],
        no_killswitch: !args.killswitch,
    };

    tracing::info!("Starting VPN with args: {:?}", run_args);

    // Use run_with_args_and_ready to get confirmation when setup completes
    tokio::select! {
        result = proxyvpn_app::run_with_args_and_ready(&run_args, ready_tx) => {
            match &result {
                Ok(()) => tracing::info!("VPN run completed"),
                Err(e) => tracing::error!("VPN run error: {}", e),
            }
            result
        }
        _ = shutdown => {
            tracing::info!("Shutdown signal received, stopping VPN");
            Ok(())
        }
    }
}

fn resolve_state_dir() -> PathBuf {
    let is_root = proxyvpn_util::is_root();

    if is_root {
        PathBuf::from("/run/proxyvpn")
    } else if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
        PathBuf::from(dir).join("proxyvpn")
    } else {
        std::env::temp_dir().join(format!("proxyvpn-{}", std::process::id()))
    }
}
