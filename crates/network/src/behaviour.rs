use libp2p::identity::Keypair;
use libp2p::kad::store::MemoryStore;
use libp2p::{gossipsub, kad, swarm::NetworkBehaviour};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::time::Duration;
use tokio::io;

#[derive(NetworkBehaviour)]
pub struct MyBehaviour {
    pub gossipsub: gossipsub::Behaviour,
    pub kademlia: kad::Behaviour<MemoryStore>,
}

impl MyBehaviour {
    pub fn new(key: Keypair) -> Result<Self, Box<dyn std::error::Error>> {
        // Create a Gossipsub topic
        let peer_id = key.public().to_peer_id();
        let message_id_fn = |message: &gossipsub::Message| {
            let mut s = DefaultHasher::new();
            message.data.hash(&mut s);
            gossipsub::MessageId::from(s.finish().to_string())
        };
        // Set a custom gossipsub configuration
        let gossipsub_config = gossipsub::ConfigBuilder::default()
            .heartbeat_interval(Duration::from_secs(10))
            .validation_mode(gossipsub::ValidationMode::Strict)
            .duplicate_cache_time(Duration::from_millis(10))
            .message_id_fn(message_id_fn)
            .build()
            .map_err(|msg| io::Error::new(io::ErrorKind::Other, msg))?; // Proper error handling required

        // Create a Gossipsub behaviour
        let gossipsub = gossipsub::Behaviour::new(
            gossipsub::MessageAuthenticity::Signed(key.clone()),
            gossipsub_config,
        )?;

        // Create a Kademlia behaviour
        let mut cfg = kad::Config::default();
        cfg.set_query_timeout(Duration::from_secs(5 * 60));
        let store = kad::store::MemoryStore::new(peer_id);
        let kademlia = kad::Behaviour::with_config(peer_id, store, cfg);

        Ok(Self {
            gossipsub,
            kademlia,
        })
    }
}
