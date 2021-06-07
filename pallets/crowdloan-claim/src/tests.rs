// Copyright 2021 Parity Technologies (UK) Ltd.
// This file is part of Centrifuge (centrifuge.io) parachain.

// Cumulus is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Cumulus is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Cumulus.  If not, see <http://www.gnu.org/licenses/>.

//! Crowdloan claim pallet's unit test cases

// ----------------------------------------------------------------------------
// Imports and dependencies
// ----------------------------------------------------------------------------

use crate::{mock::*, Error as CrowdloanClaimError, *};
use hex;

use frame_support::{assert_noop, assert_ok};
use sc_rpc_api::state::ReadProof;
use sp_core::H256;
use sp_runtime::generic::BlockId;
use sp_std::str::FromStr;

lazy_static! {
    static ref AT: BlockId<mock::Block> = BlockId::hash(H256([
        179, 248, 101, 170, 172, 118, 184, 49, 49, 203, 205, 254, 58, 65, 241, 174, 74, 194, 9, 84,
        71, 138, 206, 149, 253, 151, 220, 96, 20, 87, 241, 132
    ]));
    static ref ROOT: H256 = H256([
        119, 115, 207, 116, 164, 194, 108, 32, 227, 26, 51, 110, 7, 66, 13, 75, 136, 126, 168, 72,
        148, 56, 92, 157, 48, 33, 95, 89, 159, 0, 47, 29
    ]);
}

fn to_hash(block_id: BlockId<mock::Block>) -> H256 {
    match block_id {
        BlockId::Hash(hash) => hash,
        BlockId::Number(..) => unreachable!(),
    }
}

struct Contributor {
    at: BlockId<mock::Block>,
    proof: ReadProof<H256>,
    signature: MultiSignature,
    parachain_account: u64,
    relaychain_account: AccountId32,
    contribution: u64,
}

fn get_alice() -> Contributor {
    Contributor {
        at: *AT,
        proof: ReadProof {
            at: to_hash(*AT),
            proof: vec! [
                hex::decode("7f00043593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d4400001a93fa350e00000000000000000000").unwrap().into(),
                hex::decode("80fffe800cfa750d79fe9f8dbd8bfd8bdcd8106725721ef079a6ebeb7c70e9ac4087e5f0806e39bf4935b2c4ee469ebe0caba3218232720cbde463ed32e82abe2cd16b4369806d840375ac50023cba0bf62aae154dbbcf8f9f7581b5ca59023a9b239a588cd68013d33a36a2874ba872cd9aa3678f1b0f51fe7b6ac3406f49ba56d1f113592fa3808486e2070a1edd40fa2341ccc2a4c08044dfe1e749adc6a9751a6457996eb01e8073ed69ffc8439133a73ca21171d11e905cf8c4a513d81d083d7ead82a8780ad58005d3f6f393526e332f639d5ab614b733c8160b7640ce44f15dd32d0f7efd0ee280d744e4f6f39f678509e71e9351373bafd93625cd7577597c0834d2ff50d5a22b80f659ce83e60662db5e205203451ffdc1f948dfd7dce42b77b17600bfe2a407f48007467d9cb336f4483d4a87fe50caa6301f6f020f4fe7bd62bd3312a07155b15580333312bdae42046be3aa88365cc0fbfc742a148e170930606d5485e6acf97630804c2ae754c2fd9fa33f47eea2c27d07fb96452cd539397b848f122dcb8e3abfa580e5d50384b0a787eec53eb734a38ed29145f33aaebbf96ca98e14ac84e0d3d2da80abe0339eaa5b4e3529c30b48702220a22be8c03c72f74a895c3af27644a1e02a80631198ab85157bab54212b9a397e49e79ce0cb3e76b889b8a0b0db948e9a5963").unwrap().into(),
                hex::decode("81068800807351a4feda356884ed36d6565d5bc5c2d706ff2bde0f08a5818513f8680c2d9280734caf72c52261c99ae6448f64ac6b3316f20ee7e2b0e7cc988b17d5b0f17c77").unwrap().into(),
                hex::decode("8106008180c5af1cb4e59c3a7851cbca197844315cec141cc5733da5ed1228a828989947cf801366bacdd2b9f61a5c9792af7f8fcea45ce071ddde753bfdf6c79e102f1d1927").unwrap().into(),
                hex::decode("8002a6804ddeba5402e3a2b74dd4e895699325b48af772b6b8264bff376f6d9a3dec9133545ee295d143ed41353167609a3d816584100a000000809db3c1292cb68f3f718498a5156c017c9b7fb7eb13dfe18157710101ab5e262680ccfebb484f3ddae2b53c8b4d5042ca38b1f9a21076440b100ce0e67eeb73b8e380d1213771c9115b0db742b21215a2c8aff8cffadca07e5be4a475aab64e668a6a").unwrap().into(),
                hex::decode("80002380372157afbdcde71343b20de553f2cc86265bb047d3cf20e798577d31575db15f80ebbe7356b417304b01dba19d64360272ee97d068e833bdf7b1c4a4e0cf4be9c58023cec1746760eb9fd847267d65578016d6a334e7f68c353930f2bdf78630bdfd").unwrap().into(),
                hex::decode("7f29696c645f73746f726167653a64656661756c743ac40cac02c4ed0673d410e5a6fc91234cd1287902634e34ee2b379c4e8a7131ca80d53b077d616969f8f94bd3b63d78afd9a1eaf557f166bb7cd6838144e7a4f779").unwrap().into()
            ]
        },
        signature: MultiSignature::Sr25519(sp_core::sr25519::Signature(
            hex::decode("a0db0cf026ffe5f0bc859681c9a1816e8a15991947753d6d7ecd1ac69c6e204c4ecf8f534d52ec9e76505e770dfa2b5d9614eca5e4d1de556dfa0de40dc7328f").unwrap().try_into().unwrap()
        )),
        parachain_account: 1,
        relaychain_account: AccountId32::from_str("0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d").unwrap(),
        contribution: 4000,

    }
}

