// Standard library imports
use std::sync::Arc;
use std::{collections::HashMap, fmt};

// Third-party library imports
use async_trait::async_trait;
use hex::encode;
use ice_frost::{
    keys::{GroupVerifyingKey, IndividualSigningKey, IndividualVerifyingKey},
    parameters::ThresholdParameters,
    sign::{
        PartialThresholdSignature, PublicCommitmentShareList, SecretCommitmentShareList,
        SignatureAggregator,
    },
    testing::Secp256k1Sha256,
    CipherSuite, ToBytes,
};
use libp2p::{Multiaddr, PeerId};
use sha2::{Digest, Sha256};
use thiserror::*;
use tokio::sync::Mutex;

// Project-specific imports
use messages::{
    actor_type::ActorType,
    message::{AppStateChangeMessage, GossipEngineMessage, Message},
    NETWORK_TOPIC, VERIFIER_TOPIC,
};
use methods::{TX_PROVER_ELF, TX_PROVER_ID};
use network::utils::NodeType;
use ractor::{Actor, ActorProcessingErr, ActorRef};
use risc0_zkvm::{default_prover, ExecutorEnv, InnerReceipt, VerifierContext};
use tx_utils::utils::{EthAccount, Transaction};

// Local crate imports
use crate::cast_message;

type ActiveProvers = Arc<Mutex<Vec<(PeerId, Multiaddr)>>>;

/// A generic error type to propagate errors from this actor
/// and other actors that interact with it
#[derive(Debug, Clone, Error)]
pub enum AppStateEngineError {
    #[error("Error occurred in app engine: {0}")]
    Custom(String),
}

impl Default for AppStateEngineError {
    fn default() -> Self {
        AppStateEngineError::Custom("AppStateEngine unable to acquire actor".to_string())
    }
}

/// The actor struct for the AppState Engine actor
#[derive(Clone, Debug, Default)]
pub struct AppStateEngineActor;

impl AppStateEngineActor {
    pub fn new() -> Self {
        Self
    }
}

pub struct CommonState {
    pub peer_id: PeerId,
    pub group_key: Option<GroupVerifyingKey<Secp256k1Sha256>>,
    pub signing_key: Option<IndividualSigningKey<Secp256k1Sha256>>,
    pub sk_list: Option<SecretCommitmentShareList<Secp256k1Sha256>>,
    pub pk_list: Option<PublicCommitmentShareList<Secp256k1Sha256>>,
    pub accounts: HashMap<String, tx_utils::utils::EthAccount>,
}

pub enum NodeState {
    Relay {
        common_state: CommonState,
        active_provers: ActiveProvers,
        n: u32, // Number of nodes with keyshares
        t: u32, // Threshold of nodes
    },
    Prover {
        common_state: CommonState,
        my_node_idx: u32,
        n: u32, // Number of nodes with keyshares
        t: u32, // Threshold of nodes
        shares: HashMap<
            u32,
            (
                PublicCommitmentShareList<Secp256k1Sha256>,
                IndividualVerifyingKey<Secp256k1Sha256>,
            ),
        >,
    },
    Verifier {
        common_state: CommonState,
        shares: HashMap<
            u32,
            (
                PublicCommitmentShareList<Secp256k1Sha256>,
                IndividualVerifyingKey<Secp256k1Sha256>,
            ),
        >,
        data: HashMap<String, Vec<PartialThresholdSignature<Secp256k1Sha256>>>,
    },
}

pub struct AppEngineState {
    pub node_type: NodeType,
    pub state: Option<NodeState>,
}

/// TODO!
impl fmt::Debug for AppEngineState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppEngineState")
            .field("peer_id", &self.node_type)
            .finish()
    }
}

impl AppEngineState {}

#[async_trait]
impl Actor for AppStateEngineActor {
    type Msg = AppStateChangeMessage;
    type State = AppEngineState;
    type Arguments = (PeerId, NodeType);

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        args: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        let seed = 123;
        let num_accounts = 5;
        let accounts = tx_utils::utils::simulate_eth_accounts(seed, num_accounts);
        let common_state = CommonState {
            peer_id: args.0,
            group_key: None,
            signing_key: None,
            sk_list: None,
            pk_list: None,
            accounts: HashMap::new(),
        };
        let state = if args.1 == NodeType::Relay {
            Some(NodeState::Relay {
                common_state,
                active_provers: Arc::new(Mutex::new(Vec::new())),
                n: 0,
                t: 0,
            })
        } else if args.1 == NodeType::Prover {
            Some(NodeState::Prover {
                common_state,
                my_node_idx: 0,
                n: 0,
                t: 0,
                shares: HashMap::new(),
            })
        } else if args.1 == NodeType::Verifier {
            cast_message!(
                ActorType::GossipEngine,
                GossipEngineMessage::Subscribe(VERIFIER_TOPIC.to_string())
            );
            Some(NodeState::Verifier {
                common_state,
                shares: HashMap::new(),
                data: HashMap::new(),
            })
        } else {
            println!("Appstate not supported for NodeType: {:?}", args.1);
            None
        };

