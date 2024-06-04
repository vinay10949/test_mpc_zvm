// network.rs
use crate::behaviour::MyBehaviour;
use libp2p::gossipsub::IdentTopic;
use libp2p::identity::Keypair;
use libp2p::{noise, tcp, yamux, Multiaddr, PeerId, Swarm, SwarmBuilder};
use messages::NETWORK_TOPIC;
use std::error::Error;
use std::time::Duration;

pub async fn init_swarm(
    keypair: Option<Keypair>,
    bootstrap_addresses: Option<Vec<(PeerId, Multiaddr)>>,
    port: String,
) -> Result<Swarm<MyBehaviour>, Box<dyn Error>> {
    // If keypair is provided, use it for identity
    let builder = if let Some(keypair) = keypair {
        SwarmBuilder::with_existing_identity(keypair)
    } else {
        SwarmBuilder::with_new_identity()
    };
    // Create a libp2p swarm with configuration
    let mut swarm = builder
        .with_tokio()
        .with_tcp(
            tcp::Config::default().port_reuse(true),
            noise::Config::new,
            yamux::Config::default,
        )?
        .with_quic()
        .with_behaviour(|keypair| {
            if bootstrap_addresses.is_none() {
                println!("Bootstrap Peer ID :{}", keypair.public().to_peer_id());
            }
            MyBehaviour::new(keypair.clone()).unwrap()
        })?
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
        .build();

    // Add bootstrap nodes if provided
    if let Some(ref bootstrap_addresses) = bootstrap_addresses {
        for (peer_id, multi_addr) in bootstrap_addresses {
            swarm
                .behaviour_mut()
                .kademlia
                .add_address(peer_id, multi_addr.clone());

            swarm.behaviour_mut().kademlia.bootstrap()?;
        }
    }

    // Subscribe to the topic (can be done after swarm creation)
    swarm
        .behaviour_mut()
        .gossipsub
        .subscribe(&IdentTopic::new(NETWORK_TOPIC))?;

    let listen_address = format!("/ip4/0.0.0.0/udp/{}/quic-v1", port);
    swarm.listen_on(listen_address.parse()?)?;

    Ok(swarm)
}

pub async fn subscribe(
    swarm: &mut Swarm<MyBehaviour>,
    topic: IdentTopic,
) -> Result<(), Box<dyn Error>> {
    swarm.behaviour_mut().gossipsub.subscribe(&topic)?;
    Ok(())
}

pub async fn unsubscribe(
    swarm: &mut Swarm<MyBehaviour>,
    topic: IdentTopic,
) -> Result<(), Box<dyn Error>> {
    swarm.behaviour_mut().gossipsub.unsubscribe(&topic)?;
    Ok(())
}
