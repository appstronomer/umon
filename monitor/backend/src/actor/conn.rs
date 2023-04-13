use std::{
    collections::{HashMap},
    time::{Duration, Instant},
    clone::Clone,
    sync::Arc, os::linux::fs::MetadataExt, str::FromStr,
};

use futures_util::{
    SinkExt, 
    StreamExt,
    stream::{SplitStream, SplitSink}
};
use serde::{Serialize, Deserialize};
use serde_json::{Error as SerError, Value as SerValue};
use tokio::{
    sync::{
        oneshot::channel as channel_one,
        mpsc::{channel, Sender, Receiver, error::SendError,}, 
    },
    net::TcpStream,
    time::{timeout, sleep},
};
use warp::{
    filters::ws::{WebSocket, Message},
};

use crate::{
    actor::{
        chan::{channel_sec as channel_sec, RxConn, TxConn, SignalConnOut},
        comm::{Signal as SignalComm, FromConn as FromConnComm},
        db::{Signal as SignalDb, FromConn as FromConnDb},
    },
    model::{
        session::{Token},
        user::{Login}, 
        dataflow::{Group, Unit, Value, Update, Record},
    }
};


// INPUT PROTOCOL PART
#[derive(Deserialize)]
#[serde(tag = "t")]
pub enum Input{
    #[serde(rename = "p")]
    Pong{
        #[serde(rename = "v")]
        val: u64
    },
}