fn get_bob() -> Contributor {
    Contributor {
        at: *AT,
        proof: ReadProof {
            at: to_hash(*AT),
            proof: vec![
                hex::decode("7f29696c645f73746f726167653a64656661756c743ac40cac02c4ed0673d410e5a6fc91234cd1287902634e34ee2b379c4e8a7131ca80d53b077d616969f8f94bd3b63d78afd9a1eaf557f166bb7cd6838144e7a4f779").unwrap().into(),
                hex::decode("80fffe800cfa750d79fe9f8dbd8bfd8bdcd8106725721ef079a6ebeb7c70e9ac4087e5f0806e39bf4935b2c4ee469ebe0caba3218232720cbde463ed32e82abe2cd16b4369806d840375ac50023cba0bf62aae154dbbcf8f9f7581b5ca59023a9b239a588cd68013d33a36a2874ba872cd9aa3678f1b0f51fe7b6ac3406f49ba56d1f113592fa3808486e2070a1edd40fa2341ccc2a4c08044dfe1e749adc6a9751a6457996eb01e8073ed69ffc8439133a73ca21171d11e905cf8c4a513d81d083d7ead82a8780ad58005d3f6f393526e332f639d5ab614b733c8160b7640ce44f15dd32d0f7efd0ee280d744e4f6f39f678509e71e9351373bafd93625cd7577597c0834d2ff50d5a22b80f659ce83e60662db5e205203451ffdc1f948dfd7dce42b77b17600bfe2a407f48007467d9cb336f4483d4a87fe50caa6301f6f020f4fe7bd62bd3312a07155b15580333312bdae42046be3aa88365cc0fbfc742a148e170930606d5485e6acf97630804c2ae754c2fd9fa33f47eea2c27d07fb96452cd539397b848f122dcb8e3abfa580e5d50384b0a787eec53eb734a38ed29145f33aaebbf96ca98e14ac84e0d3d2da80abe0339eaa5b4e3529c30b48702220a22be8c03c72f74a895c3af27644a1e02a80631198ab85157bab54212b9a397e49e79ce0cb3e76b889b8a0b0db948e9a5963").unwrap().into(),
                hex::decode("7f000eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a484400001a93fa350e00000000000000000000").unwrap().into(),
                hex::decode("80002380372157afbdcde71343b20de553f2cc86265bb047d3cf20e798577d31575db15f80ebbe7356b417304b01dba19d64360272ee97d068e833bdf7b1c4a4e0cf4be9c58023cec1746760eb9fd847267d65578016d6a334e7f68c353930f2bdf78630bdfd").unwrap().into(),
                hex::decode("8106008180c5af1cb4e59c3a7851cbca197844315cec141cc5733da5ed1228a828989947cf801366bacdd2b9f61a5c9792af7f8fcea45ce071ddde753bfdf6c79e102f1d1927").unwrap().into(),
                hex::decode("8002a6804ddeba5402e3a2b74dd4e895699325b48af772b6b8264bff376f6d9a3dec9133545ee295d143ed41353167609a3d816584100a000000809db3c1292cb68f3f718498a5156c017c9b7fb7eb13dfe18157710101ab5e262680ccfebb484f3ddae2b53c8b4d5042ca38b1f9a21076440b100ce0e67eeb73b8e380d1213771c9115b0db742b21215a2c8aff8cffadca07e5be4a475aab64e668a6a").unwrap().into(),
                hex::decode("81068800807351a4feda356884ed36d6565d5bc5c2d706ff2bde0f08a5818513f8680c2d9280734caf72c52261c99ae6448f64ac6b3316f20ee7e2b0e7cc988b17d5b0f17c77").unwrap().into()
            ]
        },
        signature: MultiSignature::Sr25519(sp_core::sr25519::Signature(
            hex::decode("e209475ff72ba5cb4438eb8e978b32b2fa515d8802f6c17182eff9557177c805b91a229662414a71c8de6d640346ea797d97f40bfdba859bcd098cddb16ba081").unwrap().try_into().unwrap()
        )),
        parachain_account: 2,
        relaychain_account: AccountId32::from_str("0x8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48").unwrap(),
        contribution: 4000,

    }
}

