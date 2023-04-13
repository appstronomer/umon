use core::slice::{Iter};
use std::{
    hash::{Hash, Hasher}, 
    collections::{HashSet, hash_set::Iter as IterSet, HashMap, hash_map::Iter as IterMap},
};

use crate::model::{
    user::{Login},
    dataflow::{Group, Unit},
};


#[derive(Debug)]
pub struct Name {
    val: String,
}
impl Name {
    pub fn new(val: String) -> Self {
        Self { val }
    }
    pub fn into_string(self) -> String {
        self.val
    }
}
impl Clone for Name {
    fn clone(&self) -> Self {
        Self { val: self.val.clone() }
    }
}
impl Hash for Name {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.val.hash(state);
    }
}
impl PartialEq for Name {
    fn eq(&self, other: &Self) -> bool {
        self.val == other.val
    }
}
impl Eq for Name {}



#[derive(Debug)]
pub struct Wplace {
    name: Name,
    set_login: HashSet<Login>,
    pubtop: HashMap<Group, HashSet<Unit>>,
}
impl Wplace {
    pub fn new(name: Name, pubtop: HashMap<Group, HashSet<Unit>>) -> Self {
        Self {
            name, pubtop,
            set_login: HashSet::new(),
        }
    }

    pub fn remove_login(&mut self, login: &Login) {
        self.set_login.remove(login);
    }

    pub fn add_login(&mut self, login: Login) {
        self.set_login.insert(login);
    }

    pub fn iter_pubtop(&self) -> IterMap<Group, HashSet<Unit>> {
        self.pubtop.iter()
    }

    pub fn check_unit(&self, group: &Group, unit: &Unit) -> bool {
        if let Some(set) = self.pubtop.get(group) {
            set.contains(unit)
        } else {
            false
        }
    }

    pub fn iter_login(&self) -> IterSet<Login> {
        self.set_login.iter()
    }

    pub fn get_name(&self) -> &Name {
        &self.name
    }

    pub fn len_login(&self) -> usize {
        self.set_login.len()
    }

    pub fn len_pubtop(&self) -> usize {
        self.pubtop.len()
    }

    pub fn is_empty(&self) -> bool {
        self.set_login.is_empty()
    }
}