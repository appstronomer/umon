use std::collections::{HashMap, VecDeque};

use rusqlite::{Connection, Statement, named_params, Error as SqlErr, ErrorCode as SqlErrorCode, ffi::Error as SqlErrInner, OptionalExtension};

use crate::config::ConfigServeGroup;
use crate::model::dataflow::{Group, Unit, Data, Update, Record};

struct StateUnit {
    count: u64,
    count_min: u64,
    count_max: u64,
    record_last: Option<Record<Update>>,
}

pub struct RepoData<'a> {
    stmt_data_get: Statement<'a>,
    stmt_data_rm_old_count: Statement<'a>,
    stmt_data_push: Statement<'a>,
    map_group: HashMap<Group, HashMap<Unit, u32>>,
    map_state: HashMap<u32, StateUnit>, // <id_unit, StateUnit>
    id_units_overflowed: VecDeque<u32>,
}
impl <'a>RepoData <'a> {
    pub fn new(conn: &'a Connection, cfg_groups: &HashMap<Group, ConfigServeGroup>) -> Self {
        let mut count_units: usize = 0;
        let (id_group_max, id_unit_max) = prepare::init(conn);
        let mut id_group_new = if let Some(id) = id_group_max { id + 1 } else { 0 };
        let mut id_unit_new = if let Some(id) = id_unit_max { id + 1 } else { 0 };

        let mut stmt_group_set = prepare::stmt_group_set(conn);
        let mut stmt_group_get = prepare::stmt_group_get(conn);
        let mut stmt_unit_set = prepare::stmt_unit_set(conn);
        let mut stmt_unit_get = prepare::stmt_unit_get(conn);
        let mut stmt_data_get_last = prepare::stmt_data_get_last(conn);
        let mut stmt_data_get_count = prepare::stmt_data_get_count(conn);

        let mut map_group = HashMap::new();
        let mut map_state = HashMap::new();
        
        for (group, cfg_group) in cfg_groups {
            match stmt_group_set.execute(named_params! {":id": &id_group_new, ":name": group.to_str()}) {
                Ok(_) => { id_group_new += 1 },
                Err(SqlErr::SqliteFailure(SqlErrInner{code: SqlErrorCode::ConstraintViolation, extended_code: 2067}, _)) => {},
                Err(err) => panic!("Rusqlite: RepoData: counstructor: group insert error: {}", err.to_string()),
            }
            let id_group = prepare::unwrap(stmt_group_get.query_row(named_params! {":name": group.to_str()}, |row| {
                let id: u32 = row.get(0)?;
                Ok(id)
            }));
            count_units += cfg_group.units.len();
            let mut map_units: HashMap<Unit, u32> = HashMap::with_capacity(cfg_group.units.len());
            for (unit, cfg_unit) in cfg_group.units.iter() {
                match stmt_unit_set.execute(named_params! {":id": &id_unit_new, ":id_group": &id_group, ":name": unit.to_str()}) {
                    Ok(_) => { id_unit_new += 1 },
                    Err(SqlErr::SqliteFailure(SqlErrInner{code: SqlErrorCode::ConstraintViolation, extended_code: 2067}, _)) => {},
                    Err(err) => panic!("Rusqlite: RepoData: counstructor: unit insert error: {}", err.to_string()),
                }
                // TODO: error below! Can't get unit by name!
                let id_unit = prepare::unwrap(stmt_unit_get.query_row(named_params! {":name": unit.to_str(), ":id_group": id_group}, |row| {
                    let id: u32 = row.get(0)?;
                    Ok(id)
                }));
                let record_last_opt = prepare::unwrap(stmt_data_get_last.query_row(named_params! {":id_unit": &id_unit}, |row| {
                    let id_record: u64 = row.get(0)?;
                    let time: i64 = row.get(1)?;
                    let upd_type: u8 = row.get(2)?;
                    let upd_val: Option<Vec<u8>> = row.get(3)?;
                    let update = Update::from_ser(upd_type, upd_val);
                    Ok(Record{
                        id: id_record,
                        is_saved: true,
                        time: time,
                        val: update,
                    })
                }).optional());
                let state = if let Some(record_last) = record_last_opt {
                    let id_record_count = prepare::unwrap(stmt_data_get_count.query_row(named_params! {":id_unit": &id_unit}, |row| {
                        let id: u64 = row.get(0)?;
                        Ok(id)
                    }));
                    StateUnit{
                        count_min: cfg_unit.count_min, 
                        count_max: cfg_unit.count_max,
                        count: id_record_count,
                        record_last: Some(record_last),
                    }
                } else {
                    StateUnit{
                        count_min: cfg_unit.count_min, 
                        count_max: cfg_unit.count_max,
                        count: 0,
                        record_last: None,
                    }
                };
                map_units.insert(unit.clone(), id_unit);
                map_state.insert(id_unit, state);
            }
            map_group.insert(group.clone(), map_units);
        }

        Self { 
            map_group,
            map_state,
            id_units_overflowed: VecDeque::with_capacity(count_units),
            stmt_data_get: prepare::stmt_data_get(conn),
            stmt_data_rm_old_count: prepare::stmt_data_rm_old_count(conn),
            stmt_data_push: prepare::stmt_data_push(conn),
        }
    }

