use cursive::traits::Resizable;
use cursive::view::Nameable;
use cursive::views::{Dialog, EditView, LinearLayout, TextView};
use cursive::{Cursive, CursiveExt};
use jsonrpsee::http_client::HttpClientBuilder;
use rpc::MpcRpcClient;
use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime;
use tx_utils::utils::{simulate_eth_accounts, Transaction};

fn main() {
    let seed = 123;
    let num_accounts = 5;
    let accounts = simulate_eth_accounts(seed, num_accounts);

    println!("Simulated Ethereum Accounts:");
    for (address, account) in accounts.iter() {
        println!("{:?}: {:?}", address, account);
    }
    let mut siv = Cursive::default();
    let txns = Arc::new(Mutex::new(Vec::new()));
    let cloned_1 = txns.clone();
    let cloned_2 = txns.clone();

    siv.add_layer(
        Dialog::new()
            .title("Send Transaction")
            .content(
                LinearLayout::vertical()
                    .child(TextView::new("From:"))
                    .child(EditView::new().with_name("from").fixed_width(20))
                    .child(TextView::new("To:"))
                    .child(EditView::new().with_name("to").fixed_width(20))
                    .child(TextView::new("Amount:"))
                    .child(EditView::new().with_name("amount").fixed_width(20)),
            )
            .button("Add", move |s| {
                let from = s
                    .call_on_name("from", |v: &mut EditView| v.get_content())
                    .unwrap();
                let to = s
                    .call_on_name("to", |v: &mut EditView| v.get_content())
                    .unwrap();
                let amount = s
                    .call_on_name("amount", |v: &mut EditView| v.get_content())
                    .unwrap();
                // Check if the `from` address is valid
                if !accounts.contains_key(&(*from).clone()) {
                    s.add_layer(Dialog::info("Invalid From Address"));
                } else if !accounts.contains_key(&(*to).clone()) {
                    // Check if the `to` address is valid
                    s.add_layer(Dialog::info("Invalid To Address"));
                } else if let Ok(amount) = amount.parse::<u64>() {
                    let txn = Transaction {
                        from: (*from).clone(),
                        to: (*to).clone(),
                        amount,
                    };
                    cloned_1.lock().unwrap().push(txn);
                    s.add_layer(Dialog::info("Transaction Added!"));
                } else {
                    // If none of the above conditions match, the amount is invalid
                    s.add_layer(Dialog::info("Invalid amount!"));
                }
            })
            .button("Send", move |s| {
                // Await the send_transaction function within the closure
                send_transaction(cloned_2.clone());

                s.add_layer(Dialog::info("Transactions Sent!"));
            })
            .button("Quit", |s| s.quit()),
    );

    siv.run();
}

fn send_transaction(txns: Arc<Mutex<Vec<Transaction>>>) {
    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        println!("Connectin");
        let client = HttpClientBuilder::default()
            .build("http://localhost:9095")
            .expect("Failed to build client");

        let response = client.process_txn(txns.lock().unwrap().clone()).await;
        println!("Response :{:?}", response)
    });
}
