// SPDX-FileCopyrightText: The endjine authors
// SPDX-License-Identifier: MPL-2.0

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[expect(clippy::struct_field_names)]
pub struct Rules {
    #[serde(rename = "match")]
    r#match: RulesMatch,
    rules: Vec<RulesItem>,
    rv: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RulesItem {
    col: String,
    con: String,
    param: String,
    v: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RulesMatch {
    One,
    All,
}
