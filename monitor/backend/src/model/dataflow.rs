use std::{
    hash::{Hash, Hasher}, 
};

use bytes::Bytes;
use base64::encode;
use serde::{Deserialize, Deserializer, Serialize, Serializer};



#[derive(Debug)]
pub struct Group {
    val: String,
}
impl Group {
    pub fn new(val: String) -> Self {
        Self { val }
    }
    pub fn into_string(self) -> String {
        self.val
    }
    pub fn to_str(&self) -> &str {
        &self.val
    }
}
impl ToString for Group {
    fn to_string(&self) -> String {
        self.val.clone()
    }
}
impl Clone for Group {
    fn clone(&self) -> Self {
        Self { val: self.val.clone() }
    }
}
impl Hash for Group {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.val.hash(state);
    }
}
impl PartialEq for Group {
    fn eq(&self, other: &Self) -> bool {
        self.val == other.val
    }
}
impl Eq for Group {}

impl<'de> Deserialize<'de> for Group {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
        Ok(Group::new(String::deserialize(deserializer)?))
    }
}
impl Serialize for Group {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer, {
        self.val.serialize(serializer)
    }
}



#[derive(Debug)]
pub struct Unit {
    val: String,
}
impl Unit {
    pub fn new(val: String) -> Self {
        Self { val }
    }
    pub fn into_string(self) -> String {
        self.val
    }
    pub fn to_str(&self) -> &str {
        &self.val
    }
}
impl ToString for Unit {
    fn to_string(&self) -> String {
        self.val.clone()
    }
}
impl Clone for Unit {
    fn clone(&self) -> Self {
        Self { val: self.val.clone() }
    }
}
impl Hash for Unit {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.val.hash(state);
    }
}
impl PartialEq for Unit {
    fn eq(&self, other: &Self) -> bool {
        self.val == other.val
    }
}
impl Eq for Unit {}
impl<'de> Deserialize<'de> for Unit {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
        Ok(Unit::new(String::deserialize(deserializer)?))
    }
}
impl Serialize for Unit {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer, {
        self.val.serialize(serializer)
    }
}



#[derive(Debug)]
pub struct Value {
    bytes: Bytes,
}
impl Value {
    pub fn new(bytes: Bytes) -> Self {
        Self { bytes }
    }
    pub fn into_base64(self) -> String {
        encode(self.bytes)
    }
}
impl Clone for Value {
    fn clone(&self) -> Self {
        Self { bytes: self.bytes.clone() }
    }
}
impl Serialize for Value {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer, {
        encode(&self.bytes).serialize(serializer)
    }
}



#[derive(Serialize, Debug)]
#[serde(tag = "y")]
pub enum Update {
    #[serde(rename = "n")]
    Online,
    #[serde(rename = "f")]
    Offline,
    #[serde(rename = "v")]
    Value{
        #[serde(rename = "v")]
        value: Value
    },
}
impl Update {
    pub fn to_ser(&self) -> (u8, Option<&[u8]>) {
        match self {
            Update::Offline => (0, None),
            Update::Online => (1, None),
            Update::Value{value} => (2, Some(value.bytes.as_ref())),
        }
    }

    pub fn from_ser(upd_type: u8, upd_bytes: Option<Vec<u8>>) -> Self {
        match upd_type {
            0 => Self::Offline,
            1 => Self::Online,
            _ => match upd_bytes {
                Some(b) => Self::Value{value: Value{bytes: b.into()}},
                None => Self::Value{value: Value{bytes: Bytes::new()}}, // TODO: check conversion
            }
        }
    }
}
impl Clone for Update {
    fn clone(&self) -> Self {
        match self {
            Self::Online => Self::Online,
            Self::Offline => Self::Offline,
            Self::Value{value} => Self::Value{value: value.clone()},
        }
    }
}



#[derive(Serialize, Debug)]
pub struct Record <T> where T: Clone + Send + Sync + Serialize {
    #[serde(rename = "s")]
    pub is_saved: bool,
    #[serde(rename = "i")]
    pub id: u64,
    #[serde(rename = "t")]
    pub time: i64,
    #[serde(flatten, rename = "z")] // TODO: remove 'rename = "z"'
    pub val: T
}
impl <T> Record <T> where T: Clone + Send + Sync + Serialize {
    pub fn establish_now(id: u64, val: T) -> Self {
        Self{
            id, val,
            is_saved: false,
            time: chrono::offset::Utc::now().timestamp_millis(),
        }
    } 
}
impl <T> Clone for Record <T> where T: Clone + Send + Sync + Serialize {
    fn clone(&self) -> Self {
        Self { 
            id: self.id.clone(),
            is_saved: self.is_saved, 
            time: self.time.clone(),
            val: self.val.clone(),
        }
    }
}

#[derive(Debug)]
pub enum Data <T> where T: Clone + Send + Sync {
    Single{group: Group, unit: Unit, update: T},
    Multi{vec: Vec<(Group, Vec<(Unit, T)>)>}
}
impl <T> Clone for Data<T> where T: Clone + Send + Sync {
    fn clone(&self) -> Self {
        match self {
            Self::Single { group, unit, update } => Self::Single { group: group.clone(), unit: unit.clone(), update: update.clone() },
            Self::Multi { vec } => Self::Multi {vec: vec.clone()},
        }
    }
}