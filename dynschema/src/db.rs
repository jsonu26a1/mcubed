use std::rc::Rc;
use std::collections::HashMap;

// use crate::schema::{ Name, Schema, Model, SchemaField, Relation, SchemaBuilder };
use crate::schema::{ self, Name };

pub type Key = u64;

pub struct Database {
    rtx: redb::ReadTransaction,
    models: Box<[Model]>,
    // sorted list for binary search lookup
    model_names: Rc<[(Name, usize)]>,
}

impl Database {
    pub fn new(schema: &schema::Schema, rtx: redb::ReadTransaction) -> Self {
        // TODO rewrite schema module first
        todo!();
    }
}

enum Change {
    Create,
    Update,
    Delete,
}

pub struct Model {
    table: redb::ReadOnlyTable<Key, &'static [u8]>,
    schema: Rc<schema::Model>,
    cache: HashMap<Key, Option<Rc<Instance>>>,
    next_key: Key,
    /*
        // we could just store a list of all changed Keys, but...
        // changes: Vec<(Key, Change)>,
        create: Vec<Key>,
        // maybe we want to check if an instance has already been modified? or Instance could have a dirty flag?
        modify: HashSet<Key>,
        delete: Vec<Key>,
    */
    // yeah, store a flat list of all changed keys, add dirty flag to Instance
    changes: Vec<(Key, Change)>,
}

impl Model {
    pub fn new(schema: Rc<schema::Model>) -> Self {
        
    }
}

pub struct Instance {
    schema: Rc<schema::Model>,
    key: Key,
    fields: Vec<FieldValue>,
    dirty: bool,
}

impl Instance {

}
