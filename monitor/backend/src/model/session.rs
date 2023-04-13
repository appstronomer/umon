use std::{
    hash::{Hash, Hasher}, 
    str::FromStr,
    collections::{HashMap},
};

use rusty_ulid::{Ulid, DecodingError};
use tokio::{
    sync::mpsc::{Sender, Receiver, channel},
    time::{timeout, Duration, Instant, sleep},
};
use warp::filters::ws::{WebSocket};
use futures_util::{
    future::join_all,
};

use crate::actor::{
    chan::TxConn,
    conn::{Conn},
    comm::{Signal as SignalComm, FromSession},
    db::{Signal as SignalDb},
};
use crate::model::{
    user::{Login},
    dataflow::{Group, Unit, Value, Update, Data, Record},
};


#[derive(Debug)]
pub struct Token {
    ulid: Ulid,
}
impl Token {
    pub fn new() -> Self {
        Self {
            ulid: Ulid::generate(),
        }
    }
}
impl FromStr for Token {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self{
            ulid:Ulid::from_str(s).map_err(|_| () )?
        })
    }
}
impl ToString for Token {
    fn to_string(&self) -> String {
        self.ulid.to_string()
    }
}
impl Clone for Token {
    fn clone(&self) -> Self {
        Self { ulid: self.ulid.clone() }
    }
}
impl Hash for Token {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.ulid.hash(state);
    }
}
impl PartialEq for Token {
    fn eq(&self, other: &Self) -> bool {
        self.ulid == other.ulid
    }
}
impl Eq for Token {}




enum State {
    Closed,
    Offline(Instant),
    Online(HashMap<u64, TxConn>),
}


pub struct Session {
    idx_conn: u64,
    token: Token,
    login: Login,
    tx_comm: Sender<SignalComm>,
    is_reminded: bool,
    dur: Duration,

    state: State,
}
impl Session {

    pub fn new(login: Login, token: Token, dur: Duration, tx_comm: Sender<SignalComm>) -> Self {
        let mut obj = Self {
            token, login, tx_comm, dur,
            idx_conn: 0,
            is_reminded: false,
            state: State::Offline(Instant::now()),
        };
        obj.remind(obj.dur.clone());
        obj
    }

    pub async fn send_data(&mut self, data: Data<Record<Update>>) {
        if let State::Online(map) = &mut self.state {
            let len = map.len();
            let mut vec_fut = Vec::with_capacity(len);
            let mut vec_id = Vec::with_capacity(len);
            for (id_conn, tx_conn) in map.iter(){
                vec_fut.push( tx_conn.send_data(data.clone()));
                vec_id.push(id_conn.to_owned());
            }
            let res_vec = join_all(vec_fut).await;
            for i in 0..len {
                if let Err(_) = res_vec[i] {
                    map.remove(&vec_id[i]);
                }
            }
            if map.is_empty() { 
                self.go_offline();
            } 
        }
    }

    pub async fn conn_close(&mut self, id: &u64) {
        if let State::Online(map) = &mut self.state {
            if let Some(tx) = map.remove(id) {
                tx.send_close().await;
            }
            if map.is_empty() {
                self.go_offline();
            }
        }
    }

    pub fn conn_add(&mut self, ws: WebSocket, map: HashMap<Group, Vec<Unit>>, tx_db: Sender<SignalDb>) {
        self.idx_conn += 1;
        let conn = Conn::new(self.login.clone(), self.token.clone(), self.idx_conn, ws, self.tx_comm.clone(), map, tx_db);
        if let State::Online(map) = &mut self.state {
            map.insert(self.idx_conn, conn.get_tx());
        } else {
            let mut map = HashMap::new();
            map.insert(self.idx_conn, conn.get_tx());
            self.state = State::Online(map);
        }
        tokio::spawn(async move { conn.serve().await });  
    }

    pub fn heartbeat(&mut self) -> Result<(), ()> {
        self.is_reminded = false;
        if let State::Offline(inst) = self.state {
            let elapsed = inst.elapsed();
            if elapsed >= self.dur {
                self.state = State::Closed;
                return Err(());
            } else {
                self.remind(self.dur - elapsed);
            }
        }
        Ok(())
    }

    pub fn refresh(&mut self) {
        if let State::Offline(_) = self.state {
            self.state = State::Offline(Instant::now());
        }
    }

    pub fn is_online(&self) -> bool {
        if let State::Online(_) = self.state {
            true
        } else {
            false
        }
    }


    fn remind(&mut self, dur: Duration) {
        self.is_reminded = true;
        let tx_comm = self.tx_comm.clone();
        let login = self.login.to_owned();
        let token = self.token.clone();
        tokio::spawn(async move {
            sleep(dur).await;
            tx_comm.send(SignalComm::FromSession(login.clone(), token.clone(), FromSession::SessionHeartbeat)).await;
        });
    }

    fn go_offline(&mut self) {
        self.state = State::Offline(Instant::now());
        if !self.is_reminded {
            self.remind(self.dur);
        }
    }
}



