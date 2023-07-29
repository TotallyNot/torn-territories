use std::{collections::HashMap, io::Cursor, ops::Deref, rc::Rc};

use image::{
    codecs::tiff::TiffDecoder, imageops::replace, ColorType, GenericImageView, GrayImage,
    ImageDecoder,
};
use resvg::usvg::{self, NodeExt, Rect};
use rust_embed::RustEmbed;
use svgtypes::SimplePathSegment;

include!(concat!(env!("OUT_DIR"), "/codegen.rs"));

pub const MAP_WIDTH: u32 = 6_256;
pub const MAP_HEIGHT: u32 = 3_648;
const TILE_WIDTH: u32 = 600;
const TILE_HEIGHT: u32 = 400;

#[derive(RustEmbed)]
#[folder = "static/map_tiles"]
#[include = "*.tiff"]
struct MapTiles;

pub fn path_for_territory(id: &str) -> Option<usvg::tiny_skia_path::Path> {
    let instructions = PATHS.get(id)?;

    let mut builder = usvg::tiny_skia_path::PathBuilder::new();

    for inst in *instructions {
        match inst {
            SimplePathSegment::MoveTo { x, y } => {
                builder.move_to(*x as f32, *y as f32);
            }
            SimplePathSegment::LineTo { x, y } => {
                builder.line_to(*x as f32, *y as f32);
            }
            SimplePathSegment::Quadratic { x1, y1, x, y } => {
                builder.quad_to(*x1 as f32, *y1 as f32, *x as f32, *y as f32);
            }
            SimplePathSegment::CurveTo {
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            } => {
                builder.cubic_to(
                    *x1 as f32, *y1 as f32, *x2 as f32, *y2 as f32, *x as f32, *y as f32,
                );
            }
            SimplePathSegment::ClosePath => {
                builder.close();
            }
        }
    }

    builder.finish()
}

pub fn bbox_for_path(path: &usvg::tiny_skia_path::Path, factor: f32, ar: f32) -> Rect {
    let bounds = path.bounds();

    let bounds_ar = bounds.width() / bounds.height();
    if bounds_ar > ar {
        let width = bounds.width() / factor;
        let height = width / ar;

        let x = bounds.x() - (width * (1f32 - factor) / 2f32);
        let y = bounds.y() - (height - bounds.height()) / 2f32;

        Rect::from_xywh(x, y, width, height).unwrap()
    } else {
        let height = bounds.height() / factor;
        let width = height * ar;

        let y = bounds.y() - (height * (1f32 - factor) / 2f32);
        let x = bounds.x() - (width - bounds.width()) / 2f32;

        Rect::from_xywh(x, y, width, height).unwrap()
    }
}

pub fn colour_from_hex(hex: &str) -> Option<usvg::Color> {
    if !hex.starts_with('#') || hex.len() != 7 {
        return None;
    }

    let r = u8::from_str_radix(&hex[1..=2], 16).ok()?;
    let g = u8::from_str_radix(&hex[3..=4], 16).ok()?;
    let b = u8::from_str_radix(&hex[5..=6], 16).ok()?;

    Some(usvg::Color::new_rgb(r, g, b))
}

pub fn element_for_territory(
    id: &str,
    fill: Option<usvg::Fill>,
    stroke: Option<usvg::Stroke>,
) -> Option<usvg::Path> {
    Some(usvg::Path {
        id: "".to_owned(),
        transform: usvg::Transform::identity(),
        visibility: usvg::Visibility::Visible,
        fill,
        stroke,
        paint_order: usvg::PaintOrder::FillAndStroke,
        rendering_mode: usvg::ShapeRendering::CrispEdges,
        text_bbox: None,
        data: Rc::new(path_for_territory(id)?),
    })
}

pub fn fit_view_box(bbox: Rect) -> image::math::Rect {
    let x = bbox.x() as u32;
    let y = bbox.y() as u32;
    let width = ((bbox.x() + bbox.width()).min(MAP_WIDTH as f32) - bbox.x()) as u32;
    let height = ((bbox.y() + bbox.height()).min(MAP_HEIGHT as f32) - bbox.y()) as u32;

    image::math::Rect {
        x,
        y,
        width,
        height,
    }
}

#[derive(Debug, Clone)]
pub struct RenderInstruction {
    pub colour: usvg::Color,
    pub opacity: f32,
}

