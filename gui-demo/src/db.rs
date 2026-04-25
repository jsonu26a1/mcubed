use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::rc::Rc;

use crate::model::{ HasPrimaryKey, Id, Model, FieldType, Instance, Field, Tag };

// use redb::{ Database as ReDatabase, Error, ReadableDatabase, TableDefinition };
use redb::{ Database as ReDatabase, Error, TableDefinition, ReadOnlyTable, ReadTransaction, ReadableTable, WriteTransaction, TableHandle as ReTableHandle };
use postcard::{from_bytes, to_allocvec};
use serde::{Serialize, Deserialize};

pub type GenericTable<'n, 'a> = TableDefinition<'n, u64, &'a [u8]>;

// not using this?
// const TABLE: TableDefinition<u64, &[u8]> = TableDefinition::new()



// this would need repeated impl for checking cache, then getting from database, and inserting into cache
// #[derive(Default)]
// struct DataCache {
//     models: HashMap<Id<Model>, (Model, HashMap<Id<Instance>, Instance>)>,
//     tags: HashMap<Id<Tag>, Tag>,
// }

// #[derive(Default)]
// pub struct Cache {
//     pub models: HashMap<Id<Model>, Model>,
//     pub instances: HashMap<Id<Instance>, Instance>,
//     pub tags: HashMap<Id<Tag>, Tag>,
// }

// impl Cache {
//     pub fn get_model(&self, id: &Id<Model>) -> Option<&Model> {
//         self.models.get(id)
//     }
// }

// pub struct DbHandle {
//     models: ReadOnlyTable<u64, &'static [u8]>,
//     instances: ReadOnlyTable<u64, &'static [u8]>,
//     tags: ReadOnlyTable<u64, &'static [u8]>,
//     cache: RefCell<Cache>,
// }

// impl DbHandle {
//     pub fn new(rtxn: &ReadTransaction) -> Result<Self, redb::Error> {
//         Ok(Self {
//             models: rtxn.open_table(GenericTable::new("models"))?,
//             instances: rtxn.open_table(GenericTable::new("instances"))?,
//             tags: rtxn.open_table(GenericTable::new("tags"))?,
//             cache: Default::default(),
//         })
//     }

//     pub fn get_model(&self, id: &Id<Model>) -> Result<Option<&Model>, anyhow::Error> {
//         if let Some(model) = self.cache.models.get(id) {
//             return Ok(Some(model));
//         }
//         let value = match self.models.get(id.i)? {
//             Some(v) => v,
//             None => return Ok(None),
//         };
//         let mut model = from_bytes::<Model>(value)?;
//         model.id = *id;
//         // self.cache.models.insert()
//         let entry = self.cache.models.entry(*id).insert_entry(model);
//         Ok(Some(entry.get))
//     }
// }



// I think we need Rc<T> in the hashmap
// pub struct TableHandle<T> {
//     table: ReadOnlyTable<u64, &'static [u8]>,
//     cache: HashMap<Id<T>, T>,
// }

// impl<'t, T: HasPrimaryKey + Deserialize<'t>> TableHandle<T> {
//     // fn fetch_from_table(&self, id) -> Result<Option<&T>
//     pub fn get(&mut self, id: &Id<T>) -> Result<Option<&T>, anyhow::Error> {
//         {
//             if let Some(v) = self.cache.get(id) {
//                 return Ok(Some(v));
//             }
//         }
//         // let b = match self.table.get(id.i)? {
//         //     Some(g) => g.value(),
//         //     None => return Ok(None),
//         // };
//         let g = match self.table.get(id.i)? {
//             Some(g) => g,
//             None => return Ok(None),
//         };
//         let b = g.value();

//         let mut v = postcard::from_bytes::<T>(b"test")?;
//         // let mut v = postcard::from_bytes::<T>(b)?;
//         v.set_key(*id);
//         // let entry = self.cache.entry(*id).insert_entry(v);
//         // Ok(Some(entry.get()))
//         self.cache.insert(*id, v);
//         Ok(self.cache.get(id))
//     }
// }

pub struct DbHandle {
    pub models: TableHandle<Model>,
    pub instances: TableHandle<Instance>,
    pub tags: TableHandle<Tag>,
}

