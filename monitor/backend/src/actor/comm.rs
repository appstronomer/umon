use core::slice::{Iter as IterVec};
use std::{
    collections::{HashMap, HashSet, hash_map::Iter as IterMap},
};

use tokio::{
    sync::{
        mpsc::{Sender, Receiver}, 
        oneshot::{Sender as SenderOne},
    },
    time::Duration,
};
use warp::filters::ws::WebSocket;

use crate::actor::{
    db::{Signal as SignalDb},
};
use crate::model::{
    session::{Token},
    user::{Login, User},
    dataflow::{Group, Unit, Update, Data, Record},
    wplace::{Name as NameWplace, Wplace},
};


#[derive(Debug)]
pub enum Signal {
    FromServer(FromServer),
    FromDist(FromDist),
    FromDb(FromDb),
    FromSession(Login, Token, FromSession),
    FromConn(Login, Token, u64, FromConn),
}
#[derive(Debug)]
pub enum FromDb {
    Datapack(Vec<Data<Record<Update>>>),
    Data(Data<Record<Update>>),
    Closed,
}
#[derive(Debug)]
pub enum FromDist {
    Data(Data<Update>),
    Closed,
}
#[derive(Debug)]
pub enum FromConn {
    Closed,
}

#[derive(Debug)]
pub enum FromSession {
    SessionHeartbeat,
}
#[derive(Debug)]
pub enum FromServer {
    WplaceGet{login: Login, token: Token, tx: SenderOne<Result<HashMap<Group, Vec<Unit>>, ()>>},
    UnitCheck{login: Login, token: Token, group: Group, unit: Unit, tx: SenderOne<Result<(Group, Unit), ()>>},
    SessionCheck{login: Login, token: Token, tx: SenderOne<Result<(), ()>>},
    SessionMake{login: Login, wplace: Option<Wplace>, tx: SenderOne<Result<(Login, Token), Login>>},
    WsAdd{login: Login, token: Token, ws: WebSocket, tx: SenderOne<Result<(), WebSocket>>},
}
pub enum FromAuth {
    Success{token: String, pubs: Vec<(String, Vec<String>)>},
    Failure{token: String}
}



pub struct Comm {
    rx: Receiver<Signal>,
    tx: Sender<Signal>,
    tx_db: Sender<SignalDb>,
    map_group: HashMap<Group, HashMap<Unit, HashSet<NameWplace>>>,
    map_wplace: HashMap<NameWplace, Wplace>,
    map_user: HashMap<Login, User>,
    dur_sess: Duration,
}

impl Comm {
    
    pub fn new(rx: Receiver<Signal>, tx: Sender<Signal>, tx_db: Sender<SignalDb>,  dur_sess: Duration) -> Self {
        Self {
            rx, tx, tx_db,
            map_user: HashMap::new(),
            map_group: HashMap::new(),
            map_wplace: HashMap::new(),
            dur_sess,
        }
    }

    pub fn close(&mut self) {
        // println!("[COMM]: close");
        self.rx.close();
    }

    async fn destruct(&mut self) {
        // TODO: implement
        // println!("[COMM]: destruct");
        todo!()
    }

    pub async fn serve(&mut self) {
        // println!("[COMM]: serve started");
        while let Some(signal) = self.rx.recv().await {
            // println!("[COMM]: signal {:?}", signal);
            match signal {
                Signal::FromDb(cmd) => self.serve_db(cmd).await,
                Signal::FromDist(cmd) => todo!(), // TODO: remove this enum variant
                Signal::FromSession(login, token, cmd) => match cmd {
                    FromSession::SessionHeartbeat => self.serve_sess_heartbeat(login, token),
                },
                Signal::FromConn(login, token, id, cmd) => match cmd {
                    FromConn::Closed => self.serve_conn_closed(login, token, id).await,
                },
                Signal::FromServer(cmd) => match cmd {
                    FromServer::SessionCheck { login, token, tx } => { 
                        let _ = tx.send( self.serve_http_sess_check(login, token) ); 
                    },
                    FromServer::SessionMake { login, wplace, tx } => {
                        let _ = tx.send( self.serve_http_sess_make(login, wplace) );
                    },
                    FromServer::WsAdd { login, token, ws, tx } => {
                        let _ = tx.send( self.serve_http_ws_add(login, token, ws) );
                    },
                    FromServer::WplaceGet { login, token, tx } => {
                        let _ = tx.send( self.serve_http_wplace_get(login, token) );
                    },
                    FromServer::UnitCheck { login, token, group, unit, tx } => {
                        let _ = tx.send( self.serve_http_unit_check(login, token, group, unit) );
                    },
                },
            }
        }
        self.destruct().await;
    }


