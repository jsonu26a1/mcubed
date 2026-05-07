use std::collections::BTreeMap;
use std::rc::Rc;


pub type Name = Rc<str>;



pub struct Schema {
    models: Vec<Model>,
    model_names: BTreeMap<Name, usize>,
    // TODO I'm unsure if we actually need this field;
    // relations: Vec<Relation>,
}

impl Schema {
    pub fn models(&self) -> &[Model] {
        &self.models
    }
    pub fn get(&self, name: impl AsRef<str>) -> Option<&Model> {
        self.models.get(*self.model_names.get(name.as_ref())?)
    }
}



pub struct Model {
    id: usize,
    name: Name,
    fields: Vec<(Name, SchemaField)>,
    field_names: BTreeMap<Name, usize>,
}

impl Model {
    pub fn id(&self) -> usize {
        self.id
    }

    pub fn name(&self) -> &Name {
        &self.name
    }

    pub fn fields(&self) -> &[(Name, SchemaField)] {
        &self.fields
    }

    pub fn get(&self, name: impl AsRef<str>) -> Option<&(Name, SchemaField)> {
        self.fields.get(*self.field_names.get(name.as_ref())?)
    }
}


#[derive(Copy, Clone)]
pub enum FieldType {
    Text,
    Int,
    Real,
    Buffer,
}



pub enum SchemaField {
    Text,
    Int,
    Real,
    Buffer,
    RelationOne { model_id: usize, field: usize },
    RelationMany { model_id: usize, field: usize },
}

impl From<FieldType> for SchemaField {
    fn from(ftype: FieldType) -> Self {
        match ftype {
            FieldType::Text => Self::Text,
            FieldType::Int => Self::Int,
            FieldType::Real => Self::Real,
            FieldType::Buffer => Self::Buffer,
        }
    }
}


/*
I think I got some things backwards with "one-to-many";
suppose we have models Track and Album;
Track should have a field "album" which is SchemaField::RelationOne { model: Album, field: "tracks" };
Album should have a field "tracks" which is SchemaField::RelationMany { model: Track, field: "album" };
to declare this, we would call SchemaBuilder::relation(RelationKind::ManyToOne, Album, "tracks", Track, "album");
Album has many "tracks", but Track has one "album"; hence, RelationKind::ManyToOne

where I got confused was with struct Relation and it's field `many: bool`, and the semantics of "from" and "to".
but I'm not even sure we need that field! so I won't worry about it now.

*/

// pub struct Relation {
//     from_model: usize,
//     from_field: usize,
//     to_model: usize,
//     to_field: usize,
//     many: bool,
// }



pub enum RelationKind {
    ManyToMany,
    ManyToOne,
    OneToOne,
    // OneToMany,
}



pub struct SchemaBuilder {
    schema: Schema,
}

impl SchemaBuilder {
    pub fn new() -> Self {
        Self {
            schema: Schema {
                models: vec![],
                model_names: BTreeMap::new(),
                // relations: vec![],
            }
        }
    }

    pub fn finish(self) -> Schema {
        self.schema
    }

    pub fn model(&mut self, name: &str, fields: &[(&str, FieldType)]) -> Result<Name, String> {
        let schema = &mut self.schema;
        if schema.model_names.contains_key(name) {
            return Err("model name already exists".into());
        }
        let name: Name = name.into();
        let mut model = Model {
            id: schema.models.len(),
            name: name.clone(),
            fields: vec![],
            field_names: BTreeMap::new(),
        };
        for (fname, ftype) in fields {
            let fname: Name = (*fname).into();
            if model.field_names.contains_key(&fname) {
                return Err("field name already exists on model".into());
            }
            model.field_names.insert(fname.clone(), model.fields.len());
            model.fields.push((fname, (*ftype).into()));
            // model.fields.push((fname, SchemaField::from(*ftype)));
        }
        schema.model_names.insert(name.clone(), schema.models.len());
        schema.models.push(model);
        Ok(name)
    }

    pub fn relation<A1, A2, B1, B2>(&mut self, kind: RelationKind, a_model: A1, a_field: A2, b_model: B1, b_field: B2) -> Result<(), String>
    where
        A1: AsRef<str>,
        A2: Into<Name> + AsRef<str>,
        B1: AsRef<str>,
        B2: Into<Name> + AsRef<str>,
    {
        let schema = &mut self.schema;
        let a_id = *schema.model_names.get(a_model.as_ref()).ok_or_else(|| "model name not found")?;
        let b_id = *schema.model_names.get(b_model.as_ref()).ok_or_else(|| "model name not found")?;
        let a = &schema.models[a_id];
        let b = &schema.models[b_id];
        if a.field_names.contains_key(a_field.as_ref()) {
            return Err("field name already exists on model".into());
        }
        if b.field_names.contains_key(b_field.as_ref()) {
            return Err("field name already exists on model".into());
        }
        let a_fname = a_field.into();
        let a_fid = a.fields.len();
        let b_fname = b_field.into();
        let b_fid = b.fields.len();
        schema.models[a_id].field_names.insert(a_fname.clone(), a_fid);
        schema.models[b_id].field_names.insert(b_fname.clone(), b_fid);
        let (a_many, b_many) = match kind {
            RelationKind::ManyToMany => (true,  true),
            RelationKind::ManyToOne =>  (true,  false),
            RelationKind::OneToOne =>   (false, false),
            // RelationKind::OneToMany =>  (false, true),
        };
        if a_many {
            schema.models[a_id].fields.push((a_fname, SchemaField::RelationMany { model_id: b_id, field: b_fid }));
        } else {
            schema.models[a_id].fields.push((a_fname, SchemaField::RelationOne { model_id: b_id, field: b_fid }));
        }
        if b_many {
            schema.models[b_id].fields.push((b_fname, SchemaField::RelationMany { model_id: a_id, field: a_fid }));
        } else {
            schema.models[b_id].fields.push((b_fname, SchemaField::RelationOne { model_id: a_id, field: a_fid }));
        }
        // schema.relations.push(Relation { from_model: a.id, from_field: a_fid, to_model: b.id, to_field: b_fid, many: b_many });
        // schema.relations.push(Relation { from_model: b.id, from_field: b_fid, to_model: a.id, to_field: a_fid, many: a_many });
        Ok(())
    }
}

/*
TODO; calling SchemaBuilder::relation, many_model has to look up the Name in a btree. but we could create a "ModelRef" type,
which can either be a Name or an id: usize, with an IntoModelRef trait that converts an id: usize or Name (or the From/Into
trait would be better). then, SchemaBuilder::model could return an id: usize instead of a Name; and the API would be a bit
more flexible. but this would definitely be a premature optimization; schema building only happens once, it's easier to just
use Name to refer to a model, and there's not a huge benefit to supporting looking up by id: usize here. maybe later tho?
*/

