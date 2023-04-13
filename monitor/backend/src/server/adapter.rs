use std::collections::HashMap;

use futures_util::SinkExt;
use tokio::time::{Duration, timeout};
use tokio::sync::{
    mpsc::{Sender, error::SendError},
    oneshot::channel as channel_one,
};
use warp::{
    reject::{custom as reject_custom, Rejection},
    ws::WebSocket,
};

use crate::actor::{
    comm::{Signal as SignalComm, FromServer as FromServerComm},
    db::{Signal as SignalDb, FromServer as FromServerDb},
};
use crate::model::{
    user::Login,
    session::Token,
    wplace::Wplace,
    dataflow::{Group, Unit, Record, Update},
};
use crate::server::reject::ErrorServer;


#[derive(Clone)]
pub struct Comm {
    tx_actor: Sender<SignalComm>,
}

impl Comm {
    pub fn new(tx_actor: Sender<SignalComm>) -> Self {
        Self{
            tx_actor
        }
    }

    pub async fn ws_add(&self, login: Login, token: Token, ws: WebSocket) -> Result<(), Option<(WebSocket, u16, String)>> {
        let (tx, rx) = channel_one::<Result<(), WebSocket>>();
        if let Err(err) = self.send_actor(FromServerComm::WsAdd { login, token, ws, tx }).await {
            if let Some(FromServerComm::WsAdd { login, token, ws, tx: _ }) = err {
                println!("[CommAdapter] Actor unreached: WsAdd: login={}, token={}", login.to_string(), token.to_string()); // TODO: log this
                Err(Some((ws, 1012, "Service Restart".to_string())))
            } else {
                println!("[CommAdapter] Actor unreached: WsAdd: wrong responce"); // TODO: log this
                Err(None)
            }
        } else {
            match rx.await {
                Ok(res) => match res {
                    Ok(_) => Ok(()),
                    Err(ws) => Err(Some((ws, 3000, "Unauthorized".to_string()))),
                },
                Err(_) => {
                    println!("[CommAdapter] Actor unresponded: WsAdd"); // TODO: log this
                    Err(None)
                },
            }
        }
    }

    pub async fn sess_make(&self, login: Login, wplace: Option<Wplace>) -> Result<(Login, Option<Token>), Rejection> {
        let (tx, rx) = channel_one::<Result<(Login, Token), Login>>();
        if let Err(err) = self.send_actor(FromServerComm::SessionMake { login, wplace, tx } ).await {
            if let Some(FromServerComm::SessionMake { login, wplace, tx: _ }) = err {
                println!("[CommAdapter] Actor unreached: SessionMake: login={}, wplace={:?}", login.to_string(), wplace); // TODO: log this
                Err(reject_custom(ErrorServer::InternalServerError))
            } else {
                println!("[CommAdapter] Actor unreached: SessionMake: wrong responce"); // TODO: log this
                Err(reject_custom(ErrorServer::InternalServerError))
            }
        } else {
            match rx.await {
                Ok(res) => match res {
                    Ok((login, token)) => Ok((login, Some(token))),
                    Err(login) => Ok((login, None)),
                },
                Err(_) => {
                    println!("[CommAdapter] Actor unresponded: SessionMake"); // TODO: log this
                    Err(reject_custom(ErrorServer::InternalServerError))
                },
            }
        }
    }

    pub async fn wplace_get(&self, login: Login, token: Token) -> Result<HashMap<Group, Vec<Unit>>, Rejection> {
        let (tx, rx) = channel_one::<Result<HashMap<Group, Vec<Unit>>, ()>>();
        if let Err(err) = self.send_actor(FromServerComm::WplaceGet { login, token, tx } ).await {
            if let Some(FromServerComm::WplaceGet { login, token, tx: _ }) = err {
                println!("[CommAdapter] Actor unreached: WplaceGet: login={}, token={}", login.to_string(), token.to_string()); // TODO: log this
                Err(reject_custom(ErrorServer::InternalServerError))
            } else {
                println!("[CommAdapter] Actor unreached: WplaceGet: wrong responce"); // TODO: log this
                Err(reject_custom(ErrorServer::InternalServerError))
            }
        } else {
            match rx.await {
                Ok(res) => match res {
                    Ok(map) => Ok(map),
                    Err(_) => Err(reject_custom(ErrorServer::Unauthorized)),
                },
                Err(_) => {
                    println!("[CommAdapter] Actor unresponded: WplaceGet"); // TODO: log this
                    Err(reject_custom(ErrorServer::InternalServerError))
                },
            }
        }
    }