pub fn render_territories(
    view_port: image::math::Rect,
    fill: HashMap<String, RenderInstruction>,
    mut stroke: HashMap<String, RenderInstruction>,
) -> image::RgbaImage {
    let root = usvg::Node::new(usvg::NodeKind::Group(usvg::Group {
        id: "".to_owned(),
        transform: usvg::Transform::identity(),
        opacity: usvg::NormalizedF32::ONE,
        blend_mode: usvg::BlendMode::Normal,
        isolate: false,
        clip_path: None,
        mask: None,
        filters: vec![],
    }));

    for (id, inst) in &fill {
        let border = stroke.remove(id).map(|i| usvg::Stroke {
            paint: usvg::Paint::Color(i.colour),
            dasharray: None,
            dashoffset: 0f32,
            miterlimit: usvg::StrokeMiterlimit::new(4f32),
            opacity: usvg::NormalizedF32::new(i.opacity).unwrap(),
            width: usvg::NonZeroPositiveF32::new(4f32).unwrap(),
            linecap: usvg::LineCap::Butt,
            linejoin: usvg::LineJoin::Miter,
        });

        let fill = Some(usvg::Fill {
            paint: usvg::Paint::Color(inst.colour),
            opacity: usvg::NormalizedF32::new(inst.opacity).unwrap(),
            rule: usvg::FillRule::NonZero,
        });

        let path = element_for_territory(id, fill, border).unwrap();
        root.append_kind(usvg::NodeKind::Path(path));
    }

    for (id, inst) in stroke {
        let border = Some(usvg::Stroke {
            paint: usvg::Paint::Color(inst.colour),
            dasharray: None,
            dashoffset: 0f32,
            miterlimit: usvg::StrokeMiterlimit::new(4f32),
            opacity: usvg::NormalizedF32::new(inst.opacity).unwrap(),
            width: usvg::NonZeroPositiveF32::new(4f32).unwrap(),
            linecap: usvg::LineCap::Butt,
            linejoin: usvg::LineJoin::Miter,
        });

        let path = element_for_territory(&id, None, border).unwrap();
        root.append_kind(usvg::NodeKind::Path(path));
    }

    let tree = resvg::Tree::from_usvg(&usvg::Tree {
        size: usvg::Size::from_wh(view_port.width as f32, view_port.height as f32).unwrap(),
        view_box: usvg::ViewBox {
            rect: usvg::NonZeroRect::from_xywh(
                view_port.x as f32,
                view_port.y as f32,
                view_port.width as f32,
                view_port.height as f32,
            )
            .unwrap(),
            aspect: usvg::AspectRatio::default(),
        },
        root,
    });

    let mut pixmap = resvg::tiny_skia::Pixmap::new(view_port.width, view_port.height).unwrap();
    tree.render(
        resvg::tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );

    image::RgbaImage::from_raw(view_port.width, view_port.height, pixmap.take()).unwrap()
}

pub fn load_map_segment(x: u32, y: u32, w: u32, h: u32) -> GrayImage {
    let mut image = GrayImage::new(w, h);
    let mut cursor = (x, y);

    while cursor.1 < (y + h) {
        let x_tile = cursor.0 / TILE_WIDTH + 1;
        let y_tile = cursor.1 / TILE_HEIGHT + 1;
        let x_min = cursor.0 % TILE_WIDTH;
        let y_min = cursor.1 % TILE_HEIGHT;
        let width = ((x + w) - cursor.0).min(TILE_WIDTH - (cursor.0 % TILE_WIDTH));
        let height = ((y + h) - cursor.1).min(TILE_HEIGHT - (cursor.1 % TILE_HEIGHT));

        let tile = MapTiles::get(&format!("map_{x_tile}_{y_tile}.tiff")).unwrap();

        let decoder = TiffDecoder::new(Cursor::new(tile.data)).unwrap();
        assert!(decoder.color_type() == ColorType::L8);
        let mut buf = vec![0; decoder.total_bytes() as usize];
        let (d_x, d_y) = decoder.dimensions();
        decoder.read_image(&mut buf).unwrap();

        let tile = GrayImage::from_raw(d_x, d_y, buf).unwrap();
        let view = tile.view(x_min, y_min, width, height);
        replace(
            &mut image,
            view.deref(),
            (cursor.0 - x) as i64,
            (cursor.1 - y) as i64,
        );

        if cursor.0 + TILE_WIDTH > x + w {
            cursor = (x, cursor.1 + height);
        } else {
            cursor.0 += width;
        }
    }

    image
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        let image = super::load_map_segment(MAP_WIDTH - 2000, MAP_HEIGHT - 2000, 2000, 2000);
        image.save("test.tiff").unwrap();
    }
}
