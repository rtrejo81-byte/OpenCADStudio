//! Volcado del OLE2FRAME de un DWG, para comparar la estructura que escribe
//! AutoCAD con la que escribe LGB CAD Studio. Uso:
//!   cargo run --release --example ole_dump -- "ruta.dwg"
use std::io::Cursor;

fn main() {
    let path = std::env::args().nth(1).expect("ruta del dwg");
    let bytes = std::fs::read(&path).expect("no se pudo leer el archivo");
    let doc = codec::io::dwg::DwgReader::from_stream(Cursor::new(bytes))
        .read()
        .expect("no se pudo abrir el DWG");

    for (handle, object) in doc.objects.iter() {
        let codec::objects::ObjectType::Entity(entity) = object else {
            continue;
        };
        let codec::entities::EntityType::Ole2Frame(frame) = entity else {
            continue;
        };
        println!("== OLE2FRAME handle {:X} ==", handle.value());
        println!("version: {}", frame.version);
        println!("source_application: {:?}", frame.source_application);
        println!("ole_object_type: {:?}", frame.ole_object_type);
        println!("is_paper_space: {}", frame.is_paper_space);
        println!("dwg_mode: {}  lock_aspect: {}", frame.dwg_mode, frame.lock_aspect);
        println!("upper_left: {:?}", frame.upper_left_corner);
        println!("lower_right: {:?}", frame.lower_right_corner);
        println!("envelope: {:?}", frame.envelope);
        println!(
            "storage: leading={} trailing={} compuesto={}",
            frame.storage.leading_records.len(),
            frame.storage.trailing_records.len(),
            frame.storage.compound_file.is_some()
        );
        if let Some(compound) = &frame.storage.compound_file {
            let mut texto = format!("{compound:?}");
            if texto.len() > 4000 {
                texto.truncate(4000);
                texto.push_str("…");
            }
            println!("--- compuesto ---");
            println!("{texto}");
        }
    }
}