// OUTPUT PROTOCOL PART
#[derive(Serialize, Debug)]
#[serde(tag = "x")]
pub enum Output {
    #[serde(rename = "c")]
    CtrlConnected{
        #[serde(rename = "m")]
        map: HashMap<Group, Vec<(Unit, Option<Record<Update>>)>>,
    },
    #[serde(rename = "p")]
    CtrlPing{
        #[serde(rename = "v")]
        val: u64
    },
    #[serde(rename = "d")]
    Data{
        #[serde(rename = "d")]
        data: Vec<DtoRecord>
    },
}
#[derive(Serialize, Debug)]
pub struct DtoRecord {
    #[serde(rename = "i")]
    id: u64, 
    #[serde(rename = "t")]
    time: i64, 
    #[serde(rename = "g")]
    group: String, 
    #[serde(rename = "u")]
    unit: String,
    #[serde(flatten, rename = "z")]
    update: DtoUpdate,
}
impl DtoRecord {
    fn new(group: Group, unit: Unit, record: Record<Update>) -> Self {
        Self{
            id: record.id,
            time: record.time,
            group: group.into_string(), 
            unit: unit.into_string(),
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
    fn new(update: Update) -> Self {
        match update {
            Update::Online => Self::Online,
            Update::Offline => Self::Offline,
            Update::Value{value} => Self::Value{v: value.into_base64()},
        }
    }
}


pub struct Conn {
    login: Login,
    token: Token,
    id: u64,
    ping_duration: Duration,
    ping: u64,
    pong: Option<u64>,
    writer: SplitSink<WebSocket, Message>,
    rx: RxConn,
    tx: TxConn,
    tx_actor: Sender<SignalComm>,
    config: Option<(SplitStream<WebSocket>, HashMap<Group, Vec<Unit>>, Sender<SignalDb>)>
}

impl Conn {
    pub fn new(
        login: Login,
        token: Token,
        id: u64,
        ws: WebSocket,
        tx_actor: Sender<SignalComm>,
        map: HashMap<Group, Vec<Unit>>, 
        tx_db: Sender<SignalDb>,
    ) -> Self {
        let (tx, rx) = channel_sec();
        let (writer, reader) = ws.split();
        Self { 
            login, token, id, writer, rx, tx, tx_actor,
            config: Some((reader, map, tx_db)),
            ping: 0,
            pong: None,
            ping_duration: Duration::from_secs(5),
        }
    }

    pub fn close(&mut self) {
        println!("[CONN]: close");
        self.rx.close();
    }

    async fn destruct(mut self) {
        // println!("[CONN]: destruct");
        let _ = self.tx.send_close().await;
        let _ = self.writer.close().await;
        if !self.tx_actor.is_closed() {
            let _ = self.tx_actor.send( SignalComm::FromConn(self.login, self.token, self.id, FromConnComm::Closed)).await;
        }
    }

    pub fn get_tx(& self) -> TxConn { self.tx.clone() }

    async fn send_ws(&mut self, out: Output) -> Result<(), ()> {
        // println!("[CONN]: send: {:?}", &out);
        match serde_json::to_string(&out) {
            Ok(text) => if let Ok(Ok(_)) = timeout(Duration::from_secs(5), self.writer.send(Message::text(text))).await {
                return Ok(());
            },
            Err(err) => println!("[CONN] send: serde_json.to_string err: {:#?}", err) // TODO: log this
        }
        self.close();
        Err(())
    }

    async fn init(&mut self) -> Result<(), ()> {
        if let Some((reader, map, tx_db)) = self.config.take() {
            let (tx, rx) = channel_one::<HashMap<Group, Vec<(Unit, Option<Record<Update>>)>>>();
            tx_db.send(SignalDb::FromConn(FromConnDb::Last { map, tx_resp: tx })).await.map_err(|_| ())?;
            let map = rx.await.map_err(|_| ())?;
            tokio::spawn(loop_reader(reader, self.tx.clone()));
            tokio::spawn(loop_heartbeat(self.tx.clone(), self.ping_duration));
            let _ = self.send_ws(Output::CtrlConnected{map}).await;
            let _ = self.send_ws(Output::CtrlPing{val:self.ping }).await;
            return Ok(());
        }
        Err(())
    }

    pub async fn serve(mut self) {
        if let Ok(_) = self.init().await {
            while let Some(signal) = self.rx.recv().await {
                // println!("[CONN]: {:?}", signal);
                match signal {
                    SignalConnOut::Tick => self.serve_tick().await,
                    SignalConnOut::Pong(val) => self.pong = Some(val),
                    SignalConnOut::Data(group, unit, record) => self.serve_data(group, unit, record).await,
                    SignalConnOut::DataMap(map) => self.serve_data_map(map).await,
                    SignalConnOut::Close => self.close(),
                }
            }
        }        
        self.destruct().await;
    }

    async fn serve_data_map(&mut self, map: HashMap<(Group, Unit), Record<Update>>) {
        let mut vec = Vec::with_capacity(map.len());
        for ((group, unit), update) in map {
            vec.push(DtoRecord::new(group, unit, update))
        }
        self.send_ws(Output::Data{data: vec}).await;
    }

    async fn serve_data(&mut self, group: Group, unit: Unit, record: Record<Update>) {
        self.send_ws(Output::Data{data: vec![DtoRecord::new(group, unit, record)]}).await;
    }

    async fn serve_tick(&mut self) {
        if let Some(pong) = self.pong {
            if self.ping == pong {
                self.pong = None;
                self.ping += 1;
                self.send_ws(Output::CtrlPing{val:self.ping }).await;
                return;
            }
        }
        self.close();
    }

}


async fn loop_reader(mut reader: SplitStream<WebSocket>, tx: TxConn) {
    while let Some(Ok(msg)) = reader.next().await {
        if let Ok(str) = msg.to_str() {
            if let Ok(input) = serde_json::from_str::<Input>(str) {
                match input {
                    Input::Pong { val } => if let Err(_) = tx.send_pong(val).await {
                        break; 
                    },
                }
            } else {
                break;
            } 
        } else {
            break;
        }
    }
    tx.send_close().await;
}

async fn loop_heartbeat(tx: TxConn, duration: Duration) {
    sleep(duration).await;
    while let Ok(_) = tx.send_tick().await {
        sleep(duration).await;
    }
}
