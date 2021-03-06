use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    pub admin: Addr,
    pub pool_total_amount: Uint128,
}

pub const AMOUNT_BY_USER: Map<&Addr, Uint128> = Map::new("amount");
pub const GAIN_BY_USER: Map<&Addr, Uint128> = Map::new("gain");

pub const STATE: Item<State> = Item::new("state");
