#![cfg(test)]
//! Fractional resource ownership (Issue #89): split/merge round trips,
//! split validation, owner-only merges, and transfers between owners.

use soroban_sdk::testutils::Address as _;
use soroban_sdk::{symbol_short, Address, Env};
use stellar_nebula_nomad::{NebulaNomadContract, NebulaNomadContractClient};

fn setup() -> (Env, NebulaNomadContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract = env.register(NebulaNomadContract, ());
    let client = NebulaNomadContractClient::new(&env, &contract);
    let admin = Address::generate(&env);
    client.initialize_fractional(&admin);
    (env, client, admin)
}

#[test]
fn split_then_merge_round_trip_restores_whole_resource() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    let ore = symbol_short!("ore");

    let shares = client.fractionalize_resource(&owner, &ore, &100, &4);

    assert_eq!(shares.len(), 4);
    assert_eq!(client.get_total_shares(), 4);
    assert_eq!(client.get_owner_shares(&owner).len(), 4);
    for share_id in shares.iter() {
        let share = client.get_share(&share_id).unwrap();
        assert_eq!(share.amount, 25);
        assert_eq!(share.owner, owner);
    }

    let merged = client.merge_fractions(&owner, &shares.clone());

    assert_eq!(merged, 100);
    assert_eq!(client.get_owner_shares(&owner).len(), 0);
    assert!(client.get_share(&shares.get(0).unwrap()).is_none());
}

#[test]
fn split_with_remainder_is_rejected() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);

    // 100 does not divide evenly into 3 shares.
    assert!(client
        .try_fractionalize_resource(&owner, &symbol_short!("ore"), &100, &3)
        .is_err());
}

#[test]
fn split_beyond_per_tx_limit_is_rejected() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);

    // 51 shares exceeds MAX_FRACTIONS_PER_TX (50), even though 5100 splits evenly.
    assert!(client
        .try_fractionalize_resource(&owner, &symbol_short!("ore"), &5100, &51)
        .is_err());
}

#[test]
fn non_owner_cannot_merge_shares() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    let other = Address::generate(&env);

    let shares = client.fractionalize_resource(&owner, &symbol_short!("gas"), &40, &4);

    assert!(client.try_merge_fractions(&other, &shares).is_err());
    assert_eq!(client.get_owner_shares(&owner).len(), 4);
}

#[test]
fn transferred_share_moves_merge_rights_to_new_owner() {
    let (env, client, _admin) = setup();
    let owner = Address::generate(&env);
    let buyer = Address::generate(&env);

    let shares = client.fractionalize_resource(&owner, &symbol_short!("ore"), &100, &2);
    let moved = shares.get(0).unwrap();
    client.transfer_share(&owner, &buyer, &moved);

    assert_eq!(client.get_share(&moved).unwrap().owner, buyer);
    assert_eq!(client.get_owner_shares(&buyer).len(), 1);

    // The previous owner can no longer merge the transferred share.
    assert!(client.try_merge_fractions(&owner, &shares).is_err());
}
