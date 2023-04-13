use std::net::SocketAddr;

use clap::Parser;
use tokio::sync::mpsc::{channel, Sender};
use tokio::time::Duration;
use rusqlite::Connection;

mod actor;
mod model;
mod args;
mod config;
mod fs;
mod server;

use args::Cli;
use config::*;
use actor::{
    comm::{Comm, Signal as SignalComm},
    db::{Db, Signal as SignalDb}, 
    dist::Dist,
};


fn main() {
    let cli = Cli::parse();
    match cli.command {
        args::Commands::Serve(serve) => cmd_serve(serve.address, serve.config),
    }
}

fn cmd_serve(addr: SocketAddr, cfg: ConfigServe) {
    let (tx_comm, rx_comm) = channel::<SignalComm>(cfg.db.tx_count_max);
    let (tx_db, rx_db) = channel::<SignalDb>(cfg.db.tx_count_max);
    let tx_comm_db = tx_comm.clone();
    let comm = Comm::new(rx_comm, tx_comm.clone(), tx_db.clone(), Duration::from_secs(60*30));
    let dist = Dist::new(tx_db.clone(), &cfg.groups);

    std::panic::set_hook(Box::new(|x| {
        println!("Thread paniced: {x}");
        std::process::exit(1);
    }));

    std::thread::spawn(move || { 
        // TODO: try to move connection creation inside Db::new() method
        let conn = Connection::open(&cfg.db.file).expect("unable to open or create a database file with provided filename");
        let mut db = Db::new(&conn, rx_db, tx_comm_db, cfg.db, &cfg.groups);
        db.serve(); 
    });
    std::thread::spawn(move || {
        cmd_serve_dist(dist);
    });
    cmd_serve_web(comm, addr, cfg.path, cfg.dir, tx_comm, tx_db);
}

#[tokio::main(flavor = "current_thread")]
async fn cmd_serve_web(mut comm: Comm, addr: SocketAddr, cfg_path: ConfigServePath, cfg_dir: ConfigServeDir, tx_comm: Sender<SignalComm>, tx_db: Sender<SignalDb>) {
    let handle_comm = tokio::spawn(async move { 
        comm.serve().await 
    });
    let handle_server = tokio::spawn(async move {
        server::serve(addr, cfg_path, cfg_dir, tx_comm, tx_db).await
    });
    if let Err(err) = tokio::try_join!(handle_comm, handle_server) {
        panic!("cmd_serve_web finished with error: {err}");
    }
}

#[tokio::main(flavor = "current_thread")]
async fn cmd_serve_dist(mut dist: Dist) {
    dist.serve().await
}
