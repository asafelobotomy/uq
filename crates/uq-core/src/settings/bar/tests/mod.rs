use super::super::{Islands, Settings};
use super::*;
use serde_json::json;

fn ids(z: &[BarItem]) -> Vec<&str> {
    z.iter().map(|i| i.id.as_str()).collect()
}

mod items;
mod load;