    async fn serve_db(&mut self, cmd: FromDb) {
        match cmd {
            FromDb::Data(data) => self.serve_db_data(data).await,
            FromDb::Datapack(vec_data) => {
                for data in vec_data {
                    self.serve_db_data(data).await;
                }
            },
            FromDb::Closed => self.close(),
        }
    }

    async fn serve_db_data(&mut self, data: Data<Record<Update>>) {
        match data {
            Data::Single { group, unit, update } => self.serve_data_single(group, unit, update).await,
            Data::Multi { vec } => {
                for (group, updates) in vec {
                    self.serve_data_multi(group, updates).await;
                }
            }
        }
    }

    async fn serve_conn_closed(&mut self, login: Login, token: Token, id: u64) {
        if let Some(user) = self.map_user.get_mut(&login) {
            user.conn_close(&token, &id).await;
        }
    }

    fn serve_http_wplace_get(&mut self, login: Login, token: Token) -> Result<HashMap<Group, Vec<Unit>>, ()> {
        if let Some(user) = self.map_user.get_mut(&login) {
            if user.sess_check(&token) {
                return if let Some(wplace) = self.map_wplace.get(user.get_name_wplace()) {
                    let mut map = HashMap::with_capacity(wplace.len_pubtop());
                    for (group, vec_unit) in wplace.iter_pubtop() {
                        let mut vec = Vec::with_capacity(vec_unit.len());
                        for unit in vec_unit {
                            vec.push(unit.clone())
                        }
                        map.insert(group.clone(), vec);
                    }
                    Ok(map)
                } else {
                    println!("[COMM]: serve_http_wplace_get: expected wplace not found"); // TODO: log this
                    Err(())
                };
            }
        }
        Err(())
    }

    fn serve_http_unit_check(&mut self, login: Login, token: Token, group: Group, unit: Unit,) -> Result<(Group, Unit), ()> {
        if let Some(user) = self.map_user.get_mut(&login) {
            if user.sess_check(&token) {
                return if let Some(wplace) = self.map_wplace.get(user.get_name_wplace()) {
                    if wplace.check_unit(&group, &unit) {
                        Ok((group, unit))
                    } else {
                        Err(())
                    }
                } else {
                    println!("[COMM]: serve_http_wplace_get: expected wplace not found"); // TODO: log this
                    Err(())
                };
            }
        }
        Err(())
    }

    fn serve_http_ws_add(&mut self, login: Login, token: Token, ws: WebSocket) -> Result<(), WebSocket> {
        if let Some(user) = self.map_user.get_mut(&login) {
            if let Some(wplace) = self.map_wplace.get(user.get_name_wplace()) {
                return if let Err(ws) = user.conn_add(&token, wplace, ws, self.tx_db.clone()) {
                    Err(ws)
                } else {
                    Ok(())
                };
            }
        } 
        return Err(ws)
    }