    pub async fn unit_check(&self, login: Login, token: Token, group: Group, unit: Unit) -> Result<(Group, Unit), Rejection> {
        let (tx, rx) = channel_one::<Result<(Group, Unit), ()>>();
        if let Err(err) = self.send_actor(FromServerComm::UnitCheck { login, token, group, unit, tx } ).await {
            if let Some(FromServerComm::UnitCheck { login, token, group, unit, tx: _ }) = err {
                println!("[CommAdapter] Actor unreached: UnitCheck: login={}, token={}, group={}, unit={}", login.to_string(), token.to_string(), group.to_string(), unit.to_string()); // TODO: log this
                Err(reject_custom(ErrorServer::InternalServerError))
            } else {
                println!("[CommAdapter] Actor unreached: UnitCheck: wrong responce"); // TODO: log this
                Err(reject_custom(ErrorServer::InternalServerError))
            }
        } else {
            match rx.await {
                Ok(res) => match res {
                    Ok(group_unit) => Ok(group_unit),
                    Err(_) => Err(reject_custom(ErrorServer::Unauthorized)),
                },
                Err(_) => {
                    println!("[CommAdapter] Actor unresponded: UnitCheck"); // TODO: log this
                    Err(reject_custom(ErrorServer::InternalServerError))
                },
            }
        }
    }

    async fn send_actor(&self, cmd: FromServerComm) -> Result<(), Option<FromServerComm>> {
        if let Err(err) = self.tx_actor.send(SignalComm::FromServer(cmd)).await {
            if let SendError(SignalComm::FromServer(cmd)) = err {
                Err(Some(cmd))
            } else {
                Err(None)
            }
        } else {
            Ok(())
        }   
    }
}


#[derive(Clone)]
pub struct Db {
    tx_actor: Sender<SignalDb>,
}

impl Db {
    pub fn new(tx_actor: Sender<SignalDb>) -> Self {
        Self{
            tx_actor
        }
    }

    pub async fn get_data(&mut self, group: Group, unit: Unit, idx_min: u64, idx_max: u64) -> Result<Vec<Record<Update>>, Rejection> {
        let (tx, rx) = channel_one::<Result<Vec<Record<Update>>, ()>>();
        if let Err(err) = self.send_actor(FromServerDb::Get { group, unit, idx_min, idx_max, tx_resp: tx } ).await {
            if let Some(FromServerDb::Get { group, unit, idx_min, idx_max, tx_resp: _ }) = err {
                println!("[DBAdapter] Actor unreached: Get: group={}, unit={}, idx_min={}, idx_max={}", group.to_str(), unit.to_str(), idx_min, idx_max); // TODO: log this
                Err(reject_custom(ErrorServer::InternalServerError))
            } else {
                println!("[DBAdapter] Actor unreached: Get: wrong responce"); // TODO: log this
                Err(reject_custom(ErrorServer::InternalServerError))
            }
        } else {
            match rx.await {
                Ok(res) => match res {
                    Ok(vec_record) => Ok(vec_record),
                    Err(_) => Err(reject_custom(ErrorServer::BadRequest)),
                },
                Err(_) => {
                    println!("[DBAdapter] Actor unresponded: Get"); // TODO: log this
                    Err(reject_custom(ErrorServer::InternalServerError))
                },
            }
        }
    }

    // map: HashMap<Group, Vec<Unit>>, tx_resp: OneSender<HashMap<Group, Vec<(Unit, Option<Record<Update>>)>>>
    // HashMap<Group, Vec<(Unit, Option<Record<Update>>)>>
    pub async fn get_last(&self, map: HashMap<Group, Vec<Unit>>) -> Result<HashMap<Group, Vec<(Unit, Option<Record<Update>>)>>, Rejection> {
        let (tx, rx) = channel_one::<HashMap<Group, Vec<(Unit, Option<Record<Update>>)>>>();
        if let Err(err) = self.send_actor(FromServerDb::Last { map, tx_resp: tx }).await {
            if let Some(FromServerDb::Last{map, tx_resp: _}) = err {
                println!("[DBAdapter] Actor unreached: Last: map={:?}", map); // TODO: log this
                Err(reject_custom(ErrorServer::InternalServerError))
            } else {
                println!("[DBAdapter] Actor unreached: Last: wrong responce"); // TODO: log this
                Err(reject_custom(ErrorServer::InternalServerError))
            }
        } else {
            match rx.await {
                Ok(res) => Ok(res),
                Err(_) => {
                    println!("[DBAdapter] Actor unresponded: Last"); // TODO: log this
                    Err(reject_custom(ErrorServer::InternalServerError))
                },
            }
        }
    }

    async fn send_actor(&self, cmd: FromServerDb) -> Result<(), Option<FromServerDb>> {
        if let Err(err) = self.tx_actor.send(SignalDb::FromServer(cmd)).await {
            if let SendError(SignalDb::FromServer(cmd)) = err {
                Err(Some(cmd))
            } else {
                Err(None)
            }
        } else {
            Ok(())
        }   
    }
}
