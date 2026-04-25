

use crate::model::{ HasPrimaryKey, Id, Model, FieldType, Instance, Field, Tag };
use crate::db::{ DbHandle, TableHandle };

use redb::{ ReadableDatabase, ReadableTable };

use eframe::egui;

pub struct App {
    db: DbHandle,
}

impl App {
    pub fn new() -> Result<Self, anyhow::Error> {
        let rdb = redb::Database::open("local_demo.redb")?;
        let rtxn = rdb.begin_read()?;
        let mut db = DbHandle::new(&rtxn)?;
        let mut keys = vec![];
        // for i in ReadableTable::iter(db.instances.table)? {
        for i in db.instances.table.iter()? {
            keys.push(Id::new(i?.0.value()));
        }
        for key in keys {
            let v = db.instances.get(key)?;
            println!("- {v:#?}");
        }
        Ok(Self { db })
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.heading("My egui Application");
        });
    }
}

fn render_instance()