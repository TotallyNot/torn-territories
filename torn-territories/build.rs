use std::{
    collections::HashMap,
    env,
    fmt::Write as _,
    fs::File,
    io::{BufWriter, Write},
    path::Path,
};

#[derive(serde::Deserialize)]
struct Territory<'a> {
    shape: &'a str,
    db_id: i32,
    sector: u8,
    slots: u16,
    neighbors: Option<Vec<&'a str>>,
}

#[derive(PartialEq, Eq, Hash)]
pub struct TerritoryId([u8; 3]);

impl phf::PhfHash for TerritoryId {
    fn phf_hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.phf_hash(state)
    }
}

impl phf_shared::FmtConst for TerritoryId {
    fn fmt_const(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TerritoryId({:?})", &self.0)
    }
}

fn main() {
    let territories: HashMap<String, Territory> =
        serde_json::from_slice(include_bytes!("./static/territory_shapes.json")).unwrap();
    let mut map = phf_codegen::Map::<TerritoryId>::new();
    let mut storage: Vec<([u8; 3], String)> = Vec::new();

    for (id, tert) in territories {
        let mut path = "&[".to_owned();
        let mut first = true;

        for segment in svgtypes::SimplifyingPathParser::from(tert.shape) {
            let segment = segment.unwrap();
            if first {
                first = false;
            } else {
                write!(path, ",").unwrap();
            }

            match segment {
                svgtypes::SimplePathSegment::MoveTo { x, y } => {
                    write!(
                        path,
                        "svgtypes::SimplePathSegment::MoveTo {{ x: {}f64, y: {}f64 }}",
                        x, y
                    )
                    .unwrap();
                }
                svgtypes::SimplePathSegment::LineTo { x, y } => {
                    write!(
                        path,
                        "svgtypes::SimplePathSegment::LineTo {{ x: {}f64, y: {}f64 }}",
                        x, y
                    )
                    .unwrap();
                }
                svgtypes::SimplePathSegment::Quadratic { x1, y1, x, y } => {
                    write!(path, "svgtypes::SimplePathSegment::Quadratic {{ x1: {}f64, y1: {}f64, x: {}f64, y: {}f64 }}", x1, y1, x, y).unwrap();
                }
                svgtypes::SimplePathSegment::CurveTo {
                    x1,
                    y1,
                    x2,
                    y2,
                    x,
                    y,
                } => {
                    write!(path, "svgtypes::SimplePathSegment::CurveTo {{ x1: {}f64, y1: {}f64, x2: {}f64, y2: {}f64, x: {}f64, y: {}f64 }}", x1, y1, x2, y2, x, y).unwrap();
                }
                svgtypes::SimplePathSegment::ClosePath => {
                    write!(path, "svgtypes::SimplePathSegment::ClosePath").unwrap();
                }
            }
        }

        let id_bytes = id.as_bytes().try_into().unwrap();

        write!(path, "]").unwrap();

        let neighbors = tert
            .neighbors
            .unwrap_or_default()
            .into_iter()
            .map(|id| format!("TerritoryId({:?})", id.as_bytes()))
            .collect::<Vec<_>>()
            .join(",");

        storage.push((
            id_bytes,
            format!(
                "TerritoryInfo {{ sector: {}, db_id: {}, slots: {}, neighbors: &[{}], shape: {} }}",
                tert.sector, tert.db_id, tert.slots, neighbors, path
            ),
        ));
    }

    for (id, path) in storage {
        map.entry(TerritoryId(id), &path);
    }

    let path = Path::new(&env::var("OUT_DIR").unwrap()).join("codegen.rs");
    let mut file = BufWriter::new(File::create(path).unwrap());

    writeln!(
        &mut file,
        "static TERRITORY_INFO: phf::Map<TerritoryId, TerritoryInfo> = {};",
        map.build()
    )
    .unwrap();
}