    pub fn data_push(&mut self, data: Data<Update>) -> Option<(usize, Data<Record<Update>>)> {
        match data {
            Data::Single { group, unit, update } => {
                if let Some(map_units) = self.map_group.get(&group) {
                    if let Some(id_unit) = map_units.get(&unit) {
                        if let Some(state_unit) = self.map_state.get_mut(id_unit) {
                            let id_record = if let Some(record_last) = state_unit.record_last.as_ref() { record_last.id + 1 } else { 0 };
                            let mut record = Record::establish_now(id_record,update);
                            state_unit.record_last = Some(record.clone());
                            state_unit.count += 1;
                            if state_unit.count >= state_unit.count_max { self.id_units_overflowed.push_back(*id_unit) }
                            let id_unit_owned = id_unit.clone();
                            self.data_push_single(id_unit_owned, &mut record);
                            Some((1, Data::Single { group, unit, update: record }))
                        } else {
                            // TODO: LOG unsynchronised
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            },
            Data::Multi { vec } => {
                let mut vec_record: Vec<(Group, Vec<(Unit, Record<Update>)>)> = Vec::with_capacity(vec.len());
                let mut count_total: usize = 0;
                for (group, vec_unit) in vec {
                    if let Some(vec_unit_record) = self.data_push_group(&group, vec_unit) {
                        count_total = count_total.checked_add(vec_unit_record.len()).unwrap_or(usize::MAX);
                        vec_record.push((group, vec_unit_record));
                    }
                }
                if vec_record.is_empty() {
                    None
                } else {
                    Some((count_total, Data::Multi { vec: vec_record }))
                }
            },
        }
    }

    pub fn overflow_resolve(&mut self) {
        while let Some(id_unit) = self.id_units_overflowed.pop_front() {
            // println!("[DB] overflow_resolve id_unit={}", id_unit);
            if let Some(state_unit) = self.map_state.get_mut(&id_unit) {
                // println!("[DB] overflow_resolve state_unit={} count={}", id_unit, state_unit.count);
                if let Err (err) = self.stmt_data_rm_old_count.execute(named_params! {
                    ":id_unit": &id_unit,
                    ":offset": &state_unit.count_min,
                }) {
                    // TODO: log error: SQL error
                    println!("Db:RepoData:overflow_resolve error: {}", err);
                } else {
                    state_unit.count = state_unit.count.checked_sub(state_unit.count_min).unwrap_or(0);
                }
            } else {
                // TODO: log error: unsynchronised
                println!("Db:RepoData:overflow_resolve unsynchronised error");
            }
        }
    }

    pub fn data_get(&mut self, group: &Group, unit: &Unit, idx_min: u64, idx_max: u64) -> Result<Vec<Record<Update>>, ()> {
        if let Some(map_unit) = self.map_group.get(group) {
            if let Some(id_unit) = map_unit.get(unit) {
                // :id_unit :id_record_min :id_record_max
                let iter_res = self.stmt_data_get.query_map(named_params! {
                    ":id_unit": id_unit,
                    ":id_record_min": idx_min,
                    ":id_record_max": idx_max,
                }, |row| {
                    // id_record, time, type, val
                    let id_record: u64 = row.get(0)?;
                    let time: i64 = row.get(1)?;
                    let upd_type: u8 = row.get(2)?;
                    let upd_val: Option<Vec<u8>> = row.get(3)?;
                    let update = Update::from_ser(upd_type, upd_val);
                    Ok(Record{
                        id: id_record,
                        is_saved: true,
                        time: time,
                        val: update,
                    })
                });

                match iter_res {
                    Ok(iter) => {
                        let vec: Vec<Record<Update>> = iter.filter_map(|record_res| record_res.ok()).collect();
                        return Ok(vec);
                    },
                    Err(err) => {
                        println!("[RepoData] data_get: rusqlite error: {}", err);
                        return Err(());
                    },
                }
            }
        }
        // TODO: check all 'else'
        Err(()) // bad request
    }

    pub fn data_last(&self, map: HashMap<Group, Vec<Unit>>) -> HashMap<Group, Vec<(Unit, Option<Record<Update>>)>> {
        let mut map_res = HashMap::with_capacity(map.len());
        for (group, units) in map {
            let mut vec_res = Vec::with_capacity(units.len());
            if let Some(map_unit) = self.map_group.get(&group) {
                for unit in units {
                    let item = if let Some(id_unit) = map_unit.get(&unit) {
                        if let Some(state_unit) = self.map_state.get(id_unit) {
                            (unit, state_unit.record_last.clone())
                        } else {
                            (unit, None)
                        }
                    } else {
                        (unit, None)
                    };
                    vec_res.push(item);
                }
            } else {
                for unit in units {
                    vec_res.push((unit, None))
                }
            }
            map_res.insert(group, vec_res);
        } 
        map_res
    } 

    fn data_push_group(&mut self, group: &Group, vec_unit: Vec<(Unit, Update)>) -> Option<Vec<(Unit, Record<Update>)>> {
        let mut vec_unit_record: Vec<(Unit, Record<Update>)> = Vec::with_capacity(vec_unit.len());
        if let Some(map_units) = self.map_group.get(group) {
            let mut vec_insert = Vec::with_capacity(vec_unit.len());
            for (unit, update) in vec_unit {
                if let Some(id_unit) = map_units.get(&unit) {
                    if let Some(state_unit) = self.map_state.get_mut(id_unit) {
                        let id_record = if let Some(record_last) = state_unit.record_last.as_ref() { record_last.id + 1 } else { 0 };
                        let record = Record::establish_now(id_record, update);
                        state_unit.record_last = Some(record.clone());
                        state_unit.count += 1;
                        if state_unit.count >= state_unit.count_max { self.id_units_overflowed.push_back(*id_unit) }
                        vec_insert.push((*id_unit, unit, record));
                    } else {
                        // TODO: LOG unsynchronised
                    }
                } 
            }
            for (id_unit, unit, mut record) in vec_insert {
                self.data_push_single(id_unit, &mut record);
                vec_unit_record.push((unit, record));
            }
        }
        if vec_unit_record.is_empty() {
            None
        } else {
            Some(vec_unit_record)
        }        
    }

    fn data_push_single(&mut self, id_unit: u32, record: &mut Record<Update>) {
        let (upd_type, upd_val) = record.val.to_ser();
        match self.stmt_data_push.execute(named_params! {
            ":id_unit": id_unit, 
            ":id_record": record.id,
            ":time": record.time,
            ":type": upd_type,
            ":value": upd_val,
        }) {
            Ok(_) => record.is_saved = true,
            Err(err) => {
                // TODO: LOG
                println!("Db::RepoData::data_push_single error: {}", err.to_string());
            },
        }
    }

}

mod prepare {
    use rusqlite::{Connection, Statement, Error, OptionalExtension};

    pub fn stmt_group_get<'a>(conn: &'a Connection) -> Statement<'a> {
        unwrap(conn.prepare("SELECT id FROM groups WHERE name = :name LIMIT 1"))
    }
    pub fn stmt_group_set<'a>(conn: &'a Connection) -> Statement<'a> {
        unwrap(conn.prepare("INSERT INTO groups (id, name) VALUES (:id, :name)"))
    }

    pub fn stmt_unit_get<'a>(conn: &'a Connection) -> Statement<'a> {
        unwrap(conn.prepare("SELECT id FROM units WHERE name = :name AND fk_unit_group = :id_group LIMIT 1"))
    }
    pub fn stmt_unit_set<'a>(conn: &'a Connection) -> Statement<'a> {
        unwrap(conn.prepare("INSERT INTO units (id, fk_unit_group, name) VALUES (:id, :id_group, :name)"))
    }

    pub fn stmt_data_push<'a>(conn: &'a Connection) -> Statement<'a> {
        unwrap(conn.prepare("INSERT INTO data (fk_data_unit, id_record, time, type, val) VALUES (:id_unit, :id_record, :time, :type, :value)"))
    }
    pub fn stmt_data_get<'a>(conn: &'a Connection) -> Statement<'a> {
        unwrap(conn.prepare("SELECT id_record, time, type, val FROM data WHERE fk_data_unit = :id_unit AND id_record >= :id_record_min AND id_record <= :id_record_max"))
    }
    pub fn stmt_data_get_last<'a>(conn: &'a Connection) -> Statement<'a> {
        unwrap(conn.prepare("SELECT id_record, time, type, val FROM data WHERE fk_data_unit = :id_unit ORDER BY id_record DESC LIMIT 1"))
    }
    pub fn stmt_data_get_count<'a>(conn: &'a Connection) -> Statement<'a> {
        unwrap(conn.prepare("SELECT COUNT(*) FROM data WHERE fk_data_unit = :id_unit"))
    }
    pub fn stmt_data_rm_old_count<'a>(conn: &'a Connection) -> Statement<'a> {
        // DELETE FROM data WHERE rowid in (select rowid from data WHERE fk_data_unit = :id_unit ORDER BY rowid DESC limit -1 offset :offset);
        // unwrap(conn.prepare("DELETE FROM data WHERE rowid in (SELECT rowid FROM data WHERE fk_data_unit = :id_unit ORDER BY id_record ASC LIMIT :count)"))
        unwrap(conn.prepare("DELETE FROM data WHERE rowid in (select rowid from data WHERE fk_data_unit = :id_unit ORDER BY rowid DESC limit -1 offset :offset)"))
    }

    // To select all records from single group
    // select g.name, u.id, d.id_record, u.name, u.fk_unit_group, d.val from groups as g left join units as u on g.id = u.fk_unit_group left join data as d on u.id = d.fk_data_unit where g.name = 'local-group2';
    // select g.name, u.id, max(d.id_record), u.name, u.fk_unit_group, d.val from groups as g left join units as u on g.id = u.fk_unit_group left join data as d on u.id = d.fk_data_unit where g.name = 'local-group2' group by u.id;

    // (id_group_max, id_unit_max)
    pub fn init(conn: &Connection) -> (Option<u32>, Option<u32>) {
        unwrap(conn.execute(
        "CREATE TABLE IF NOT EXISTS groups(
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL UNIQUE
        )", []));
        
        unwrap(conn.execute(
        "CREATE TABLE IF NOT EXISTS units(
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            fk_unit_group INTEGER NOT NULL,
            FOREIGN KEY (fk_unit_group) REFERENCES groups(id),
            UNIQUE(name, fk_unit_group)
        )", []));
        
        unwrap(conn.execute(
        "CREATE TABLE IF NOT EXISTS data(
            id_record INTEGER NOT NULL,
            time INTEGER,
            type INTEGER NOT NULL,
            val BLOB,
            fk_data_unit INTEGER NOT NULL,
            FOREIGN KEY (fk_data_unit) REFERENCES units(id)
        )", []));
        unwrap(conn.execute(
        "CREATE INDEX IF NOT EXISTS index_data_record ON data
        (fk_data_unit, id_record)", [])); 
        unwrap(conn.execute(
        "CREATE INDEX IF NOT EXISTS index_data_time ON data
        (fk_data_unit, time)", []));

        let id_group_max = unwrap(conn.query_row("SELECT id FROM groups ORDER BY id DESC LIMIT 1", [], |row| {
            let id: u32 = row.get(0)?;
            Ok(id)
        }).optional());

        let id_unit_max = unwrap(conn.query_row("SELECT id FROM units ORDER BY id DESC LIMIT 1", [], |row| {
            let id: u32 = row.get(0)?;
            Ok(id)
        }).optional());

        (id_group_max, id_unit_max)
    }

    pub fn unwrap<T>(res: Result<T, Error>) -> T {
        match res {
            Ok(ok) => ok,
            Err(err) => panic!("Rusqlite: RepoData: prepare error: {}", err.to_string()),
        }
    }
}