    fn serve_http_sess_make(&mut self, login: Login, wplace_opt: Option<Wplace>) -> Result<(Login, Token), Login> {
        if let Some(user) = self.map_user.get_mut(&login) {
            Ok((login, user.sess_make()))
        } else if let Some(mut wplace_new) = wplace_opt {
            let mut user: User = User::new(login.clone(), wplace_new.get_name().clone(), self.dur_sess, self.tx.clone());
            let token = user.sess_make();

            let wplace = if let Some(wplace) = self.map_wplace.get_mut(wplace_new.get_name()) {
                wplace
            } else {
                // TODO: refactor when this will be stable: https://doc.rust-lang.org/std/collections/struct.HashMap.html#method.raw_entry_mut
                let name = wplace_new.get_name().clone();
                self.map_wplace.insert(wplace_new.get_name().clone(), wplace_new);
                self.map_wplace.get_mut(&name).unwrap()
            };
            wplace.add_login(login.clone());
            for (group, vec_unit) in wplace.iter_pubtop() {
                // let group = self.map_group.entry(group.clone()).or_insert_with(|| HashMap::new());
                let group_map_unit = if let Some(group_map_unit) = self.map_group.get_mut(group) {
                    group_map_unit
                } else {
                    // TODO: refactor when this will be stable: https://doc.rust-lang.org/std/collections/struct.HashMap.html#method.raw_entry_mut
                    let group_map_unit = HashMap::new();
                    self.map_group.insert(group.clone(), group_map_unit);
                    self.map_group.get_mut(group).unwrap()
                };
                for unit in vec_unit {
                    if let Some(unit) = group_map_unit.get_mut(unit) {
                        unit.insert(wplace.get_name().clone());
                    } else {
                        let mut set = HashSet::new();
                        set.insert(wplace.get_name().clone());
                        group_map_unit.insert(unit.clone(), set);
                    }
                }
            }
            self.map_user.insert(login.clone(), user);
            Ok((login, token))
        } else {
            Err(login)
        }
    }


    fn serve_http_sess_check(&mut self, login: Login, token: Token) -> Result<(), ()> {
        if let Some(user) = self.map_user.get_mut(&login) {
            if user.sess_check(&token) {
                return Ok(());
            }
        }
        Err(())
    }


    fn serve_sess_heartbeat(&mut self, login: Login, token: Token) {
        if let Some(user) = self.map_user.get_mut(&login) {
            if let Err(_) = user.sess_heartbeat(&token) {
                self.remove_user(&login);
            }
        }
    }
    
    async fn serve_data_single(&mut self, group: Group, unit: Unit, update: Record<Update>) {
        // println!("[COMM]: data: {:?} >> {:?} :: {:?}", group, unit, update);
        let mut wplace_err_opt: Option<Vec<Wplace>> = None;
        if let Some(map_unit) = self.map_group.get_mut(&group) {
            if let Some(set_name) = map_unit.get_mut(&unit) {
                let mut name_err_opt: Option<Vec<NameWplace>> = None;
                for name in set_name.iter() {
                    if let Some(wplace) = self.map_wplace.get_mut(name) {
                        let mut login_err_opt: Option<Vec<Login>> = None;
                        for login in wplace.iter_login() {
                            if let Some(user) = self.map_user.get_mut(login) {
                                user.send_data(Data::Single { group: group.clone(), unit: unit.clone(), update: update.clone() }).await;
                            } else {
                                login_err_opt.get_or_insert_with(|| Vec::with_capacity(wplace.len_login())).push(login.clone());
                            }
                        }
                        if let Some(login_err_vec) = login_err_opt {
                            for login in login_err_vec {
                                wplace.remove_login(&login);
                            }
                        }
                        if wplace.is_empty() {
                            if let Some(wplace_deleted) = self.map_wplace.remove(name) {
                                wplace_err_opt.get_or_insert_with(|| Vec::with_capacity(set_name.len())).push(wplace_deleted);
                            }
                        }
                    } else {
                        name_err_opt.get_or_insert_with(|| Vec::with_capacity(set_name.len())).push(name.clone());
                    }
                }
                if let Some(name_err_vec) = name_err_opt {
                    for name in name_err_vec {
                        set_name.remove(&name);
                    }
                }
                if set_name.is_empty() {
                    map_unit.remove(&unit);
                }
            }
            if map_unit.is_empty() {
                self.map_group.remove(&group);
            }
        }
        if let Some(wplace_vec) = wplace_err_opt {
            for wplace in wplace_vec {
                self.clear_wplace_pubtop(wplace.get_name(), wplace.iter_pubtop());
            }
        }
    }