fn get_charlie() -> Contributor {
    Contributor {
        at: *AT,
        proof: ReadProof {
            at: to_hash(*AT),
            proof: vec![
                hex::decode("8002a6804ddeba5402e3a2b74dd4e895699325b48af772b6b8264bff376f6d9a3dec9133545ee295d143ed41353167609a3d816584100a000000809db3c1292cb68f3f718498a5156c017c9b7fb7eb13dfe18157710101ab5e262680ccfebb484f3ddae2b53c8b4d5042ca38b1f9a21076440b100ce0e67eeb73b8e380d1213771c9115b0db742b21215a2c8aff8cffadca07e5be4a475aab64e668a6a").unwrap().into(),
                hex::decode("80fffe800cfa750d79fe9f8dbd8bfd8bdcd8106725721ef079a6ebeb7c70e9ac4087e5f0806e39bf4935b2c4ee469ebe0caba3218232720cbde463ed32e82abe2cd16b4369806d840375ac50023cba0bf62aae154dbbcf8f9f7581b5ca59023a9b239a588cd68013d33a36a2874ba872cd9aa3678f1b0f51fe7b6ac3406f49ba56d1f113592fa3808486e2070a1edd40fa2341ccc2a4c08044dfe1e749adc6a9751a6457996eb01e8073ed69ffc8439133a73ca21171d11e905cf8c4a513d81d083d7ead82a8780ad58005d3f6f393526e332f639d5ab614b733c8160b7640ce44f15dd32d0f7efd0ee280d744e4f6f39f678509e71e9351373bafd93625cd7577597c0834d2ff50d5a22b80f659ce83e60662db5e205203451ffdc1f948dfd7dce42b77b17600bfe2a407f48007467d9cb336f4483d4a87fe50caa6301f6f020f4fe7bd62bd3312a07155b15580333312bdae42046be3aa88365cc0fbfc742a148e170930606d5485e6acf97630804c2ae754c2fd9fa33f47eea2c27d07fb96452cd539397b848f122dcb8e3abfa580e5d50384b0a787eec53eb734a38ed29145f33aaebbf96ca98e14ac84e0d3d2da80abe0339eaa5b4e3529c30b48702220a22be8c03c72f74a895c3af27644a1e02a80631198ab85157bab54212b9a397e49e79ce0cb3e76b889b8a0b0db948e9a5963").unwrap().into(),
                hex::decode("81068800807351a4feda356884ed36d6565d5bc5c2d706ff2bde0f08a5818513f8680c2d9280734caf72c52261c99ae6448f64ac6b3316f20ee7e2b0e7cc988b17d5b0f17c77").unwrap().into(),
                hex::decode("8106008180c5af1cb4e59c3a7851cbca197844315cec141cc5733da5ed1228a828989947cf801366bacdd2b9f61a5c9792af7f8fcea45ce071ddde753bfdf6c79e102f1d1927").unwrap().into(),
                hex::decode("80002380372157afbdcde71343b20de553f2cc86265bb047d3cf20e798577d31575db15f80ebbe7356b417304b01dba19d64360272ee97d068e833bdf7b1c4a4e0cf4be9c58023cec1746760eb9fd847267d65578016d6a334e7f68c353930f2bdf78630bdfd").unwrap().into(),
                hex::decode("7f0000b5ab205c6974c9ea841be688864633dc9ca8a357843eeacf2314649965fe224400008d49fd1a0700000000000000000000").unwrap().into(),
                hex::decode("7f29696c645f73746f726167653a64656661756c743ac40cac02c4ed0673d410e5a6fc91234cd1287902634e34ee2b379c4e8a7131ca80d53b077d616969f8f94bd3b63d78afd9a1eaf557f166bb7cd6838144e7a4f779").unwrap().into()
            ]
        },
        signature: MultiSignature::Sr25519(sp_core::sr25519::Signature(
            hex::decode("b2f5e70f5a43ef8e1fa044f5dcb788201bec11ae12c977833834fb847963014f587ddc1f074a58e71c801147802b858af61ace8952fafed01174d175e8910687").unwrap().try_into().unwrap()
        )),
        parachain_account: 3,
        relaychain_account: AccountId32::from_str("0x90b5ab205c6974c9ea841be688864633dc9ca8a357843eeacf2314649965fe22").unwrap(),
        contribution: 2000,
    }
}

