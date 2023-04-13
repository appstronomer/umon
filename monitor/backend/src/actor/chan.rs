use std::collections::HashMap;

use tokio::sync::mpsc::{channel as channel_tokio, Sender, Receiver, error::SendError};

use crate::model::dataflow::{Group, Unit, Update, Data, Record};


enum SignalConnIn {
    Req,
    Close,
    Tick,
    Pong(u64),
    Data(Data<Record<Update>>),
}

#[derive(Debug)]
pub enum SignalConnOut{
    Close,
    Tick,
    Pong(u64),
    Data(Group, Unit, Record<Update>),
    DataMap(HashMap<(Group, Unit), Record<Update>>),
}

pub struct RxConn {
    rx: Receiver<SignalConnOut>,
    tx: Sender<SignalConnIn>,
}
impl RxConn {
    pub async fn recv(&mut self) -> Option<SignalConnOut> {
        if let Err(_) = self.tx.send(SignalConnIn::Req).await {
            None
        } else {
            self.rx.recv().await
        }
    }

    pub fn close(&mut self) {
        self.rx.close(); 
    }
}



pub struct TxConn {
    tx: Sender<SignalConnIn>
}
impl Clone for TxConn {
    fn clone(&self) -> Self {
        Self { tx: self.tx.clone() }
    }
}
impl TxConn {
    pub async fn send_data(&self, data: Data<Record<Update>>) -> Result<(), Option<Data<Record<Update>>>> {
        if let Err(err) = self.tx.send(SignalConnIn::Data(data)).await {
            if let SendError(SignalConnIn::Data(data)) = err {
                Err(Some(data))
            } else {
                Err(None)
            }
        } else {
            Ok(())
        }
    }
    pub async fn send_tick(&self) -> Result<(), ()> {
        if let Err(_) = self.tx.send(SignalConnIn::Tick).await {
            Err(())
        } else {
            Ok(())
        }
    }
    pub async fn send_pong(&self, val: u64) -> Result<(), Option<u64>> {
        if let Err(err) = self.tx.send(SignalConnIn::Pong(val)).await {
            if let SendError(SignalConnIn::Pong(value)) = err {
                Err(Some(value))
            } else {
                Err(None)
            }
        } else {
            Ok(())
        }
    }
    pub async fn send_close(&self) -> Result<(), ()> {
        if let Err(err) = self.tx.send(SignalConnIn::Close).await {
            Err(())
        } else {
            Ok(())
        }
    }
}


struct ChannelConn {
    is_closed: bool,
    is_awaiting: bool,
    pong: Option<u64>,
    tick: Option<()>,
    map: Option<HashMap<(Group, Unit), Record<Update>>>, 
    rx: Receiver<SignalConnIn>,
    tx: Sender<SignalConnOut>,
}
impl ChannelConn {
    fn close(&mut self) {
        if !self.is_closed {
            self.rx.close();
            self.is_closed = true;
        }
        if self.is_awaiting {
            self.is_awaiting = false;
            self.tx.try_send(SignalConnOut::Close);
        }
    }

    async fn serve(&mut self) {
        while let Some(signal) = self.rx.recv().await {
            match signal {
                SignalConnIn::Req => self.serve_req(),
                SignalConnIn::Close => self.close(),
                SignalConnIn::Tick => self.serve_tick(),
                SignalConnIn::Pong(val) => self.serve_pong(val),
                SignalConnIn::Data(data) => self.serve_data(data),
            }
        }
    }

    fn serve_pong(&mut self, val: u64) {
        if self.is_awaiting {
            self.is_awaiting = false;
            if let Err(_) = self.tx.try_send(SignalConnOut::Pong(val)){
                self.close();
            }
        } else if self.pong.is_none() {
            self.pong = Some(val);
        } else {
            self.close();
        }
    }

    fn serve_tick(&mut self) {
        if self.is_awaiting {
            self.is_awaiting = false;
            if let Err(_) = self.tx.try_send(SignalConnOut::Tick){
                self.close();
            }
        } else if self.tick.is_none() {
            self.tick = Some(());
        }
    }

    fn serve_data(&mut self, data: Data<Record<Update>>) {
        if self.is_awaiting {
            self.is_awaiting = false;
            let message = match data {
                Data::Single { group, unit, update } => SignalConnOut::Data(group, unit, update),
                Data::Multi { vec } => {
                    let mut map: HashMap<(Group, Unit), Record<Update>> = HashMap::with_capacity(vec.len());
                    for (group, vec_update) in vec {
                        for (unit, update) in vec_update {
                            map.insert((group.clone(), unit), update);
                        }
                    }
                    SignalConnOut::DataMap(map)
                },
            };
            if let Err(_) = self.tx.try_send(message){
                self.close();
            }
        } else {
            let map = self.map.get_or_insert_with(|| HashMap::new());
            match data {
                Data::Single { group, unit, update } => { 
                    map.insert((group, unit), update);
                },
                Data::Multi { vec } => {
                    for (group, vec_update) in vec {
                        for (unit, update) in vec_update {
                            map.insert((group.clone(), unit), update);
                        }
                    }
                },
            }
        } 
    }

    fn serve_req(&mut self) {
        if self.is_closed {
            self.tx.try_send(SignalConnOut::Close);
        } if let Some(_) = self.tick.take() {
            if let Err(err) = self.tx.try_send(SignalConnOut::Tick) {
                self.close();
            }
        } else if let Some(val) = self.pong.take() {
            if let Err(err) = self.tx.try_send(SignalConnOut::Pong(val)) {
                self.close();
            }
        } else if let Some(map) = self.map.take() {
            if let Err(err) = self.tx.try_send(SignalConnOut::DataMap(map)) {
                self.close();
            }
        } else {
            self.is_awaiting = true;
        }
    }
    
}


pub fn channel_sec() -> (TxConn, RxConn) {
    let (tx_in, rx_in) = channel_tokio::<SignalConnIn>(1);
    let (tx_out, rx_out) = channel_tokio::<SignalConnOut>(1);

    let mut ch = ChannelConn{
        is_closed: false,
        is_awaiting: false,
        tick: None,
        pong: None,
        map: None,
        rx: rx_in,
        tx: tx_out,
    };

    tokio::spawn(async move{
        ch.serve().await
    });

    (TxConn{tx: tx_in.clone()} , RxConn{rx: rx_out, tx: tx_in})
}
