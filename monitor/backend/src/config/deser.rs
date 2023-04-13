use std::{
    hash::Hash,
    collections::HashMap,
    path::{Path, PathBuf}, str::FromStr,
    time::Duration,
};

use url::Host;
use lazy_static::lazy_static;
use regex::Regex;
use warp::{filters::BoxedFilter, Filter};
use serde::{de, Deserialize, Deserializer};


use crate::model::dataflow::{Group, Unit};
use crate::config::{ConfigMqttUnit, ConfigServe, ConfigServePath, ConfigServeDir, ConfigServeDb, ConfigServeGroup};

lazy_static!{
    static ref REGEX: Regex = Regex::new(r"^(?P<host>(?:(?:(?:[0-9]|[1-9][0-9]|1[0-9]{2}|2[0-4][0-9]|25[0-5])\.){3}(?:[0-9]|[1-9][0-9]|1[0-9]{2}|2[0-4][0-9]|25[0-5]))|(?:(?:(?:[a-zA-Z0-9]|[a-zA-Z0-9][a-zA-Z0-9\-]*[a-zA-Z0-9])\.)*(?:[A-Za-z0-9]|[A-Za-z0-9][A-Za-z0-9\-]*[A-Za-z0-9]))):(?P<port>(?:6553[0-5])|(?:655[0-2][0-9])|(?:65[0-4][0-9]{2})|(?:6[0-4][0-9]{3})|(?:[1-5][0-9]{4})|(?:[0-5]{1,5})|(?:[0-9]{1,4}))$").unwrap();
}


pub fn deserialize_unit_map<'de, D>(deserializer: D) -> Result<HashMap<Unit, ConfigMqttUnit>, D::Error>
where D: de::Deserializer<'de>,
{
    let map: HashMap<Unit, ConfigMqttUnit> = HashMap::deserialize(deserializer)?;
    for (_, cfg_unit) in map.iter() {
        if cfg_unit.count_min >= cfg_unit.count_max {
            return Err(de::Error::custom(format!(
                "count_min should be less than count_max for each unit; given count_min={}, count_max={}", 
                &cfg_unit.count_min, 
                &cfg_unit.count_max
            )))
        }
    }
    Ok(map)
}

pub fn deserialize_qos<'de, D>(deserializer: D) -> Result<u8, D::Error>
where D: de::Deserializer<'de>,
{
    let qos = u8::deserialize(deserializer)?;
    if qos >= 0 && qos < 3 {
        Ok(qos)
    } else {
        Err(de::Error::custom(format!("qos should be between 0 and 2$ given: {}", qos)))
    }
}

pub fn deserialize_duration_sec<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where D: de::Deserializer<'de>,
{
    let secs = u64::deserialize(deserializer)?;
    Ok(Duration::from_secs(secs))
}

pub fn deserialize_dir<'de, D>(deserializer: D) -> Result<PathBuf, D::Error>
where D: de::Deserializer<'de>,
{
    let string = String::deserialize(deserializer)?;
    Ok(handle_dir::<D>(string)?)
}

pub fn deserialize_dir_opt<'de, D>(deserializer: D) -> Result<Option<PathBuf>, D::Error>
where D: de::Deserializer<'de>,
{
    let string_opt = Option::<String>::deserialize(deserializer)?;
    if let Some(string) = string_opt {
        Ok(Some(handle_dir::<D>(string)?))
    } else {
        Ok(None)
    }
}

pub fn deserialize_path<'de, D>(deserializer: D) -> Result<BoxedFilter<()>, D::Error>
where D: de::Deserializer<'de>,
{
    let string = String::deserialize(deserializer)?;
    Ok(handle_path::<D>(string)?)
}

pub fn deserialize_path_opt<'de, D>(deserializer: D) -> Result<Option<BoxedFilter<()>>, D::Error>
where D: de::Deserializer<'de>,
{
    let string_opt = Option::<String>::deserialize(deserializer)?;
    if let Some(string) = string_opt {
        Ok(Some(handle_path::<D>(string)?))
    } else {
        Ok(None)
    }   
}


fn handle_dir<'de, D>(string: String) -> Result<PathBuf, D::Error>
where D: de::Deserializer<'de>,
{
    let path = Path::new(&string).canonicalize().map_err(de::Error::custom)?;
    let meta = path.metadata().map_err(de::Error::custom)?;
    if !meta.is_dir() {
        Err(de::Error::custom(format!("path {} should be a directory", path.display())))
    } else {
        Ok(path)
    }
}

fn handle_path<'de, D>(string: String) -> Result<BoxedFilter<()>, D::Error>
where D: de::Deserializer<'de>,
{
    if string.eq("/") {
        return Ok(warp::any().boxed());
    }
    let mut split = string.split('/');
    let mut path = warp::any().boxed();
    if let Some(part) = split.next() {
        if !part.is_empty() {
            path = path.and(warp::path(part.to_string())).boxed();
        }
    }
    while let Some(part) = split.next() {
        if part.is_empty() {
            return Err(de::Error::custom(format!("path should not end with '/' (slash) and should not have empty parts between '/' (slashes)")));
        } else {
            path = path.and(warp::path(part.to_string())).boxed();
        }
    }
    Ok(path)
}
