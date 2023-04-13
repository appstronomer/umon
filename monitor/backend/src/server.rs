use std::{
    collections::{HashSet, HashMap},
    path::{Path as FsPath, PathBuf as FsPathBuf},
    net::SocketAddr, 
    str::FromStr,
    sync::Arc,
};

use base64;
use serde_json::{from_str as deser, to_string as ser};
use http::StatusCode;
use futures_util::{FutureExt, stream::StreamExt, SinkExt};
use serde::{Deserialize, Serialize};
use tokio::time::{timeout, Duration};
use tokio::sync::{
    Mutex,
    mpsc::{Sender, error::SendError},
    oneshot::{channel as channel_one},
};
use warp::{
    ws::Message as MessageWs,
    reject::{custom as reject_custom, Reject, Rejection},
    Filter,
    reply,
    Reply, ws::WebSocket,
};

mod adapter;
mod reject;
mod model;

use reject::{handle as reject_handle, ErrorServer};
use adapter::{Comm as AdapterComm, Db as AdapterDb};
use model::{Sess, Auth, QueryHist, DtoRecord, DtoUpdate};
use crate::fs;
use crate::config::{ConfigServePath, ConfigServeDir, ConfigUser, ConfigWplace};
use crate::model::{
    session::Token, 
    user::Login,
    dataflow::{Group, Unit},
    wplace::{Wplace, Name as NameWplace},
};
use crate::actor::{
    comm::{Signal as SignalComm, FromServer as FromServerComm},
    db::{Signal as SignalDb},
};


pub async fn serve(addr: SocketAddr, paths: ConfigServePath, dirs: ConfigServeDir, tx_comm: Sender<SignalComm>, tx_db: Sender<SignalDb>) {
    let adapter_comm = AdapterComm::new(tx_comm);
    let adapter_db = AdapterDb::new(tx_db);

    let dir_public_opt = dirs.public.clone();
    let path_public_opt = paths.public;
    let dirs_arc = Arc::new(Mutex::new(dirs));

    let path_app_login = warp::post()
        .and( warp::path("login") )
        .and( warp::body::content_length_limit(1024 * 2) )
        .and( warp::body::json() )
        .and( with(dirs_arc.clone()) )
        .and( with(adapter_comm.clone()) )
        .and_then( act_login );

    let path_app_ws = warp::path("ws")
        .and( warp::ws() )
        .and( with(adapter_comm.clone()) )
        .map( act_ws );

    let path_app_wplace = warp::path("wplace")
        .and( warp::header::<String>("sess").and_then(handle_sess_parse) )
        .and( with(adapter_comm.clone()) )
        .and_then(act_wplace);

    let path_app_wplace_last = warp::path("wplace-last")
        .and( warp::header::<String>("sess").and_then(handle_sess_parse) )
        .and( with(adapter_comm.clone()) )
        .and( with(adapter_db.clone()) )
        .and_then(act_wplace_last);

    let path_app_hist = warp::path("hist")
        .and( warp::header::<String>("sess").and_then(handle_sess_parse) )
        .and( with(adapter_comm) )
        .and( with(adapter_db) )
        .and( warp::query::<QueryHist>() )
        .and_then( act_hist );
    
    let path_app = paths.app.and(
            path_app_login
            .or(path_app_ws)
            .or(path_app_wplace)
            .or(path_app_hist)
            .or(path_app_wplace_last)
        );
    
    if let Some(path_public) = path_public_opt {
        if let Some(dir_public) = dir_public_opt {
            let path_router = path_app
                .or(path_public.and(warp::fs::dir(dir_public)))
                .recover(reject_handle);
            warp::serve(path_router).run(addr).await;
            return;
        }
    }

    let path_router = path_app.recover(reject_handle);
    warp::serve(path_router).run(addr).await;
}


// ACTIONS
fn act_ws(handle_ws: warp::ws::Ws, adapter_comm: AdapterComm) -> impl Reply {
    handle_ws.on_upgrade( |mut ws| async move {
        if let Ok((login, token)) = help_sess_recv(&mut ws).await {
            if let Err(Some((ws, code, reason))) = adapter_comm.ws_add(login, token, ws).await {
                help_ws_close(ws, code, reason).await;
            }
        } else {
            help_ws_close(ws, 3000, "Unauthorized".to_string()).await
        }
    })
}

async fn act_hist((login, token): (Login, Token), adapter_comm: AdapterComm, mut adapter_db: AdapterDb, query: QueryHist) -> Result<impl Reply, Rejection> {
    let (idx_min, idx_max) = handle_min_max(query.min, query.max)?;
    let (group, unit) = adapter_comm.unit_check(login, token, query.group, query.unit).await?;
    let records = adapter_db.get_data(group, unit, idx_min, idx_max).await?;
    // let mut vec_res = Vec::with_capacity(records.len());
    // for record in records {
    //     vec_res.push(DtoRecord::new(record));
    // }
    Ok( warp::reply::json(&records) )
}

