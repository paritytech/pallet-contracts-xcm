// Copyright Parity Technologies (UK) Ltd.
// This file is part of Polkadot.

// Polkadot is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Polkadot is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Polkadot.  If not, see <http://www.gnu.org/licenses/>.

use impl_serde::serialize::from_hex;
use serde::Deserialize;
use sp_core::H256;
use std::{collections::HashMap, io::BufReader};

pub fn deserialize_from_hex<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let buf = String::deserialize(deserializer)?;
    from_hex(&buf).map_err(serde::de::Error::custom)
}

pub fn deserialize_selector_from_str_radix<'de, D>(deserializer: D) -> Result<[u8; 4], D::Error>
where
    D: serde::Deserializer<'de>,
{
    let buf = String::deserialize(deserializer)?;
    let selector = u32::from_str_radix(&buf[2..], 16).map_err(serde::de::Error::custom)?;
    Ok(selector.to_be_bytes())
}

pub fn deserialize_from_invokable_vec<'de, D>(
    deserializer: D,
) -> Result<HashMap<String, Invokable>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let mut map = HashMap::new();
    for item in Vec::<Invokable>::deserialize(deserializer)? {
        map.insert(item.label.clone(), item);
    }
    Ok(map)
}

#[derive(Deserialize)]
pub struct Contract {
    pub source: Source,
    pub spec: Spec,
}

#[derive(Deserialize)]
pub struct Source {
    #[serde(deserialize_with = "deserialize_from_hex")]
    pub wasm: Vec<u8>,
    pub hash: H256,
}

#[derive(Deserialize)]
pub struct Spec {
    #[serde(deserialize_with = "deserialize_from_invokable_vec")]
    pub constructors: HashMap<String, Invokable>,
    #[serde(deserialize_with = "deserialize_from_invokable_vec")]
    pub messages: HashMap<String, Invokable>,
}

#[derive(Deserialize)]
pub struct Invokable {
    #[serde(deserialize_with = "deserialize_selector_from_str_radix")]
    pub selector: [u8; 4],
    pub label: String,
}

pub fn read_contract() -> Contract {
    let file = std::fs::File::open(
        "/Users/pg/github/pallet-contracts-xcm/target/ink/xcm_contract/xcm_contract.contract",
    )
    .expect("xcm contract build not found");
    let contract = BufReader::new(file);

    serde_json::from_reader(contract).expect("Invalid json.")
}
