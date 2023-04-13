use serde::{Deserialize, Serialize};

use crate::model::dataflow::{Group, Unit, Record, Update}; 




#[derive(Deserialize, Serialize)]
pub struct Sess {
    #[serde(rename = "l")]
    pub login: String,
    #[serde(rename = "t")]
    pub token: String,
} 

#[derive(Deserialize)]
pub struct Auth {
    #[serde(rename = "l")]
    pub login: String,
    #[serde(rename = "p")]
    pub password: String,
}

#[derive(Deserialize)]
pub struct QueryHist {
    #[serde(rename = "i")]
    pub min: u64,
    #[serde(rename = "a")]
    pub max: u64,
    #[serde(rename = "g")]
    pub group: Group,
    #[serde(rename = "u")]
    pub unit: Unit,
}

#[derive(Serialize, Debug)]
pub struct DtoRecord {
    #[serde(rename = "i")]
    id: u64, 
    #[serde(rename = "t")]
    time: i64, 
    #[serde(flatten, rename = "z")]
    update: DtoUpdate,
}
impl DtoRecord {
    pub fn new(record: Record<Update>) -> Self {
        Self{
            id: record.id,
            time: record.time,
            update: DtoUpdate::new(record.val),
        }
    }
}
#[derive(Serialize, Debug)]
#[serde(tag = "y")]
pub enum DtoUpdate {
    #[serde(rename = "f")]
    Offline,
    #[serde(rename = "n")]
    Online,
    #[serde(rename = "v")]
    Value{v: String},
}
impl DtoUpdate {
    pub fn new(update: Update) -> Self {
        match update {
            Update::Online => Self::Online,
            Update::Offline => Self::Offline,
            Update::Value{value} => Self::Value{v: value.into_base64()},
        }
    }
}
