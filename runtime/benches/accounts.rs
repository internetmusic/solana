#![feature(test)]

extern crate test;

use rand::Rng;
use solana_runtime::{
    accounts::{create_test_accounts, Accounts},
    bank::*,
};
use solana_sdk::{
    account::Account,
    genesis_config::{create_genesis_config, ClusterType},
    pubkey::Pubkey,
};
use std::{collections::HashMap, path::PathBuf, sync::Arc, thread::Builder};
use test::Bencher;

fn deposit_many(bank: &Bank, pubkeys: &mut Vec<Pubkey>, num: usize) {
    for t in 0..num {
        let pubkey = Pubkey::new_rand();
        let account = Account::new((t + 1) as u64, 0, &Account::default().owner);
        pubkeys.push(pubkey);
        assert!(bank.get_account(&pubkey).is_none());
        bank.deposit(&pubkey, (t + 1) as u64);
        assert_eq!(bank.get_account(&pubkey).unwrap(), account);
    }
}

#[bench]
fn bench_has_duplicates(bencher: &mut Bencher) {
    bencher.iter(|| {
        let data = test::black_box([1, 2, 3]);
        assert!(!Accounts::has_duplicates(&data));
    })
}

#[bench]
fn test_accounts_create(bencher: &mut Bencher) {
    let (genesis_config, _) = create_genesis_config(10_000);
    let bank0 = Bank::new_with_paths(
        &genesis_config,
        vec![PathBuf::from("bench_a0")],
        &[],
        None,
        None,
    );
    bencher.iter(|| {
        let mut pubkeys: Vec<Pubkey> = vec![];
        deposit_many(&bank0, &mut pubkeys, 1000);
    });
}

#[bench]
fn test_accounts_squash(bencher: &mut Bencher) {
    let (mut genesis_config, _) = create_genesis_config(100_000);
    genesis_config.rent.burn_percent = 100; // Avoid triggering an assert in Bank::distribute_rent_to_validators()
    let bank1 = Arc::new(Bank::new_with_paths(
        &genesis_config,
        vec![PathBuf::from("bench_a1")],
        &[],
        None,
        None,
    ));
    let mut pubkeys: Vec<Pubkey> = vec![];
    deposit_many(&bank1, &mut pubkeys, 250_000);
    bank1.freeze();

    // Measures the performance of the squash operation.
    // This mainly consists of the freeze operation which calculates the
    // merkle hash of the account state and distribution of fees and rent
    let mut slot = 1u64;
    bencher.iter(|| {
        let bank2 = Arc::new(Bank::new_from_parent(&bank1, &Pubkey::default(), slot));
        bank2.deposit(&pubkeys[0], 1);
        bank2.squash();
        slot += 1;
    });
}

#[bench]
fn test_accounts_hash_bank_hash(bencher: &mut Bencher) {
    let accounts = Accounts::new(
        vec![PathBuf::from("bench_accounts_hash_internal")],
        &ClusterType::Development,
    );
    let mut pubkeys: Vec<Pubkey> = vec![];
    let num_accounts = 60_000;
    let slot = 0;
    create_test_accounts(&accounts, &mut pubkeys, num_accounts, slot);
    let ancestors = vec![(0, 0)].into_iter().collect();
    let (_, total_lamports) = accounts.accounts_db.update_accounts_hash(0, &ancestors);
    bencher.iter(|| assert!(accounts.verify_bank_hash_and_lamports(0, &ancestors, total_lamports)));
}

#[bench]
fn test_update_accounts_hash(bencher: &mut Bencher) {
    solana_logger::setup();
    let accounts = Accounts::new(
        vec![PathBuf::from("update_accounts_hash")],
        &ClusterType::Development,
    );
    let mut pubkeys: Vec<Pubkey> = vec![];
    create_test_accounts(&accounts, &mut pubkeys, 50_000, 0);
    let ancestors = vec![(0, 0)].into_iter().collect();
    bencher.iter(|| {
        accounts.accounts_db.update_accounts_hash(0, &ancestors);
    });
}

#[bench]
fn test_accounts_delta_hash(bencher: &mut Bencher) {
    solana_logger::setup();
    let accounts = Accounts::new(
        vec![PathBuf::from("accounts_delta_hash")],
        &ClusterType::Development,
    );
    let mut pubkeys: Vec<Pubkey> = vec![];
    create_test_accounts(&accounts, &mut pubkeys, 100_000, 0);
    bencher.iter(|| {
        accounts.accounts_db.get_accounts_delta_hash(0);
    });
}

#[bench]
fn bench_delete_dependencies(bencher: &mut Bencher) {
    solana_logger::setup();
    let accounts = Accounts::new(
        vec![PathBuf::from("accounts_delete_deps")],
        &ClusterType::Development,
    );
    let mut old_pubkey = Pubkey::default();
    let zero_account = Account::new(0, 0, &Account::default().owner);
    for i in 0..1000 {
        let pubkey = Pubkey::new_rand();
        let account = Account::new((i + 1) as u64, 0, &Account::default().owner);
        accounts.store_slow(i, &pubkey, &account);
        accounts.store_slow(i, &old_pubkey, &zero_account);
        old_pubkey = pubkey;
        accounts.add_root(i);
    }
    bencher.iter(|| {
        accounts.accounts_db.clean_accounts(None);
    });
}

#[bench]
#[ignore]
fn bench_concurrent_read_write(bencher: &mut Bencher) {
    let num_readers = 5;
    let accounts = Arc::new(Accounts::new(
        vec![
            PathBuf::from(std::env::var("FARF_DIR").unwrap_or_else(|_| "farf".to_string()))
                .join("concurrent_read_write"),
        ],
        &ClusterType::Development,
    ));
    let num_keys = 1000;
    let slot = 0;
    accounts.add_root(slot);
    let pubkeys: Arc<Vec<_>> = Arc::new(
        (0..num_keys)
            .map(|_| {
                let pubkey = Pubkey::new_rand();
                let account = Account::new(1, 0, &Account::default().owner);
                accounts.store_slow(slot, &pubkey, &account);
                pubkey
            })
            .collect(),
    );

    for _ in 0..num_readers {
        let accounts = accounts.clone();
        let pubkeys = pubkeys.clone();
        Builder::new()
            .name("readers".to_string())
            .spawn(move || {
                let mut rng = rand::thread_rng();
                loop {
                    let i = rng.gen_range(0, num_keys);
                    test::black_box(accounts.load_slow(&HashMap::new(), &pubkeys[i]).unwrap());
                }
            })
            .unwrap();
    }

    let num_new_keys = 1000;
    let new_accounts: Vec<_> = (0..num_new_keys)
        .map(|_| Account::new(1, 0, &Account::default().owner))
        .collect();
    bencher.iter(|| {
        for account in &new_accounts {
            // Write to a different slot than the one being read from. Because
            // there's a new account pubkey being written to every time, will
            // compete for the accounts index lock on every store
            accounts.store_slow(slot + 1, &Pubkey::new_rand(), &account);
        }
    })
}
