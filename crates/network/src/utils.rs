//! This library provides utilities for generating and managing libp2p keypairs for testing purposes.

use libp2p::identity::Keypair;
use libp2p::identity::PublicKey;
use libp2p::multiaddr::Protocol;
use libp2p::{Multiaddr, PeerId};
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::str::FromStr;

/// A struct representing a keypair name and its associated multiaddr.
#[derive(Clone, Serialize, Deserialize)]
struct KeypairName {
    name: String,
    multiaddr: String,
    node_type: NodeType,
    public_key: String,
}

#[derive(Clone, Debug, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeType {
    Bootstrap,
    Relay,
    Prover,
    Verifier,
}

/// Checks if a specific port is available for binding.
///
/// This function attempts to bind a TCP listener on the provided port. If the binding is successful,
/// it means the port is available. Otherwise, the port is considered unavailable.
///
/// # Arguments
///
/// * `port` - The port number to check.
///
/// # Returns
///
/// True if the port is available, False otherwise.
pub fn is_port_available(port: u16) -> bool {
    match TcpListener::bind(("0.0.0.0", port)) {
        Ok(listener) => {
            // The port is available because binding succeeded
            drop(listener); // Close the listener
            true
        }
        Err(_) => false, // Failed to bind, so the port is not available
    }
}

/// Generates a specified number of libp2p keypairs and stores them in a designated folder.
/// This function generates the specified number of Ed25519 keypairs.
///
/// # Arguments
///
/// * `num` - The number of keypairs to generate.
pub fn generate_kp(num: u8) {
    clear_folder("src/test_keypairs");
    let mut keypair_names = vec![];
    let mut port = 8080;
    let mut bootstrap_generated = false;
    let mut relay_generated = false;

    for i in 0..num {
        let node_type = if !bootstrap_generated {
            bootstrap_generated = true;
            NodeType::Bootstrap
        } else if !relay_generated {
            relay_generated = true;
            NodeType::Relay
        } else if i == num - 1 {
            NodeType::Verifier
        } else {
            NodeType::Prover
        };

        let keypair = libp2p::identity::Keypair::generate_secp256k1();
        let public_key = hex::encode(keypair.public().encode_protobuf());
        let encoding = keypair.to_protobuf_encoding().unwrap();
        let name = keypair.public().to_peer_id().to_string();
        let dir_path = "test_keypairs/";
        if !Path::new(dir_path).exists() {
            fs::create_dir_all(dir_path).expect("Failed to create directory");
        }
        let keypair_name = KeypairName {
            name: name.clone(),
            multiaddr: format!("/ip4/127.0.0.1/udp/{}/quic-v1", port),
            node_type,
            public_key,
        };
        port += 1;
        let mut file =
            File::create(format!("test_keypairs/{}.bin", name)).expect("Failed to create file");
        file.write_all(&encoding).expect("Failed to write to file");
        keypair_names.push(keypair_name);
    }

    let serialized_keypair_names =
        serde_json::to_string_pretty(&keypair_names).expect("Failed to serialize keypair names");

    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
        .open("test_keypairs/keypair_names.json")
        .expect("Failed to open file");

    file.write_all(serialized_keypair_names.as_bytes())
        .expect("Failed to write to file");
}

/// Fetches a list of bootstrap addresses from the stored keypairs.
/// # Returns
///
/// A vector of tuples containing `(PeerId, Multiaddr)` for each stored keypair.
pub fn fetch_bootstrap_addr() -> Vec<(PeerId, Multiaddr)> {
    let keypair_names: Vec<KeypairName> = fetch_all_keypair_names();
    keypair_names
        .into_iter()
        .map(|entry| {
            let peer_id = PeerId::from_str(&entry.name).expect("Failed to parse PeerId from name");
            let multiaddr = entry
                .multiaddr
                .parse::<Multiaddr>()
                .expect("Failed to parse Multiaddr from string");
            (peer_id, multiaddr)
        })
        .collect()
}

fn fetch_all_keypair_names() -> Vec<KeypairName> {
    let mut file: File =
        File::open("test_keypairs/keypair_names.json").expect("Failed to open keypair_names.json");

    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .expect("Failed to read keypair_names.json");

    serde_json::from_str(&contents).expect("Failed to parse keypair_names.json")
}
pub fn fetch_kp_port(idx: u8) -> (Keypair, String, NodeType, Multiaddr) {
    // Read the JSON file
    let mut file =
        File::open("test_keypairs/keypair_names.json").expect("Failed to open keypair_names.json");

    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .expect("Failed to read keypair_names.json");

    let keypair_names: Vec<KeypairName> = fetch_all_keypair_names();

    // Check if the idx is valid
    let entry = keypair_names.get(idx as usize).expect("Invalid Node Idx");
    let multiaddr: Multiaddr = entry.multiaddr.parse().unwrap();
    let mut nodeport = String::new();
    for proto in multiaddr.iter() {
        match proto {
            // If the protocol is TCP, extract the port
            Protocol::Tcp(port) => nodeport = port.to_string(),
            Protocol::Udp(port) => {
                nodeport = port.to_string();
            }
            _ => {}
        }
    }
    let mut file =
        File::open(format!("test_keypairs/{}.bin", entry.name)).expect("Failed to open file");
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .expect("Failed to read from file");

    // Import the new encoding into a new keypair
    (
        Keypair::from_protobuf_encoding(&bytes).unwrap(),
        nodeport,
        entry.node_type,
        multiaddr,
    )
}
fn clear_folder(folder_path: &str) {
    if let Ok(entries) = std::fs::read_dir(folder_path) {
        for entry in entries.flatten() {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

pub fn fetch_provers() -> Vec<(PeerId, PublicKey, Multiaddr)> {
    let mut peer_ids = vec![];
    let keypair_names: Vec<KeypairName> = fetch_all_keypair_names();
    for keypair in keypair_names.iter() {
        if keypair.node_type == NodeType::Prover {
            let peer_pub_key = PublicKey::try_decode_protobuf(
                &hex::decode(&keypair.public_key).expect("Invalid Public Key"),
            )
            .unwrap();
            peer_ids.push((
                PeerId::from_str(&keypair.name.clone()).unwrap(),
                peer_pub_key,
                Multiaddr::from_str(&keypair.multiaddr).unwrap(),
            ))
        }
    }
    peer_ids
}

pub fn validate_port(port_str: &str) -> u16 {
    match port_str.parse::<u16>() {
        Ok(port) => {
            // Check if the port is available
            if !is_port_available(port) {
                eprintln!("Error: Port {} is not available.", port);
                std::process::exit(1); // Exit the program with an error code
            }
            port
        }
        Err(err) => {
            eprintln!("Error parsing port string: {}", err);
            std::process::exit(1); // Exit the program with an error code
        }
    }
}
