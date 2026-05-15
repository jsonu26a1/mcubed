use std::collections::HashMap;
use std::rc::Rc;

pub type Name = Rc<str>;

pub struct DatabaseSchema {
    // sorted by name
    models: Box<[Rc<ModelSchema>]>,
}

impl DatabaseSchema {
    pub fn models(&self) -> &[Rc<ModelSchema>] {
        &self.models
    }
}

pub struct ModelSchema {
    name: Name,
    fields: Box<[(Name, FieldSchema)]>,
    // maps from name to id (slice is sorted by field Name)
    fields_by_name: Box<[usize]>,
}

impl ModelSchema {
    pub fn name(&self) -> &Name {
        &self.name
    }

    pub fn fields(&self) -> &[(Name, FieldSchema)] {
        &self.fields
    }

    pub fn field(&self, name: impl AsRef<str>) -> Option<&(Name, FieldSchema)> {
        let name = name.as_ref();
        let id = self.fields_by_name.binary_search_by(|id| {
            let field = &self.fields[*id];
            name.cmp(&*field.0)
        }).ok()?;
        Some(&self.fields[id])
    }
}

struct ModelBuilder {
    name: Name,
    fields: HashMap<Name, (usize, FieldSchema)>,
}

impl ModelBuilder {
    fn finish(self) -> ModelSchema {
        let mut i = self.fields.into_iter().collect::<Vec<_>>();
        i.sort_by(|(a, _), (b, _)| a.cmp(b));
        let fields_by_name = i.iter().map(|&(_, (id, _))| id).collect();
        i.sort_by_key(|&(_, (id, _))| id);
        let fields = i.into_iter().map(|(name, (_, field))| (name, field)).collect();
        ModelSchema {
            name: self.name,
            fields,
            fields_by_name,
        }
    }
}

#[derive(Clone)]
pub enum FieldType {
    Int,
    Real,
    Text,
    Buffer,
    Tuple(Vec<FieldType>),
    List(Box<FieldType>),
}

#[derive(Clone)]
pub enum FieldSchema {
    Int,
    Real,
    Text,
    Buffer,
    Tuple(Vec<FieldType>),
    List(FieldType),
    RelationOne { model: Name, field: Name },
    RelationMany { model: Name, field: Name },
}

impl From<FieldType> for FieldSchema {
    fn from(ftype: FieldType) -> Self {
        match ftype {
            FieldType::Int => Self::Int,
            FieldType::Real => Self::Real,
            FieldType::Text => Self::Text,
            FieldType::Buffer => Self::Buffer,
            FieldType::Tuple(t) => Self::Tuple(t),
            FieldType::List(l) => Self::List(*l),
        }
    }
}

pub enum RelationKind {
    OneToOne,
    OneToMany,
    ManyToMany,
}

pub struct SchemaBuilder {
    models: HashMap<Name, ModelBuilder>,
}

impl SchemaBuilder {
    pub fn new() -> Self {
        Self {
            models: HashMap::new(),
        }
    }

    pub fn finish(self) -> DatabaseSchema {
        let mut models: Box<[Rc<ModelSchema>]> = self.models.into_values().map(|mb| Rc::new(mb.finish())).collect();
        models.sort_by(|a, b| a.name.cmp(&b.name));
        DatabaseSchema {
            models,
        }
    }

    pub fn model(&mut self, name: impl Into<Name> + AsRef<str>, fields: impl IntoIterator<Item=(impl Into<Name> + AsRef<str>, FieldType)>) -> Result<Name, String> {
        let name = name.as_ref();
        if self.models.contains_key(name) {
            return Err("model name already exists".into());
        }
        let name: Name = name.into();
        let mut model = ModelBuilder {
            name: name.clone(),
            fields: HashMap::new(),
        };
        for (fname, ftype) in fields {
            let fid = model.fields.len();
            // let fname: Name = ;
            if model.fields.contains_key(fname.as_ref()) {
                return Err("field name already exists on model".into());
            }
            model.fields.insert(fname.into(), (fid, ftype.into()));
        }
        self.models.insert(name.clone(), model);
        Ok(name)
    }

    pub fn relation<A1, A2, B1, B2>(&mut self, kind: RelationKind, model_a: A1, field_a: A2, model_b: B1, field_b: B2) -> Result<(), String>
    where
        A1: AsRef<str>,
        A2: Into<Name> + AsRef<str>,
        B1: AsRef<str>,
        B2: Into<Name> + AsRef<str>,
    {
        let model_a = model_a.as_ref();
        let model_b = model_b.as_ref();
        let a = self.models.get(model_a).ok_or_else(|| "model_a name not found")?;
        let b = self.models.get(model_b).ok_or_else(|| "model_b name not found")?;
        if a.fields.contains_key(field_a.as_ref()) {
            return Err("field name already exists on model_a".into());
        }
        if b.fields.contains_key(field_b.as_ref()) {
            return Err("field name already exists on model_b".into());
        }
        let model_a_name = a.name.clone();
        let model_b_name = b.name.clone();
        let fid_a = a.fields.len();
        let fid_b = b.fields.len();
        let field_a = field_a.into();
        let field_b = field_b.into();
        let (many_a, many_b) = match kind {
            RelationKind::OneToOne =>   (false, false),
            RelationKind::OneToMany =>  (false, true),
            RelationKind::ManyToMany => (true,  true),
        };
        if many_a {
            self.models.get_mut(model_a).unwrap().fields.insert(field_a.clone(), (fid_a, FieldSchema::RelationMany {
                model: model_b_name.clone(),
                field: field_b.clone(),
            }));
        } else {
            self.models.get_mut(model_a).unwrap().fields.insert(field_a.clone(), (fid_a, FieldSchema::RelationOne {
                model: model_b_name.clone(),
                field: field_b.clone(),
            }));
        }
        if many_b {
            self.models.get_mut(model_b).unwrap().fields.insert(field_b.clone(), (fid_b, FieldSchema::RelationMany {
                model: model_a_name.clone(),
                field: field_a.clone(),
            }));
        } else {
            self.models.get_mut(model_b).unwrap().fields.insert(field_b.clone(), (fid_b, FieldSchema::RelationOne {
                model: model_a_name.clone(),
                field: field_a.clone(),
            }));
        }
        Ok(())
    }
}