fn get_false_signature() -> MultiSignature {
    MultiSignature::Sr25519(sp_core::sr25519::Signature(
        hex::decode("111111111111111111111111111111111111111111111111111111111111111111112a8e699c7b6893f649edc630bfe11c7d278fb11b5f1084972669e250cc8c").unwrap().try_into().unwrap()
    ))
}

fn get_false_proof() -> ReadProof<H256> {
    ReadProof {
        at: to_hash(*AT),
        proof: vec![
            hex::decode("1111111111111111111111111111111111111111111111111111111f376f6d9a3dec9133545ee295d143ed41353167609a3d816584100a000000809db3c1292cb68f3f718498a5156c017c9b7fb7eb13dfe18157710101ab5e262680ccfebb484f3ddae2b53c8b4d5042ca38b1f9a21076440b100ce0e67eeb73b8e380d1213771c9115b0db742b21215a2c8aff8cffadca07e5be4a475aab64e668a6a").unwrap().into(),
            hex::decode("111111111111111111111111111111111111111111111111111117e5f0806e39bf4935b2c4ee469ebe0caba3218232720cbde463ed32e82abe2cd16b4369806d840375ac50023cba0bf62aae154dbbcf8f9f7581b5ca59023a9b239a588cd68013d33a36a2874ba872cd9aa3678f1b0f51fe7b6ac3406f49ba56d1f113592fa3808486e2070a1edd40fa2341ccc2a4c08044dfe1e749adc6a9751a6457996eb01e8073ed69ffc8439133a73ca21171d11e905cf8c4a513d81d083d7ead82a8780ad58005d3f6f393526e332f639d5ab614b733c8160b7640ce44f15dd32d0f7efd0ee280d744e4f6f39f678509e71e9351373bafd93625cd7577597c0834d2ff50d5a22b80f659ce83e60662db5e205203451ffdc1f948dfd7dce42b77b17600bfe2a407f48007467d9cb336f4483d4a87fe50caa6301f6f020f4fe7bd62bd3312a07155b15580333312bdae42046be3aa88365cc0fbfc742a148e170930606d5485e6acf97630804c2ae754c2fd9fa33f47eea2c27d07fb96452cd539397b848f122dcb8e3abfa580e5d50384b0a787eec53eb734a38ed29145f33aaebbf96ca98e14ac84e0d3d2da80abe0339eaa5b4e3529c30b48702220a22be8c03c72f74a895c3af27644a1e02a80631198ab85157bab54212b9a397e49e79ce0cb3e76b889b8a0b0db948e9a5963").unwrap().into(),
            hex::decode("81068800807351a4feda356884ed36d6565d5bc5c2d706ff2bde01111111111111111111111111111111111111c99ae6448f64ac6b3316f20ee7e2b0e7cc988b17d5b0f17c77").unwrap().into(),
            hex::decode("8106008180c5af1cb4e59c3a7851cbca197844315cec141cc5733da5ed1228a828989947cf801366bacdd2b9f611111111111111111111111111111111fdf6c79e102f1d1927").unwrap().into(),
            hex::decode("80002381111111111111111111111111111111111111b047d3cf20e798577d31575db15f80ebbe7356b417304b01dba19d64360272ee97d068e833bdf7b1c4a4e0cf4be9c58023cec1746760eb9fd847267d65578016d6a334e7f68c353930f2bdf78630bdfd").unwrap().into(),
            hex::decode("7f0000b5ab201111111111111111111111111111111111111111cf2314649965fe224400008d49fd1a0700000000000000000000").unwrap().into(),
            hex::decode("7f29696c645f73746f726167653a64656661756c11111111111111111111111111111c91234cd1287902634e34ee2b379c4e8a7131ca80d53b077d616969f8f94bd3b63d78afd9a1eaf557f166bb7cd6838144e7a4f779").unwrap().into()
        ]
    }
}

