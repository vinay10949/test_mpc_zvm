use actors::{
    app_state_actor::AppStateEngineActor,
    dkg_engine_actor::DKGEngineActor,
    election_actor::ElectionEngineActor,
    gossip_engine_actor::{GossipEngineActor, GossipEngineListener},
};
use clap::Parser;
use jsonrpsee::server::ServerBuilder as RpcServerBuilder;
use libp2p::{Multiaddr, PeerId};
use messages::{actor_type::ActorType, message::Message};
use network::utils::{fetch_kp_port, generate_kp, validate_port, NodeType};
use ractor::actor::Actor;
use rpc::{MpcRpcServer, RpcServerImpl};

/// Arguments for the program
#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    /// Idx of the node from the keypairs.json file
    #[arg(long)]
    idx: Option<u8>,

    /// Idx of the relay node from the keypairs.json file
    #[arg(long)]
    relay_idx: Option<u8>,

    ///Port
    #[arg(short, long)]
    port: Option<String>,

    /// Flag to generate Key pair
    #[arg(short, long)]
    gen: Option<u8>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    if let Some(gen_args) = args.gen {
        println!("Generating Keypairs started");
        generate_kp(gen_args);
        println!("Generating Keypairs finished");
        std::process::exit(0);
    }

    let (keypair, port, node_type, multi_addr) = fetch_kp_port(args.idx.unwrap());
    let (bootstrap_keypair, _, _, bootstrap_multi_addr) = fetch_kp_port(0);
    let (relay_keypair, _, _, relay_multiaddr) = fetch_kp_port(1);
    let local_peer_id = keypair.public().to_peer_id();
    validate_port(&port);

    let (gossip_actor_sender_tx, engine_receiver_tx) = tokio::sync::mpsc::unbounded_channel::<(
        Message,
        String,
        Option<Vec<(PeerId, Multiaddr)>>,
    )>();
    let (engine_sender_tx, gossip_actor_receiver_tx) = tokio::sync::mpsc::unbounded_channel::<(
        Message,
        String,
        Option<Vec<(PeerId, Multiaddr)>>,
    )>();

    println!(
        "\n||| ====Running node:====\n||| - Node Type: {:?}\n||| - MultiAddr: {:?}\n||| - Port: {:?}\n||| ============================",
        node_type,
        multi_addr,
        port
    );
    let (_gossip_engine_actor_ref, handle) = Actor::spawn(
        Some(ActorType::GossipEngine.to_string()),
        GossipEngineActor::new(),
        (gossip_actor_sender_tx, gossip_actor_receiver_tx),
    )
    .await
    .map_err(Box::new)?;

    let (_election_handle_ref, _) = Actor::spawn(
        Some(ActorType::ElectionEngine.to_string()),
        ElectionEngineActor::new(),
        local_peer_id,
    )
    .await
    .map_err(Box::new)?;

    let (_dkg_handle_ref, _) = Actor::spawn(
        Some(ActorType::DkgEngine.to_string()),
        DKGEngineActor::new(),
        (),
    )
    .await
    .map_err(Box::new)?;

    let rpc = RpcServerImpl::new();
    let server = RpcServerBuilder::default()
        .build(format!(
            "0.0.0.0:{}",
            args.port.expect("Provide Port for RPC")
        ))
        .await
        .map_err(Box::new)?;

    let server_handle = server.start(rpc.into_rpc());

    let mut gossip_engine_listener = if node_type == NodeType::Bootstrap {
        GossipEngineListener::new(
            keypair,
            None,
            port,
            engine_sender_tx,
            engine_receiver_tx,
            node_type,
            relay_keypair.public().to_peer_id(),
            relay_multiaddr,
        )
        .await
        .unwrap()
    } else {
        GossipEngineListener::new(
            keypair,
            Some(vec![(
                bootstrap_keypair.public().to_peer_id(),
                bootstrap_multi_addr,
            )]),
            port,
            engine_sender_tx,
            engine_receiver_tx,
            node_type,
            relay_keypair.public().to_peer_id(),
            relay_multiaddr,
        )
        .await
        .unwrap()
    };

    let (_app_state_engine_actor_ref, _) = Actor::spawn(
        Some(ActorType::AppStateEngine.to_string()),
        AppStateEngineActor::new(),
        (local_peer_id, node_type),
    )
    .await
    .map_err(Box::new)?;

    tokio::spawn(server_handle.stopped());
    let _ = tokio::spawn(async move {
        gossip_engine_listener.start().await;
    })
    .await;

    handle.await.unwrap();

    Ok(())
}