impl DbHandle {
    pub fn new(rtxn: &ReadTransaction) -> Result<Self, redb::TableError> {
        Ok(Self {
            models: TableHandle::new(rtxn.open_table(GenericTable::new("models"))?),
            instances: TableHandle::new(rtxn.open_table(GenericTable::new("instances"))?),
            tags: TableHandle::new(rtxn.open_table(GenericTable::new("tags"))?),
        })
    }

    pub fn commit_write(&mut self, w: &WriteTransaction) -> anyhow::Result<()> {
        self.models.commit_write(w)?;
        self.instances.commit_write(w)?;
        self.tags.commit_write(w)?;
        Ok(())
    }

    pub fn commit_clear(&mut self) {
        self.models.commit_clear();
        self.instances.commit_clear();
        self.tags.commit_clear();
    }
}

pub struct TableHandle<T> {
    pub table: ReadOnlyTable<u64, &'static [u8]>,
    cache: HashMap<Id<T>, Rc<T>>,
    // needs_commit: bool,
    next_id: Id<T>,
    create: Vec<Id<T>>,
    // modify: HashMap<Id<T>, Rc<T>>,
    modify: Vec<Id<T>>,
    delete: Vec<Id<T>>,
}

impl<T: HasPrimaryKey + Default + for<'a> Deserialize<'a> + Serialize> TableHandle<T> {
    pub fn new(table: ReadOnlyTable<u64, &'static [u8]>) -> Self {
        Self {
            table,
            cache: HashMap::new(),
            // needs_commit: false,
            next_id: Id::new(0),
            create: vec![],
            // modify: HashMap::new(),
            modify: vec![],
            delete: vec![],
        }
    }

    pub fn get(&mut self, id: Id<T>) -> anyhow::Result<Option<Rc<T>>> {
        if let Some(v) = self.cache.get(&id) {
            if !v.get_key().is_valid() {
                return Ok(None);
            }
            return Ok(Some(v.clone()));
        }
        let g = match self.table.get(*id)? {
            Some(g) => g,
            None => return Ok(None),
        };
        let mut v = postcard::from_bytes::<T>(g.value())?;
        v.set_key(id);
        let v = Rc::new(v);
        // let entry = self.cache.entry(id).insert_entry(v);
        // Ok(Some(entry.get().clone()))
        self.cache.insert(id, v.clone());
        Ok(Some(v))
    }

    pub fn insert(&mut self, mut v: T) -> anyhow::Result<Rc<T>> {
        // TODO move this next_id initialization into TableHandle::new(); so this would
        // just return Rc<T>, and new() would be anyhow::Result<Self>
        if !self.next_id.is_valid() {
            let n = match self.table.last()? {
                Some(e) => e.0.value(),
                None => 0,
            };
            self.next_id = Id::new(n + 1);
        }
        self.create.push(self.next_id);
        v.set_key(self.next_id);
        self.next_id = Id::new(*self.next_id + 1);
        let v = Rc::new(v);
        self.cache.insert(v.get_key(), v.clone());
        Ok(v)
    }

    // user clones existing data structure, and "re-inserts" it with same key/id
    pub fn modify(&mut self, v: T) {
        self.modify.push(v.get_key());
        self.cache.insert(v.get_key(), Rc::new(v));
        // TODO should we validate anything? id.is_valid()? id already exists?
        // right now, we don't care that the Rc<T> we're replacing might have existing refs
        // to it; all that matters is that the cache has the current version.
    }

    // so, for modify(): can we allow attempting to modify in place? if we use Rc::get_mut(),
    // we could enable updating it. however, is this practical? for now, just impl the cloning
    // TableHandle::modify(); we can revisit this idea in the future.
    // pub fn modify_in_place(&mut self, )

    // now, I impl this just out of routine; but in practice, we don't usually want to delete data,
    // but just mark it as "deleted" so it can be rolled back. but this should actually remove the
    // entry from the database, just as a "routine" (for testing, whatever).
    // NOTE / TODO: we don't remove items from the cache; should we? if we want items to appear
    // "removed", we would need get() to check this list to make sure it's not re-fetching keys
    // it shouldn't; use a HashSet/BTreeSet? alternatively, we could mark the item in the cache
    // with an invalid key (the Value, the Key would still point to the invalid/removed Value)
    // then get() would check this and return None if the valid is invalid. let's do that actually.
    pub fn remove(&mut self, id: Id<T>) {
        self.delete.push(id);
        // mark entry in cache as invalid; get() will return None for "invalid" values
        match self.cache.entry(id) {
            Entry::Occupied(o) => {
                let r = o.into_mut();
                match Rc::get_mut(r) {
                    // mark Rc<T> invalid in place
                    Some(m) => *m = T::default(),
                    // otherwise insert new invalid Rc<T> in cache
                    None => *r = Rc::new(T::default()),
                };
            },
            // insert invalid Rc<T> in cache so it won't get fetched from table
            Entry::Vacant(v) => { v.insert(Rc::new(T::default())); },
        }
    }

