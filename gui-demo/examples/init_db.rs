use gui_demo::db::{ DbHandle, GenericTable };
use gui_demo::model::{ Id, Model, FieldType, Instance, Field, Tag };

use redb::{ ReadableDatabase  };

fn main() -> anyhow::Result<()> {
    let mut db = redb::Database::create("local_demo.redb")?;
    {
        // in case the database doesn't exist, create all the tables
        let w = db.begin_write()?;
        w.open_table(GenericTable::new("models"))?;
        w.open_table(GenericTable::new("instances"))?;
        w.open_table(GenericTable::new("tags"))?;
        w.commit()?;
    }
    let mut dbh = DbHandle::new(&db.begin_read()?)?;
    let model = dbh.models.insert(Model {
        name: "person".to_string(),
        fields: vec![
            ("name".to_string(), FieldType::Text),
            ("age".to_string(), FieldType::Int),
        ],
        ..Default::default()
    })?;
    for person in [("dave", 31), ("sue", 32)] {
        dbh.instances.insert(Instance {
            model_id: model.id,
            fields: vec![
                Field::Text(person.0.to_string()),
                Field::Int(person.1)
            ],
            ..Default::default()
        })?;
    }
    let w = db.begin_write()?;
    dbh.commit_write(&w)?;
    w.commit()?;
    dbh.commit_clear();
    Ok(())
}
