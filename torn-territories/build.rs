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
}

fn main() {
    let territories: HashMap<String, Territory> =
        serde_json::from_slice(include_bytes!("./static/territory_shapes.json")).unwrap();
    let mut map = phf_codegen::Map::<&'static str>::new();
    let mut storage: Vec<(String, String)> = Vec::new();

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

        write!(path, "]").unwrap();
        storage.push((id, path));
    }

    for (id, path) in &storage {
        map.entry(id, path);
    }

    let path = Path::new(&env::var("OUT_DIR").unwrap()).join("codegen.rs");
    let mut file = BufWriter::new(File::create(path).unwrap());

    writeln!(
        &mut file,
        "static PATHS: phf::Map<&'static str, &'static [svgtypes::SimplePathSegment]> = {};",
        map.build()
    )
    .unwrap();
}
