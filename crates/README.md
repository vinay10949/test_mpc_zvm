This scenario involves multiple parties, including Provers and Verifiers. Provers participate in a Distributed Key Generation (DKG) process to establish a Quorum Public Key and private key. Each Prover performs computations to deduct balances once a batch of transactions is received. These computations are carried out under the Zero-Knowledge Virtual Machine. Subsequently, a receipt is generated, signed, and a partial signature is created. This partial signature is then sent to the Verifier. Upon receiving a sufficient number of signatures, the Verifier performs threshold signing on the received data.


## Install ZKVM 
```bash
cargo install cargo-binstall
cargo binstall cargo-risczero
cargo risczero install
```

## Setup Steps

### 1. Generate Key Pairs

To generate key pairs for the nodes in the network, use the following command:

```bash
RUSTLOG=INFO cargo run --package node --bin mpc_node -- --idx 0 --gen 6| bunyan
```

### 2. Run the Bootstrap Node

Start the bootstrap node, which is responsible for initializing the network and coordinating the other nodes, with the following command:

```bash
RISC0_DEV_MODE=1 RUSTLOG=INFO cargo run --package node --bin mpc_node -- --idx 0 --relay-idx 1 --port 9090
```

### 3. Run the Relay Node

Next, run the relay node using the following command:

```bash
 RISC0_DEV_MODE=1 RUSTLOG=INFO cargo run --package node --bin mpc_node -- --idx 1 --relay-idx 1 --port 9095
```

The relay node helps in routing messages between nodes in the network.

### 4. Run the Prover Nodes

Finally, start the prover nodes with the following commands:

```bash
RISC0_DEV_MODE=1 cargo run --package node --bin mpc_node -- --idx 2 --relay-idx 1 --port 9092 
RISC0_DEV_MODE=1 cargo run --package node --bin mpc_node -- --idx 3 --relay-idx 1 --port 9093 
RISC0_DEV_MODE=1 cargo run --package node --bin mpc_node -- --idx 4 --relay-idx 1 --port 9094
RISC0_DEV_MODE=1  cargo run --package node --bin mpc_node -- --idx 5 --relay-idx 1 --port 9091
```

These commands will start the prover nodes and one verifier node, which are responsible for proving the authenticity of data and transactions within the network.

This uses Risc Zero ZKVM to prove computation for updating account balances.


First, start the bootstrap node. After that, start all other nodes except the relay node.

The relay node should be started last. It reads the generated keypair_json file and initiates the election process, followed by triggering the DKG.

After starting the relay node, navigate to the host directory and execute the following command:

```shell
cargo run --bin user_tui
```
This is ui to trigger batch of transactions.

