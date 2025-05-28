// SPDX-FileCopyrightText: The endjine authors
// SPDX-License-Identifier: MPL-2.0

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rules {
    #[serde(rename = "match")]
    pub r#match: RulesMatch,
    pub rules: Vec<RulesItem>,
    pub rv: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RulesItem {
    pub col: String,
    pub con: String,
    pub param: String,
    pub v: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RulesMatch {
    One,
    All,
}
