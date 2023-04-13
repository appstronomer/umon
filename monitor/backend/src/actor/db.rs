use std::collections::HashMap;

use rusqlite::Connection;
use tokio::sync::{
    mpsc::{channel, Sender, Receiver},
    oneshot::{Sender as OneSender},
};

mod repo_data;
mod transacrion;

use transacrion::Transaction;
use repo_data::RepoData;
use crate::config::{ ConfigServeGroup, ConfigServeDb};
use crate::model::{
    dataflow::{Group, Unit, Data, Update, Record}
};
use crate::actor::{
    comm::{Signal as SignalComm, FromDb as FromDbComm},
};


#[derive(Debug)]
pub enum Signal {
    FromConn(FromConn),
    FromServer(FromServer),
    FromDist(FromDist),
}
#[derive(Debug)]
pub enum FromConn{
    Last{map: HashMap<Group, Vec<Unit>>, tx_resp: OneSender<HashMap<Group, Vec<(Unit, Option<Record<Update>>)>>>},
}
#[derive(Debug)]
pub enum FromDist{
    Closed,
    Data(Data<Update>),
}
#[derive(Debug)]
pub enum FromServer{
    Get{group: Group, unit: Unit, idx_min: u64, idx_max: u64, tx_resp: OneSender<Result<Vec<Record<Update>>, ()>>},
    Last{map: HashMap<Group, Vec<Unit>>, tx_resp: OneSender<HashMap<Group, Vec<(Unit, Option<Record<Update>>)>>>},
}

pub struct Db<'a> {
    transaction_count_max: usize,
    repo_data: RepoData<'a>,
    tx_comm: Sender<SignalComm>,
    rx: Receiver<Signal>,
    transacrion: Transaction<'a>,
}
impl <'a> Db<'a> {
    pub fn new(conn: &'a Connection, rx: Receiver<Signal>, tx_comm: Sender<SignalComm>, cfg: ConfigServeDb, cfg_groups: &HashMap<Group, ConfigServeGroup>) -> Self {
        Self {
            transaction_count_max: cfg.tx_count_max,
            transacrion: Transaction::new(conn),
            repo_data: RepoData::new(conn, cfg_groups),
            tx_comm,
            rx,
        }
    }

    fn close(&mut self) {
        self.rx.close()
    }

    fn destruct(&mut self) {
        self.send_comm(FromDbComm::Closed);
    }

    fn send_comm(&mut self, cmd: FromDbComm) {
        if let Err(_) = self.tx_comm.blocking_send(SignalComm::FromDb(cmd)) {
            self.close();
        }
    }

    pub fn serve(&mut self) {
        while let Some(signal) = self.rx.blocking_recv() {
            self.serve_match(signal);
        }
        self.destruct();
    }

    fn serve_match(&mut self, signal: Signal) {
        match signal {
            Signal::FromDist(cmd) => match cmd {
                FromDist::Data( data ) => self.serve_dist_data(data),
                FromDist::Closed => self.close(),
            },
            Signal::FromServer(cmd) => match cmd {
                FromServer::Get { group, unit, idx_min, idx_max, tx_resp } => self.serve_server_get(tx_resp, group, unit, idx_min, idx_max),
                FromServer::Last { map, tx_resp } => self.serve_last(map, tx_resp),
            },
            Signal::FromConn(cmd) => match cmd {
                FromConn::Last { map, tx_resp } => self.serve_last(map, tx_resp),
            }
        }
    }

    fn serve_last(&mut self, map: HashMap<Group, Vec<Unit>>, tx_resp: OneSender<HashMap<Group, Vec<(Unit, Option<Record<Update>>)>>>) {
        let res = self.repo_data.data_last(map);
        let _ = tx_resp.send(res);
    }

    fn serve_dist_data(&mut self, data: Data<Update>) {
        let mut done_left = self.transaction_count_max;
        let mut signal_next: Option<Signal> = None;
        let mut datapack = Datapack::new();
        self.transacrion.begin(); // TODO: LOG
        if let Some((count, data_record)) = self.repo_data.data_push(data){
            done_left = done_left.checked_sub(count).unwrap_or(0);
            datapack.push(data_record);
        }
        loop {
            if done_left == 0 {
                break;
            }
            if let Ok(signal) = self.rx.try_recv() {
                if let Signal::FromDist(FromDist::Data(data)) = signal {
                    if let Some((count, data_record)) = self.repo_data.data_push(data) {
                        done_left = done_left.checked_sub(count).unwrap_or(0);
                        datapack.push(data_record);
                    }
                    done_left -= 1;
                } else {
                    signal_next = Some(signal);
                    break;
                }
            } else {
                break;
            }
        }
        if let Err(_) = self.transacrion.commit() {
            datapack.mark_unsaved();
        }
        self.send_datapack(datapack);
        self.repo_data.overflow_resolve();
        if let Some(signal) = signal_next {
            self.serve_match(signal);
        }
    }

    fn serve_server_get(&mut self, tx_resp: OneSender<Result<Vec<Record<Update>>, ()>>, group: Group, unit: Unit, idx_min: u64, idx_max: u64) {
        let res = self.repo_data.data_get(&group, &unit, idx_min, idx_max);
        let _ = tx_resp.send(res);
    }

    fn send_datapack(&mut self, datapack: Datapack) {
        match datapack.records {
            Records::Single(single_data) => self.send_comm(FromDbComm::Data(single_data)),
            Records::Multi(vec_data) => self.send_comm(FromDbComm::Datapack(vec_data)),
            Records::None => {},
        }
    }

}


enum Records {
    None,
    Single(Data<Record<Update>>),
    Multi(Vec<Data<Record<Update>>>),
}
struct Datapack {
    records: Records,
}
impl Datapack {
    fn new() -> Self {
        Self{records: Records::None}
    }

    fn push(&mut self, data: Data<Record<Update>>) {
        self.records = match std::mem::replace(&mut self.records, Records::None) {
            Records::None => {
                Records::Single(data)
            },
            Records::Single(single_data) => {
                let mut vec_data = Vec::new();
                vec_data.push(single_data);
                vec_data.push(data);
                Records::Multi(vec_data)
            },
            Records::Multi(mut vec_data) => {
                vec_data.push(data);
                Records::Multi(vec_data)
            },
        };
    }

    fn mark_unsaved(&mut self) {
        match &mut self.records {
            Records::Single(single_data) => Self::mark_unsaved_data(single_data),
            Records::Multi(vec_data) => {
                for single_data in vec_data {
                    Self::mark_unsaved_data(single_data)
                }
            },
            Records::None => {},
        }
    }

    fn mark_unsaved_data(data: &mut Data<Record<Update>>) {
        match data {
            Data::Single { group: _, unit: _, update } => update.is_saved = false,
            Data::Multi { vec } => {
                for (_, vec_unit) in vec {
                    for (_, record) in vec_unit {
                        record.is_saved = false;
                    }
                }
            },
        }
    }
}