    async fn serve_data_multi(&mut self, group: Group, updates: Vec<(Unit, Record<Update>)>) {
        if let Some(map_unit) = self.map_group.get_mut(&group) {
            let mut map_wplace_ok: HashMap<NameWplace, Vec<(Unit, Record<Update>)>> = HashMap::new();
            for (unit, update) in updates.iter() {
                if let Some(set_unit_wplace) = map_unit.get_mut(unit) {
                    let mut vec_wplace_rm_opt: Option<Vec<NameWplace>> = None;
                    for name_wplace in set_unit_wplace.iter() {
                        if let Some(vec_update) = map_wplace_ok.get_mut(name_wplace) {
                            vec_update.push((unit.clone(), update.clone()));
                        } else if self.map_wplace.contains_key(name_wplace) {
                            let mut vec_update: Vec<(Unit, Record<Update>)> = Vec::with_capacity(updates.len());
                            vec_update.push((unit.clone(), update.clone()));
                            map_wplace_ok.insert(name_wplace.clone(), vec_update);
                        } else {
                            vec_wplace_rm_opt.get_or_insert_with(|| Vec::with_capacity(set_unit_wplace.len())).push(name_wplace.clone());
                        }
                    }
                    if let Some(vec_wplace_rm) = vec_wplace_rm_opt {
                        for name_wplace in vec_wplace_rm {
                            set_unit_wplace.remove(&name_wplace);
                        }
                        if set_unit_wplace.is_empty() {
                            map_unit.remove(unit);
                        }
                    }
                }
            }
            if map_unit.is_empty() {
                self.map_group.remove(&group);
            }
            let mut wplace_err_opt: Option<Vec<Wplace>> = None;
            for (name_wplace, vec_update) in map_wplace_ok.iter() {
                if let Some(wplace) = self.map_wplace.get_mut(&name_wplace) {
                    let mut login_err_opt: Option<Vec<Login>> = None;
                    for login in wplace.iter_login() {
                        if let Some(user) = self.map_user.get_mut(login) {
                            let mut vec_data = Vec::with_capacity(1);
                            vec_data.push((group.clone(), vec_update.clone()));
                            user.send_data(Data::Multi { vec: vec_data }).await;
                        } else {
                            login_err_opt.get_or_insert_with(|| Vec::with_capacity(wplace.len_login())).push(login.clone());
                        }
                    }
                    if let Some(login_err_vec) = login_err_opt {
                        for login in login_err_vec {
                            wplace.remove_login(&login);
                        }
                    }
                    if wplace.is_empty() {
                        if let Some(wplace_deleted) = self.map_wplace.remove(&name_wplace) {
                            wplace_err_opt.get_or_insert_with(|| Vec::with_capacity(map_wplace_ok.len())).push(wplace_deleted);
                        }
                    }
                }
                // else {} should not happen, because all missing wplaces where deleted from "Group>Unit>NameWplace" tree during map_wplace_ok collection
            }
            if let Some(wplace_vec) = wplace_err_opt {
                for wplace in wplace_vec {
                    self.clear_wplace_pubtop(wplace.get_name(), wplace.iter_pubtop());
                }
            }
        }
    }


    fn remove_user(&mut self, login: &Login) {
        if let Some(user) = self.map_user.remove(login) {
            if let Some(wplace) = self.map_wplace.get_mut(user.get_name_wplace()) {
                wplace.remove_login(&login);
                if wplace.is_empty() {
                    if let Some(wplace) = self.map_wplace.remove(user.get_name_wplace()){
                        self.clear_wplace_pubtop(wplace.get_name(), wplace.iter_pubtop());
                    }
                }
            }
        }
    }

    fn clear_wplace_pubtop(&mut self, name_wplace: &NameWplace, iter_pubtop: IterMap<Group, HashSet<Unit>>) {
        for (group, vec_unit) in iter_pubtop {
            let mut to_delete = false;
            if let Some(map_unit) = self.map_group.get_mut(group) {
                for top in vec_unit {
                    if let Some(set_name) = map_unit.get_mut(top) {
                        set_name.remove(name_wplace);
                        if set_name.is_empty() {
                            map_unit.remove(top);
                        }
                    }
                }
                to_delete = map_unit.is_empty();
            }
            if to_delete {
                self.map_group.remove(group);
            }
        }
    }

}