        println!("Simulated Ethereum Accounts:");
        for (address, account) in accounts.iter() {
            println!("{:?}: {:?}", address, account);
        }
        Ok(AppEngineState {
            node_type: args.1,
            state,
        })
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            AppStateChangeMessage::UpdatedThreshold(new_n, new_t) => {
                update_threshold(new_n, new_t, state);
            }
            AppStateChangeMessage::UpdateElectedProvers(request) => {
                update_elected_provers(request, state).await;
            }
            AppStateChangeMessage::UpdateNodeIdx(idx) => {
                update_node_idx(&mut state.state, idx);
            }
            AppStateChangeMessage::StoreKeyShares(group_key, signing_key, sk_list, pk_list) => {
                store_group_key_and_shares(
                    &mut state.state,
                    group_key,
                    signing_key,
                    sk_list,
                    pk_list,
                );
            }
            AppStateChangeMessage::StoreVerificationShares(idx, (p_shares, p_k, g_k)) => {
                store_verification_shares(&mut state.state, idx, p_shares, p_k, g_k);
            }
            AppStateChangeMessage::ProcessTxn(transactions) => {
                prove(&mut state.state, transactions);
            }
            AppStateChangeMessage::ProcessPartialSig(
                account_state_root_hash,
                reciept,
                partial_signature,
            ) => {
                process_partial_signature(
                    &mut state.state,
                    account_state_root_hash,
                    reciept,
                    partial_signature,
                );
            }
        }

        Ok(())
    }
}

fn update_threshold(new_n: u32, new_t: u32, state: &mut AppEngineState) {
    if let Some(NodeState::Relay { n, t, .. }) | Some(NodeState::Prover { n, t, .. }) =
        &mut state.state
    {
        *n = new_n;
        *t = new_t;
        println!("Updated N: {}, T: {}", n, t);
    }
}

async fn update_elected_provers(
    request: Vec<(usize, PeerId, Multiaddr)>,
    state: &mut AppEngineState,
) {
    if let Some(NodeState::Relay { active_provers, .. }) = &mut state.state {
        let mut active_provers = active_provers.lock().await;
        *active_provers = request.into_iter().map(|(_, pid, ma)| (pid, ma)).collect();
        println!("Updated active provers");
    }
}

fn update_node_idx(state: &mut Option<NodeState>, idx: u32) {
    if let Some(NodeState::Prover { my_node_idx, .. }) = state {
        *my_node_idx = idx;
    }
}

fn store_group_key_and_shares(
    state: &mut Option<NodeState>,
    group_key: GroupVerifyingKey<Secp256k1Sha256>,
    sk: IndividualSigningKey<Secp256k1Sha256>,
    sk_list: SecretCommitmentShareList<Secp256k1Sha256>,
    pk_list: PublicCommitmentShareList<Secp256k1Sha256>,
) {
    if let Some(NodeState::Prover {
        common_state,
        shares,
        my_node_idx,
        ..
    }) = state
    {
        common_state.group_key = Some(group_key);
        common_state.signing_key = Some(sk.clone());
        common_state.sk_list = Some(sk_list);
        shares.insert(*my_node_idx, (pk_list, sk.to_public()));
    }
}

fn store_verification_shares(
    state: &mut Option<NodeState>,
    idx: u32,
    p_shares: PublicCommitmentShareList<Secp256k1Sha256>,
    p_k: IndividualVerifyingKey<Secp256k1Sha256>,
    group_key: GroupVerifyingKey<Secp256k1Sha256>,
) {
    if let Some(NodeState::Verifier {
        common_state,
        shares,
        ..
    })
    | Some(NodeState::Prover {
        common_state,
        shares,
        ..
    }) = state
    {
        shares.insert(idx, (p_shares, p_k));
        if common_state.group_key.is_none() {
            common_state.group_key = Some(group_key);
        }
    }
}

