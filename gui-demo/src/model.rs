use std::marker::PhantomData;
use std::ops::Deref;
use std::cmp::{ Eq, PartialEq };
use std::hash::{ Hash, Hasher };

use postcard::{from_bytes, to_allocvec};
use serde::{Serialize, Deserialize};

pub trait HasPrimaryKey {
    fn get_key(&self) -> Id<Self>;
    fn set_key(&mut self, key: Id<Self>);
}



#[derive(Serialize, Deserialize)]
pub struct Id<T: ?Sized> {
    i: u64,
    _t: PhantomData<*const T>,
}

impl<T> Id<T> {
    pub fn new(i: u64) -> Self {
        Id {
            i,
            _t: Default::default(),
        }
    }

    pub fn is_valid(&self) -> bool {
        self.i > 0
    }
}

impl<T> std::fmt::Debug for Id<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        // f.write_fmt(format_args!("Id({})", self.i))
        f.debug_tuple("Id").field(&self.i).finish()
    }
}

impl<T> Clone for Id<T> {
    fn clone(&self) -> Self {
        Id::new(self.i)
    }
}

impl<T> Copy for Id<T> {}

impl<T> Deref for Id<T> {
    type Target = u64;

    fn deref(&self) -> &u64 {
        &self.i
    }
}

impl<T> PartialEq for Id<T> {
    fn eq(&self, other: &Self) -> bool {
        self.i == other.i
    }
}

impl<T> Eq for Id<T> {}

impl<T> Hash for Id<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.i.hash(state);
    }
}

impl<T> Default for Id<T> {
    fn default() -> Self {
        Id::new(0)
    }
}



#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Model {
    #[serde(skip)]
    pub id: Id<Model>,
    pub name: String,
    pub fields: Vec<(String, FieldType)>,
}

impl HasPrimaryKey for Model {
    fn get_key(&self) -> Id<Self> {
        self.id
    }
    fn set_key(&mut self, key: Id<Self>) {
        self.id = key;
    }
}



#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
pub enum FieldType {
    Text,
    Int,
    Real,
    Ref(Id<Model>),
    Tag(Id<Tag>),
}



#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Instance {
    #[serde(skip)]
    pub id: Id<Instance>,
    // #[serde(skip)]
    // pub model: Rc<Model>,
    pub model_id: Id<Model>,
    pub fields: Vec<Field>,
}

impl HasPrimaryKey for Instance {
    fn get_key(&self) -> Id<Self> {
        self.id
    }
    fn set_key(&mut self, key: Id<Self>) {
        self.id = key;
    }
}



#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Field {
    Text(String),
    Int(u64),
    Real(f64),
    Ref(Id<Instance>),
    Tag(Vec<usize>),
}



#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Tag {
    #[serde(skip)]
    pub id: Id<Tag>,
    pub name: String,
    pub tags: Vec<String>,
}

impl HasPrimaryKey for Tag {
    fn get_key(&self) -> Id<Self> {
        self.id
    }
    fn set_key(&mut self, key: Id<Self>) {
        self.id = key;
    }
}