    pub fn commit_write(&mut self, w: &WriteTransaction) -> anyhow::Result<()> {
        let mut wtable = w.open_table(GenericTable::new(self.table.name()))?;
        // for key in self.create {
        for key in self.create.iter().chain(self.modify.iter()) {
            let value = match self.cache.get(key) {
                // Some(v) => (**v).to_allocvec()?,
                Some(v) => postcard::to_allocvec::<T>(v)?,
                None => continue,
            };
            wtable.insert(**key, value.as_slice())?;
        }
        // for key in self.modify {
        //     let value = match self.cache.get(key) {
        //         Some(v) => postcard::to_allocvec::<T>(v)?,
        //         None => continue,
        //     };
        //     wtable.insert(*key, value)?;
        // }
        for key in self.delete.iter() {
            wtable.remove(**key)?;
        }
        // self.create.clear();
        // self.modify.clear();
        // self.delete.clear();
        Ok(())
    }

    // call commit_write() first, and call this after WriteTransaction::commit()
    pub fn commit_clear(&mut self) {
        self.create.clear();
        self.modify.clear();
        self.delete.clear();
    }

    pub fn cache_clear(&mut self) {
        self.cache.clear();
    }

    pub fn needs_commit(&self) -> bool {
        !self.create.is_empty() && !self.modify.is_empty() && !self.delete.is_empty()
    }
}



// original
// pub struct TableHandle<T> {
//     table: ReadOnlyTable<u64, &'static [u8]>,
//     cache: HashMap<Id<T>, Rc<T>>,
// }

// impl<T: HasPrimaryKey + for<'a> Deserialize<'a>> TableHandle<T> {
//     pub fn new(table: ReadOnlyTable<u64, &'static [u8]>) -> Self {
//         Self {
//             table,
//             cache: HashMap::new(),
//         }
//     }

//     pub fn get(&mut self, id: &Id<T>) -> anyhow::Result<Option<Rc<T>>> {
//         if let Some(v) = self.cache.get(id) {
//             return Ok(Some(v.clone()));
//         }
//         let g = match self.table.get(id.i)? {
//             Some(g) => g,
//             None => return Ok(None),
//         };
//         let mut v = postcard::from_bytes::<T>(g.value())?;
//         v.set_key(*id);
//         let v = Rc::new(v);
//         let entry = self.cache.entry(*id).insert_entry(v);
//         Ok(Some(entry.get().clone()))
//     }
// }



// alternative idea for a TableHandle; avoids Rc<T> in HashMap, but requires all refs
// to exist in the cache. fetch_and_cache() requires there are no refs to the cache
// pub struct TableHandle2<T> {
//     table: ReadOnlyTable<u64, &'static [u8]>,
//     cache: HashMap<Id<T>, T>,
// }

// impl<T: HasPrimaryKey + for<'a> Deserialize<'a>> TableHandle2<T> {
//     pub fn new(table: ReadOnlyTable<u64, &'static [u8]>) -> Self {
//         Self {
//             table,
//             cache: HashMap::new(),
//         }
//     }

//     pub fn fetch_and_cache(&mut self, id: &Id<T>) -> anyhow::Result<bool> {
//         if self.cache.contains_key(id) {
//             return Ok(true);
//         }
//         let g = match self.table.get(id.i)? {
//             Some(g) => g,
//             None => return Ok(false),
//         };
//         let mut v = postcard::from_bytes::<T>(g.value())?;
//         v.set_key(*id);
//         // let v = Rc::new(v);
//         // let entry = self.cache.entry(*id).insert_entry(v);
//         self.cache.insert(*id, v);
//         Ok(true)
//     }

//     pub fn get(&self, id: &Id<T>) -> Option<&T> {
//         self.cache.get(id)
//     }
// }