fn init_module() {
    let test = H256(
        hex::decode("ec5efffa54fe5bdcc6920dfddaa3810ed3bcf4af1b9ec7311ed1e1ab493ae926")
            .unwrap()
            .try_into()
            .unwrap(),
    );
    CrowdloanClaim::initialize(Origin::signed(1), test, 0u32).unwrap();
}

// ----------------------------------------------------------------------------
// Test cases
// ----------------------------------------------------------------------------
#[test]
fn test_valid_initialize_transaction() {
    TestExternalitiesBuilder::default()
        .build(Some(init_module))
        .execute_with(|| {
            assert!(CrowdloanClaim::contributions().is_some());
            assert!(CrowdloanClaim::crowdloan_trie_index().is_some());
        })
}

#[test]
fn test_init_double() {
    TestExternalitiesBuilder::default()
        .build(Some(init_module))
        .execute_with(|| {
            assert_noop!(
                CrowdloanClaim::initialize(Origin::signed(1), *ROOT, 0u32),
                CrowdloanClaimError::<MockRuntime>::PalletAlreadyInitialized
            );
        })
}

#[test]
fn test_init_non_admin() {
    TestExternalitiesBuilder::default()
        .build(Some(init_module))
        .execute_with(|| {
            assert_noop!(
                CrowdloanClaim::initialize(Origin::signed(2), *ROOT, 0u32),
                CrowdloanClaimError::<MockRuntime>::MustBeAdministrator
            );
        })
}

