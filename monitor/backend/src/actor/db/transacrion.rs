use rusqlite::{Connection, Statement, Error};



pub struct Transaction<'a> {
    tx_begin: Statement<'a>,
    tx_commit: Statement<'a>,
    tx_rollback: Statement<'a>,
}
impl <'a>Transaction<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { 
            tx_begin: prepare::stmt_tx_begin(conn),
            tx_commit: prepare::stmt_tx_commit(conn),
            tx_rollback: prepare::stmt_tx_rollback(conn),
        }
    }

    pub fn begin(&mut self) -> Result<(), Error> {
        self.tx_begin.execute([]).map(|_| ())
    }

    pub fn commit(&mut self) -> Result<(), Error> {
        self.tx_commit.execute([]).map(|_| ())
    }

    pub fn rollback(&mut self) -> Result<(), Error> {
        self.tx_rollback.execute([]).map(|_| ())
    }
}



mod prepare {
    use rusqlite::{Connection, Statement, Error};

    pub fn stmt_tx_begin<'a>(conn: &'a Connection) -> Statement<'a> {
        unwrap(conn.prepare("BEGIN TRANSACTION;"))
    }

    pub fn stmt_tx_commit<'a>(conn: &'a Connection) -> Statement<'a> {
        unwrap(conn.prepare("COMMIT;"))
    }

    pub fn stmt_tx_rollback<'a>(conn: &'a Connection) -> Statement<'a> {
        unwrap(conn.prepare("ROLLBACK;"))
    }

    fn unwrap(res: Result<Statement, Error>) -> Statement {
        match res {
            Ok(stmt) => stmt,
            Err(err) => panic!("Rusqlite: TransactionBuilder: prepare statement error: {}", err.to_string()),
        }
    }
}