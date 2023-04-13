use std::fmt;
use std::collections::HashMap;

use indexmap::IndexMap;
use tokio::sync::mpsc::{Sender, Receiver, channel};
use bytes::Bytes;

use crate::actor::{
    sub::Sub,
    db::{Signal as SignalDb, FromDist as FromDistDb}
};
use crate::model::dataflow::{Group, Unit, Value, Update, Data};
use crate::config::{ConfigServeGroup, ConfigMqttClient};


#[derive(Debug)]
pub struct Signal {
    pub id_broker: u32,
    pub payload: Payload,
}
#[derive(Debug)]
pub enum Payload {
    Data{topic: String, message: Bytes},
    Offline,
    Online,
    Closed,
}


struct StateGroup {
    group: Group,
    map_unit: IndexMap<String, StateUnit>,
    config_client: Option<ConfigMqttClient>,
}
struct StateUnit {
    unit: Unit,
    qos: u8,
}

pub struct Dist {
    map: HashMap<u32, StateGroup>,
    tx: Sender<Signal>,
    rx: Receiver<Signal>,
    tx_out: Sender<SignalDb>,
}
impl Dist {
    pub fn new(tx_out: Sender<SignalDb>, cfg_groups: &HashMap<Group, ConfigServeGroup>) -> Self {
        let mut map: HashMap<u32, StateGroup> = HashMap::with_capacity(cfg_groups.len());
        let mut idx = 0;
        for (group, cfg_serve) in cfg_groups {
            let mut map_unit: IndexMap<String, StateUnit> = IndexMap::with_capacity(cfg_serve.units.len());
            for (unit_name, unit_cfg) in cfg_serve.units.iter() {
                map_unit.insert(unit_cfg.topic.clone(), StateUnit{unit: unit_name.clone(), qos: unit_cfg.qos.clone()});
            }
            let cfg = StateGroup {
                group: group.clone(),
                map_unit,
                config_client: Some(cfg_serve.client.clone()),
            };
            map.insert(idx, cfg);
            idx += 1;
        }
        let (tx, rx) = channel(1);
        Self { 
            tx, rx, tx_out, map,
        }
    }

    fn close(&mut self) {
        self.rx.close();
    }

    async fn destruct(&mut self) {
        let _ = self.tx_out.send(SignalDb::FromDist(FromDistDb::Closed)).await;
    }

    async fn send_out(&mut self, cmd: FromDistDb) -> Result<(), ()> {
        if let Err(_) = self.tx_out.send(SignalDb::FromDist(cmd)).await {
            self.close();
            Err(())
        } else {
            Ok(())
        }
    }

    async fn init(&mut self) -> Result<(), DistError> {
        for (id_broker, state_group) in self.map.iter_mut() {
            let mut vec_topic: Vec<(String, u8)> = Vec::with_capacity(state_group.map_unit.len());
            for (topic, state_unit) in state_group.map_unit.iter() {
                vec_topic.push((topic.clone(), state_unit.qos.clone()))
            }
            let config_client = state_group.config_client.take().ok_or_else(|| DistError::ClientConfig)?;
            let mut sub = Sub::new(id_broker.clone(), config_client, vec_topic, self.tx.clone());
            tokio::spawn(async move { sub.serve().await; });
        }
        Ok(())
    }


    pub async fn serve(&mut self) {
        if let Err(err) = self.init().await {
            println!("dist init error {}", err); // TODO: LOG
        } else {
            while let Some(signal) = self.rx.recv().await {
                match signal.payload {
                    Payload::Data { topic, message } => self.serve_data(signal.id_broker, topic, message).await,
                    Payload::Offline => self.serve_broker_fill(&signal.id_broker, Update::Offline).await,
                    Payload::Online => self.serve_broker_fill(&signal.id_broker, Update::Online).await,
                    Payload::Closed => self.close(),
                }
            }
        }
        self.destruct().await;
    }

    async fn serve_data(&mut self, id_broker: u32, topic: String, message: Bytes) {
        if let Some(state_group) = self.map.get(&id_broker) {
            if let Some(state_unit) = state_group.map_unit.get(&topic) {
                let group = state_group.group.clone();
                let unit = state_unit.unit.clone();
                let _ = self.send_out(FromDistDb::Data(Data::Single {
                    group, 
                    unit, 
                    update: Update::Value{value: Value::new(message)},
                })).await;
            }
        }
    }

    async fn serve_broker_fill(&mut self, id_broker: &u32, update: Update) {
        if let Some(cfg) = self.map.get(id_broker) {
            let mut updates = Vec::with_capacity(cfg.map_unit.len());
            for (_, state_unit) in cfg.map_unit.iter() {
                updates.push((state_unit.unit.clone(), update.clone()));
            }
            let group = cfg.group.clone();
            let mut vec_data = Vec::with_capacity(1);
            vec_data.push((group, updates));
            if let Err(_) = self.send_out(FromDistDb::Data( Data::Multi { vec: vec_data } )).await {
                return;
            }
        }
    }
}


enum DistError {
    ClientConfig,
}
impl fmt::Display for DistError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DistError::ClientConfig => write!(f, "config not found for one of clients - probably Dist::serve().await called second time"),
        }
    }
}