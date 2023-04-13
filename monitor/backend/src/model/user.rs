use std::{
    hash::{Hash, Hasher}, 
    collections::{HashMap},
    str::FromStr,
};


use warp::filters::ws::WebSocket;
use regex::Regex;
use lazy_static::lazy_static;
use tokio::{
    time::{Duration},
    sync::mpsc::{Sender, Receiver, channel},
};


use crate::model::{
    session::{Session, Token},
    dataflow::{Group, Unit, Value, Update, Data, Record},
    wplace::{Name as NameWplace},
};
use crate::actor::{
    comm::{Signal as SignalComm},
    db::{Signal as SignalDb},
};

use super::wplace::Wplace;


// lazy_static!{
//     static ref REGEX: Regex = Regex::new(r"^[a-zA-Z0-9]+(?:[.\-_][a-zA-Z0-9]+)*$").unwrap();
// }


#[derive(Debug)]
pub struct Login {
    val: String,
}
impl Login {
    pub fn new(val: String) -> Self {
        Self { val }
    }

    pub fn as_str(&self) -> &str {
        &self.val
    }
}
impl FromStr for Login {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self{ val: s.to_string()})
    }
}
impl ToString for Login {
    fn to_string(&self) -> String {
        self.val.clone()
    }
}
impl Clone for Login {
    fn clone(&self) -> Self {
        Self { val: self.val.clone() }
    }
}
impl Hash for Login {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.val.hash(state);
    }
}
impl PartialEq for Login {
    fn eq(&self, other: &Self) -> bool {
        self.val == other.val
    }
}
impl Eq for Login {}




pub struct User {
    dur_sess: Duration,
    login: Login,
    name_wplace: NameWplace,
    tx_comm: Sender<SignalComm>,
    map: HashMap<Token, Session>,
}
impl User {
    pub fn new(login: Login, name_wplace: NameWplace, dur_sess: Duration, tx_comm: Sender<SignalComm>) -> Self {
        Self {
            login, tx_comm, dur_sess, name_wplace,
            map: HashMap::new(),
        }
    }

    pub fn sess_heartbeat(&mut self, token: &Token) -> Result<(), ()> {
        if let Some(session) = self.map.get_mut(token) {
            if let Err(_) = session.heartbeat() {
                self.map.remove(token);       
            }
        }
        if self.map.is_empty() {
            Err(())
        } else {
            Ok(())
        }
    }

    pub fn sess_make(&mut self) -> Token {
        let mut token = Token::new();
        while self.map.contains_key(&token) {
            token = Token::new();
        }
        let sesssion: Session = Session::new(self.login.clone(), token.clone(), self.dur_sess, self.tx_comm.clone());
        self.map.insert(token.clone(), sesssion);
        token
    }

    pub fn sess_check(&mut self, token: &Token) -> bool {
        if let Some(session) = self.map.get_mut(token) {
            session.refresh();
            true
        } else {
            false
        }
    }

    pub async fn send_data(&mut self, data: Data<Record<Update>>) {
        for (_, session) in self.map.iter_mut() {
            if session.is_online() {
                session.send_data(data.clone()).await;
            }
        }
    }

    pub async fn conn_close(&mut self, token: &Token, id: &u64) {
        if let Some(session) = self.map.get_mut(token) {
            session.conn_close(id).await;
        }
    }

    pub fn conn_add(&mut self, token: &Token, wplace: &Wplace, ws: WebSocket, tx_db: Sender<SignalDb> ) -> Result<(), WebSocket> {
        if let Some(session) = self.map.get_mut(token) {
            let mut map: HashMap<Group, Vec<Unit>> = HashMap::with_capacity(wplace.len_pubtop());
            for (group, vec_unit) in wplace.iter_pubtop() {
                let mut vec = Vec::with_capacity(vec_unit.len());
                for unit in vec_unit {
                    vec.push(unit.clone())
                }
                map.insert(group.clone(), vec);
            }
            session.conn_add(ws, map, tx_db);
            Ok(())
        } else {
            Err(ws)
        }
    }

    pub fn get_name_wplace(&self) -> &NameWplace {
        &self.name_wplace
    }
    
}