fn process_partial_signature(
    state: &mut Option<NodeState>,
    account_state_root_hash: String,
    reciept: InnerReceipt,
    partial_signature: PartialThresholdSignature<Secp256k1Sha256>,
) {
    if let Some(NodeState::Verifier {
        data,
        common_state,
        shares,
        ..
    }) = state
    {
        if reciept.verify_integrity_with_context(&VerifierContext::default()).is_ok() {
            println!("Proof Verified");

            let entry = data.entry(account_state_root_hash.clone());
            match entry {
                std::collections::hash_map::Entry::Occupied(mut o) => {
                    o.get_mut().push(partial_signature);
                }
                std::collections::hash_map::Entry::Vacant(v) => {
                    v.insert(vec![partial_signature]);
                }
            }

            if let Some(vec) = data.get(&account_state_root_hash) {
                println!("Received signatures: {:?}", vec.len());

                if vec.len() >= 3 {
                    println!("Proceed for threshold verification");

                    let mut aggregator = SignatureAggregator::new(
                        ThresholdParameters::new(3, 2).unwrap(),
                        common_state.group_key.clone().unwrap(),
                        account_state_root_hash.as_bytes(),
                    );

                    for (idx, (a, b)) in shares.iter() {
                        aggregator.include_signer(*idx, a.commitments[0], &b).unwrap();
                    }

                    for partial_sig in vec.iter() {
                        aggregator.include_partial_signature(partial_sig);
                    }

                    let message_hash = Secp256k1Sha256::h4(account_state_root_hash.as_bytes());
                    let group_key = common_state.group_key.clone().unwrap();
                    let aggregator = aggregator.finalize().unwrap();
                    let threshold_signature = aggregator.aggregate().unwrap();

                    let verification_result1 = threshold_signature.verify(&group_key, &message_hash);
                    let verification_result2 = group_key.verify_signature(&threshold_signature, &message_hash);

                    println!(
                        "Threshold Signature: {:?}",
                        encode(threshold_signature.to_bytes().unwrap())
                    );

                    assert!(verification_result1.is_ok());
                    assert!(verification_result2.is_ok());
                }
            }
        }
    }
}


fn prove(state: &mut Option<NodeState>, transactions: Vec<Transaction>) {
    if let Some(NodeState::Prover { common_state, n, t, shares, .. }) = state {
        let env = ExecutorEnv::builder()
            .write(&transactions).unwrap()
            .write(&common_state.accounts).unwrap()
            .build().unwrap();

        let receipt = default_prover().prove(env, TX_PROVER_ELF).unwrap();
        let (account_state_root_hash, changed_accounts): (String, HashMap<String, EthAccount>) = 
            receipt.journal.decode().unwrap();

        if receipt.verify(TX_PROVER_ID).is_ok() {
            common_state.accounts = changed_accounts;
        }

        println!("Value: {:?}", receipt.inner.verify_integrity_with_context(&VerifierContext::default()));

        let group_key = common_state.group_key.clone().unwrap();
        let secret_comshares = common_state.sk_list.as_ref().unwrap();

        let mut aggregator = SignatureAggregator::new(
            ThresholdParameters::new(*n, *t).unwrap(),
            group_key.clone(),
            account_state_root_hash.as_bytes(),
        );

        for (idx, (a, b)) in shares.iter() {
            aggregator.include_signer(*idx, a.commitments[0], b).unwrap();
        }

        let message_hash = Secp256k1Sha256::h4(account_state_root_hash.as_bytes());
        let signers = aggregator.signers();
        let partial_signature = common_state.signing_key.as_ref().unwrap().clone().sign(
            &message_hash,
            &group_key,
            &mut secret_comshares.clone(),
            0,
            signers,
        ).unwrap();

        println!("Partial signature: {:?}", partial_signature);
        let receipt = InnerReceipt::Fake {
            claim: receipt.inner.get_claim().unwrap(),
        };

        println!("Sending Fake receipt: {:?}", receipt);
        let sig = partial_signature.to_bytes().unwrap();
        cast_message!(
            ActorType::GossipEngine,
            GossipEngineMessage::Gossip(
                Message::BroadcastPartialSig(account_state_root_hash, receipt, sig),
                NETWORK_TOPIC.to_string(),
            )
        );
    }
}

pub fn generate_merkle_root(transaction_hashes: &[String]) -> String {
    let concatenated_hashes = transaction_hashes.concat();
    let mut hasher = Sha256::new();
    hasher.update(concatenated_hashes.as_bytes());
    encode(hasher.finalize())
}