#[test]
fn test_invalid_signed_claim_transaction() {
    TestExternalitiesBuilder::default()
        .build(Some(init_module))
        .execute_with(|| {
            let alice = get_alice();

            assert_noop!(
                CrowdloanClaim::claim_reward(
                    Origin::none(),
                    alice.relaychain_account,
                    alice.parachain_account,
                    alice.contribution,
                    get_false_signature(),
                    alice.proof.proof.into_iter().map(|byte| byte.0).collect(),
                ),
                CrowdloanClaimError::<MockRuntime>::InvalidContributorSignature
            );
        })
}

#[test]
fn test_valid_claim() {
    TestExternalitiesBuilder::default()
        .build(Some(init_module))
        .execute_with(|| {
            let alice = get_alice();
            let at = alice.at;
            let bob = get_bob();
            let charlie = get_charlie();
            let temp = hex::decode("48616c6c6f206963682062696e20436861726c696521").unwrap();
            let bytes = std::str::from_utf8(temp.as_slice()).unwrap();
            println!("{}", bytes);
            let contribution_scale = hex::decode(
                "00008d49fd1a070000000000000000005848616c6c6f206963682062696e20436861726c696521",
            )
            .unwrap();
            //let contribution_scale = hex::decode("00001a93fa350e00000000000000000000").unwrap();
            let (value, _memo): (ContributionAmountOf<MockRuntime>, Vec<u8>) =
                Decode::decode::<&[u8]>(&mut contribution_scale.as_ref())
                    .unwrap_or((Zero::zero(), Vec::new()));

            // 0x3a6368696c645f73746f726167653a64656661756c743ac40cac02c4ed0673d410e5a6fc91234cd1287902634e34ee2b379c4e8a7131ca
            // 0xc40cac02c4ed0673d410e5a6fc91234cd1287902634e34ee2b379c4e8a7131ca
            let mut buf = Vec::new();
            buf.extend_from_slice(b"crowdloan");
            buf.extend_from_slice(&CrowdloanClaim::crowdloan_trie_index().unwrap().encode()[..]);
            let hex_buf = hex::encode(<MockRuntime as frame_system::Config>::Hashing::hash(
                buf.as_slice(),
            ));

            let original_proof = alice.proof.proof.clone();
            let proof: Vec<Vec<u8>> = alice.proof.proof.into_iter().map(|byte| byte.0).collect();

            for (count, node) in proof.iter().enumerate() {
                println!("{}", hex::encode(node.as_slice()));

                let inner_original = original_proof.get(count).unwrap();
                for (count_2, inner) in node.iter().enumerate() {
                    assert_eq!(&(inner_original.0)[count_2], inner)
                }
            }

            assert_ok!(CrowdloanClaim::claim_reward(
                Origin::none(),
                alice.relaychain_account,
                alice.parachain_account,
                alice.contribution,
                alice.signature,
                proof
            ));
        })
}

#[test]
fn test_invalid_claim_invalid_proof() {
    let alice = get_alice();
    TestExternalitiesBuilder::default()
        .build(Some(init_module))
        .execute_with(|| {
            let alice = get_alice();

            assert_noop!(
                CrowdloanClaim::claim_reward(
                    Origin::none(),
                    alice.relaychain_account,
                    alice.parachain_account,
                    alice.contribution,
                    alice.signature,
                    get_false_proof()
                        .proof
                        .into_iter()
                        .map(|byte| byte.0)
                        .collect(),
                ),
                CrowdloanClaimError::<MockRuntime>::InvalidProofOfContribution
            );
        })
}

#[test]
fn test_invalid_claim_mod_not_initalized() {
    TestExternalitiesBuilder::default()
        .build(Some(|| {}))
        .execute_with(|| {
            let alice = get_alice();

            assert_noop!(
                CrowdloanClaim::claim_reward(
                    Origin::none(),
                    alice.relaychain_account,
                    alice.parachain_account,
                    alice.contribution,
                    alice.signature,
                    alice.proof.proof.into_iter().map(|byte| byte.0).collect(),
                ),
                CrowdloanClaimError::<MockRuntime>::PalletNotInitialized
            );
        })
}
