use std::time::Duration;

use oneclient_core::discord::{DiscordRpc, Presence};

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let rpc = DiscordRpc::spawn(true);

    println!("idle presence for 10s...");
    std::thread::sleep(Duration::from_secs(10));

    println!("playing presence for 10s...");
    rpc.set_presence(Presence::Playing {
        cluster: "Example Cluster".to_owned(),
        mc_version: "1.8.9".to_owned(),
    });
    std::thread::sleep(Duration::from_secs(10));

    println!("disabling for 5s...");
    rpc.set_enabled(false);
    std::thread::sleep(Duration::from_secs(5));

    println!("re-enabling for 10s...");
    rpc.set_enabled(true);
    std::thread::sleep(Duration::from_secs(10));

    println!("shutting down");
    rpc.shutdown();
    std::thread::sleep(Duration::from_secs(1));
}
