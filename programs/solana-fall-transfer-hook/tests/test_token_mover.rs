#[allow(dead_code)]
mod helpers;

use {
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

use helpers::{
    setup, setup_mint_and_extra_metas, create_ata, mint_tokens, build_transfer_through_mover_ix,
};

#[test]
fn test_transfer_through_program_succeeds() {
    let (mut svm, payer, hook_program_id) = setup();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &hook_program_id);

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000).unwrap();

    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 1_000_000);

    let ix = build_transfer_through_mover_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &hook_program_id, 100,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();

    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "transfer through token-mover failed: {:?}", res.err());
}

#[test]
fn test_transfer_through_program_is_rate_limited() {
    let (mut svm, payer, hook_program_id) = setup();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &hook_program_id);

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000).unwrap();

    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());
    mint_tokens(&mut svm, &payer, &mint.pubkey(), &source_ata, 2_000_000);

    // Exactly the limit, through the program: allowed.
    let ix1 = build_transfer_through_mover_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &hook_program_id, 1_000_000,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix1], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let res = svm.send_transaction(tx);
    assert!(res.is_ok(), "transfer at the limit should succeed: {:?}", res.err());

    // One more base unit: the hook must still run inside our CPI and refuse.
    let ix2 = build_transfer_through_mover_ix(
        &source_ata, &dest_ata, &mint.pubkey(), &payer.pubkey(), &hook_program_id, 1,
    );
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix2], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    let err = svm
        .send_transaction(tx)
        .err()
        .expect("transfer over the limit must fail");

    assert!(
        err.meta.logs.iter().any(|l| l.contains("RateLimitExceeded")),
        "expected RateLimitExceeded from the hook, got: {:?}",
        err.meta.logs
    );
}