async fn act_login(auth: Auth, dirs: Arc<Mutex<ConfigServeDir>>, adapter_comm: AdapterComm) -> Result<impl Reply, Rejection> {
    let dirs_guard = dirs.lock().await;
    let dir_users = dirs_guard.users.clone();
    drop(dirs_guard);
    let (login, cfg_user) = handle_user_auth(auth, &dir_users).await?;
    let (login, token_opt) = adapter_comm.sess_make(login, None).await?;
    if let Some(token) = token_opt {
        handle_sess_set(login, token)
    } else {
        let dirs_guard = dirs.lock().await;
        let mut path_res = dirs_guard.wplaces.clone();
        drop(dirs_guard);
        let file_name = format!("{}.json", cfg_user.wplace);
        let path_file = fs::path_extend(&mut path_res, file_name).map_err(|_| reject_custom(ErrorServer::NotFound))?;
        if let Some(Ok(cfg_wplace)) = fs::file_deser::<ConfigWplace>(path_file).await {
            let name_wplace = NameWplace::new(cfg_user.wplace);
            let wplace = help_wplace_make(name_wplace, cfg_wplace);
            if let (login, Some(token)) = adapter_comm.sess_make(login, Some(wplace)).await? {
                handle_sess_set(login, token)
            } else {
                Err(reject_custom(ErrorServer::InternalServerError))
            }
        } else {
            Err(reject_custom(ErrorServer::NotFound))
        }
    }
}

async fn act_wplace((login, token): (Login, Token), adapter_comm: AdapterComm) -> Result<impl Reply, Rejection> {
    let wplace_cfg = adapter_comm.wplace_get(login, token).await?;
    Ok(warp::reply::json(&wplace_cfg))
}

async fn act_wplace_last((login, token): (Login, Token), adapter_comm: AdapterComm, adapter_db: AdapterDb) -> Result<impl Reply, Rejection> {
    let wplace_cfg = adapter_comm.wplace_get(login, token).await?;
    let wplace_last = adapter_db.get_last(wplace_cfg).await?;
    Ok(warp::reply::json(&wplace_last))
}


// HANDLERS
fn handle_min_max(min: u64, max: u64) -> Result<(u64, u64), Rejection> {
    if let Some(count) = max.checked_sub(min) {
        if count < 100 {
            return Ok((min, max))
        } 
    }
    Err(reject_custom(ErrorServer::BadRequest))
}

async fn handle_user_auth(auth: Auth, dir_users: &FsPathBuf) -> Result<(Login, ConfigUser), Rejection> {
    let login = Login::from_str(&auth.login).map_err(|_| reject_custom(ErrorServer::BadRequest))?;
    let file_name = format!("{}.json", login.as_str());
    if let Ok(path_file) = fs::path_extend(dir_users, file_name) {
        if let Some(Ok(cfg_user)) = fs::file_deser::<ConfigUser>(path_file).await {
            if cfg_user.password == auth.password {
                return Ok((login, cfg_user));
            }
        }
    }
    Err(reject_custom(ErrorServer::Unauthorized))
}


fn handle_sess_set(login: Login, token: Token) -> Result<reply::WithStatus<std::string::String>, Rejection> {
    let sess = Sess{login: login.to_string(), token: token.to_string()};
    let sess_string = ser(&sess).map_err(|_| reject_custom(ErrorServer::InternalServerError))?;
    let sess_base64 = base64::encode(sess_string);
    Ok(reply::with_status(sess_base64, StatusCode::OK))
}

async fn handle_sess_parse(sess: String) -> Result<(Login, Token), Rejection> {
    Ok(help_sess_parse(&sess).map_err(|_| reject_custom(ErrorServer::Unauthorized))?)
}


// HELPERS
fn help_sess_parse(sess: &str) -> Result<(Login, Token), ()> {
    let vec_u8 = base64::decode(sess).map_err(|_| ())?;
    let str = std::str::from_utf8(&vec_u8).map_err(|_| ())?;
    let auth = deser::<Sess>(str).map_err(|_| ())?;
    if let (Ok(login), Ok(token)) = ( Login::from_str(&auth.login), Token::from_str(&auth.token) ) {
        Ok((login, token))
    } else {
        Err(())
    }
}

fn help_wplace_make(name: NameWplace, cfg_wplace: ConfigWplace) -> Wplace {
    let mut map_res: HashMap<Group, HashSet<Unit>> = HashMap::with_capacity(cfg_wplace.groups.len());
    for (group_string, vec_unit_string) in cfg_wplace.groups {
        let mut set_unit: HashSet<Unit> = HashSet::new();
        for unit_string in vec_unit_string {
            set_unit.insert(Unit::new(unit_string));
        }
        map_res.insert(Group::new(group_string), set_unit);
    }
    Wplace::new(name, map_res)
}

async fn help_sess_recv(ws: &mut WebSocket) -> Result<(Login, Token), ()> {
    if let Ok(Some(Ok(msg))) = timeout(Duration::from_secs(5), ws.next()).await {
        let sess = msg.to_str().map_err(|_| ())?;
        help_sess_parse(sess)
    } else {
        Err(())
    }
}

async fn help_ws_close(mut ws: WebSocket, code: u16, reason: String) {
    let _ = timeout( Duration::from_secs(5), ws.send(MessageWs::close_with(code, reason)) ).await;
}



// CONTEXTS
fn with<T>(val: T) -> impl Filter<Extract = (T,), Error = std::convert::Infallible> + Clone
where T: Clone + Send + Sync 
{
    warp::any().map(move || val.clone())
}
