use std::{
    collections::HashMap,
    path::PathBuf,
    time::Duration, fmt::Debug,
};

use url::Host;
use warp::filters::BoxedFilter;
use serde::{de, Deserialize, Deserializer};

mod deser;

use crate::model::dataflow::{Group, Unit};
use deser::{
    deserialize_unit_map, 
    deserialize_dir, 
    deserialize_dir_opt, 
    deserialize_path, 
    deserialize_path_opt, 
    deserialize_qos,
    deserialize_duration_sec
};


#[derive(Debug)]
pub struct ConfigServe {
    pub path: ConfigServePath,
    pub dir: ConfigServeDir,
    pub groups: HashMap<Group, ConfigServeGroup>,
    pub db: ConfigServeDb,
}
impl<'de> Deserialize<'de> for ConfigServe {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
        let validator = ConfigServeValidator::deserialize(deserializer)?;
        if validator.path.public.is_some() && validator.dir.public.is_none() {
            Err(de::Error::custom(format!("if the 'path.public' config is given, then the 'dir.public' config must also be given")))
        } else if validator.dir.public.is_some() && validator.path.public.is_none() {
            Err(de::Error::custom(format!("if the 'dir.public' config is given, then the 'path.public' config must also be given")))
        } else {
            Ok(ConfigServe {
                path: validator.path,
                dir: validator.dir,
                groups: validator.groups,
                db: validator.db,
            })
        }
    }
}
#[derive(Deserialize)]
struct ConfigServeValidator {
    pub path: ConfigServePath,
    pub dir: ConfigServeDir,
    pub groups: HashMap<Group, ConfigServeGroup>,
    pub db: ConfigServeDb,
}

#[derive(Deserialize, Debug)]
pub struct ConfigServeDb {
    pub tx_count_max: usize,
    pub file: String,
}

#[derive(Deserialize, Debug)]
pub struct ConfigServeGroup {
    pub client: ConfigMqttClient,
    #[serde(deserialize_with = "deserialize_unit_map")]
    pub units: HashMap<Unit, ConfigMqttUnit> // <Topic, ConfigServeGroupUnit>
}

#[derive(Deserialize, Debug, Clone)]
pub struct ConfigMqttClient {
    pub id: String,
    #[serde(deserialize_with = "deserialize_duration_sec")]
    pub keep_alive: Duration, 
    pub clean_session: bool,
    pub capacity: usize,
    pub host: ConfigHost,
    pub port: u16,
}

#[derive(Deserialize, Debug)]
pub struct ConfigMqttUnit {
    pub topic: String,
    #[serde(deserialize_with = "deserialize_qos")]
    pub qos: u8,
    pub count_min: u64,
    pub count_max: u64,
}

#[derive(Deserialize, Debug)]
pub struct ConfigServeDir {
    #[serde(deserialize_with = "deserialize_dir")]
    pub users: PathBuf,
    #[serde(deserialize_with = "deserialize_dir")]
    pub wplaces: PathBuf, 
    #[serde(deserialize_with = "deserialize_dir_opt")]
    pub public: Option<PathBuf>,
}

#[derive(Deserialize, Debug)]
pub struct ConfigServePath {
    #[serde(deserialize_with = "deserialize_path")]
    pub app: BoxedFilter<()>,
    #[serde(deserialize_with = "deserialize_path_opt")]
    pub public: Option<BoxedFilter<()>>,
}

#[derive(Deserialize, Debug)]
pub struct ConfigWplace {
    pub groups: HashMap<String, Vec<String>>
}


#[derive(Deserialize, Debug)]
pub struct ConfigUser {
    pub password: String,
    pub wplace: String,
}


#[derive(Debug, Clone)]
pub struct ConfigHost {
    host: Host,
}
impl<'de> Deserialize<'de> for ConfigHost {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
        let text = String::deserialize(deserializer)?;
    if let Ok(host) = Host::parse(&text) {
        Ok(Self { host })
    } else {
        Err(de::Error::custom(format!("host should be IPv4, IPv6 or domain without protocol or port; given: {}", text)))
    }
    }
}
impl Into<String> for ConfigHost {
    fn into(self) -> String {
        self.host.to_string()
    }
}
