// Smoldot
// Copyright (C) 2023  Pierre Krieger
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

use smoldot::json_rpc;
use std::sync::Arc;

#[test]
fn send_request_errs_if_malformed() {
    smol::block_on(async move {
        let client = smoldot_full_node::start(smoldot_full_node::Config {
            chain: smoldot_full_node::ChainConfig {
                chain_spec: (&include_bytes!("./substrate-node-template.json")[..]).into(),
                additional_bootnodes: Vec::new(),
                keystore_memory: vec![],
                sqlite_database_path: None,
                sqlite_cache_size: 256 * 1024 * 1024,
                keystore_path: None,
            },
            relay_chain: None,
            libp2p_key: Box::new([0; 32]),
            listen_addresses: Vec::new(),
            json_rpc_listen: None,
            tasks_executor: Arc::new(|task| smol::spawn(task).detach()),
            log_callback: Arc::new(move |_, _| {}),
            jaeger_agent: None,
        })
        .await
        .unwrap();

        client.send_json_rpc_request(r#"thisisnotproperjsonrpc"#.to_owned());
        let response_raw = client.next_json_rpc_response().await;
        match json_rpc::parse::parse_response(&response_raw).unwrap() {
            json_rpc::parse::Response::ParseError { .. } => {}
            _ => unreachable!(),
        }
    });
}

#[test]
fn send_request_works_if_unknown_request() {
    smol::block_on(async move {
        let client = smoldot_full_node::start(smoldot_full_node::Config {
            chain: smoldot_full_node::ChainConfig {
                chain_spec: (&include_bytes!("./substrate-node-template.json")[..]).into(),
                additional_bootnodes: Vec::new(),
                keystore_memory: vec![],
                sqlite_database_path: None,
                sqlite_cache_size: 256 * 1024 * 1024,
                keystore_path: None,
            },
            relay_chain: None,
            libp2p_key: Box::new([0; 32]),
            listen_addresses: Vec::new(),
            json_rpc_listen: None,
            tasks_executor: Arc::new(|task| smol::spawn(task).detach()),
            log_callback: Arc::new(move |_, _| {}),
            jaeger_agent: None,
        })
        .await
        .unwrap();

        client.send_json_rpc_request(
            r#"{"jsonrpc":"2.0","id":1,"method":"thisjsonrpcmethoddoesntexist","params":[]}"#
                .to_owned(),
        );
        let response_raw = client.next_json_rpc_response().await;
        match json_rpc::parse::parse_response(&response_raw).unwrap() {
            json_rpc::parse::Response::Error { id_json, .. } => {
                assert_eq!(id_json, "1");
            }
            _ => unreachable!(),
        }
    });
}
