use std::{collections::HashMap, io::Cursor, ops::Deref, rc::Rc};

use image::{
    buffer::ConvertBuffer,
    codecs::tiff::TiffDecoder,
    imageops::{crop, overlay, replace},
    ColorType, GenericImageView, GrayImage, ImageDecoder,
};
use resvg::usvg::{self, NodeExt, Rect};
use rust_embed::RustEmbed;
use svgtypes::SimplePathSegment;

#[derive(Debug, Clone)]
pub enum TerritoryIdError {
    InvalidLength(usize),
    DoesNotExist,
    InvalidEncoding,
}

impl std::fmt::Display for TerritoryIdError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::InvalidLength(len) => write!(f, "InvalidLength: {len}"),
            Self::DoesNotExist => write!(f, "ID does not exist"),
            Self::InvalidEncoding => write!(f, "ID has invalid encoding"),
        }
    }
}

impl std::error::Error for TerritoryIdError {}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerritoryId([u8; 3]);

impl std::fmt::Debug for TerritoryId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        unsafe { write!(f, "TerritoryId({})", std::str::from_utf8_unchecked(&self.0)) }
    }
}

impl phf::PhfHash for TerritoryId {
    fn phf_hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.phf_hash(state)
    }
}

impl phf_shared::PhfBorrow<TerritoryId> for TerritoryId {
    fn borrow(&self) -> &TerritoryId {
        self
    }
}

impl std::str::FromStr for TerritoryId {
    type Err = TerritoryIdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if !s.is_ascii() {
            return Err(TerritoryIdError::InvalidEncoding);
        }

        let bytes = s.as_bytes();
        if bytes.len() != 3 {
            return Err(TerritoryIdError::InvalidLength(bytes.len()));
        }

        let mut id_bytes = [0; 3];
        id_bytes.copy_from_slice(bytes);
        let id = Self(id_bytes);

        if !TERRITORY_INFO.contains_key(&id) {
            Err(TerritoryIdError::DoesNotExist)
        } else {
            Ok(id)
        }
    }
}

impl std::fmt::Display for TerritoryId {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        unsafe { write!(f, "{}", std::str::from_utf8_unchecked(&self.0)) }
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for TerritoryId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct IdVisitor;
        impl<'de> serde::de::Visitor<'de> for IdVisitor {
            type Value = TerritoryId;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(formatter, "struct TerritoryId")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if !v.is_ascii() || v.as_bytes().len() != 3 {
                    return Err(serde::de::Error::invalid_value(
                        serde::de::Unexpected::Str(v),
                        &"A valid three letter territory ID",
                    ));
                }

                let mut id_bytes = [0; 3];
                id_bytes.copy_from_slice(v.as_bytes());

                let id = TerritoryId(id_bytes);
                if !TERRITORY_INFO.contains_key(&id) {
                    return Err(serde::de::Error::invalid_value(
                        serde::de::Unexpected::Str(v),
                        &"A valid three letter territory ID",
                    ));
                }
                Ok(id)
            }
        }

        deserializer.deserialize_str(IdVisitor)
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for TerritoryId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serde::Serialize::serialize(&self.to_string(), serializer)
    }
}

#[cfg(feature = "sqlx")]
impl<DB> sqlx::Type<DB> for TerritoryId
where
    DB: sqlx::Database,
    String: sqlx::Type<DB>,
{
    fn type_info() -> DB::TypeInfo {
        <String as sqlx::Type<DB>>::type_info()
    }

    fn compatible(ty: &<DB as sqlx::Database>::TypeInfo) -> bool {
        <String as sqlx::Type<DB>>::compatible(ty)
    }
}

#[cfg(feature = "sqlx")]
impl<DB> sqlx::Decode<'_, DB> for TerritoryId
where
    DB: sqlx::Database,
    String: for<'r> sqlx::Decode<'r, DB>,
{
    fn decode(
        value: <DB as sqlx::database::HasValueRef<'_>>::ValueRef,
    ) -> Result<Self, sqlx::error::BoxDynError> {
        let value = <String as sqlx::Decode<'_, DB>>::decode(value)?;
        Ok(value.parse()?)
    }
}

#[cfg(feature = "sqlx")]
impl<DB> sqlx::Encode<'_, DB> for TerritoryId
where
    DB: sqlx::Database,
    String: for<'q> sqlx::Encode<'q, DB>,
{
    fn encode_by_ref(
        &self,
        buf: &mut <DB as sqlx::database::HasArguments<'_>>::ArgumentBuffer,
    ) -> sqlx::encode::IsNull {
        let value = self.to_string();
        <String as sqlx::Encode<'_, DB>>::encode(value, buf)
    }

    fn encode(
        self,
        buf: &mut <DB as sqlx::database::HasArguments<'_>>::ArgumentBuffer,
    ) -> sqlx::encode::IsNull
    where
        Self: Sized,
    {
        let value = self.to_string();
        <String as sqlx::Encode<'_, DB>>::encode(value, buf)
    }
}

#[cfg(feature = "sea-orm")]
impl From<TerritoryId> for sea_orm::Value {
    fn from(value: TerritoryId) -> Self {
        Self::String(Some(value.to_string().into()))
    }
}

#[cfg(feature = "sea-orm")]
impl sea_orm::TryGetable for TerritoryId {
    fn try_get_by<I: sea_orm::ColIdx>(
        res: &sea_orm::QueryResult,
        index: I,
    ) -> Result<Self, sea_orm::TryGetError> {
        let value = String::try_get_by(res, index)?;
        value.parse().map_err(|why| {
            sea_orm::TryGetError::DbErr(sea_orm::DbErr::TryIntoErr {
                from: "String",
                into: "TerritoryId",
                source: Box::new(why),
            })
        })
    }
}

#[cfg(feature = "sea-orm")]
impl sea_orm::sea_query::ValueType for TerritoryId {
    fn try_from(v: sea_orm::Value) -> Result<Self, sea_orm::sea_query::ValueTypeErr> {
        match v {
            sea_orm::Value::String(Some(value)) => value
                .parse()
                .map_err(|_why| sea_orm::sea_query::ValueTypeErr),
            _ => Err(sea_orm::sea_query::ValueTypeErr),
        }
    }

    fn type_name() -> String {
        stringify!(TerritoryId).to_owned()
    }

    fn array_type() -> sea_orm::sea_query::ArrayType {
        sea_orm::sea_query::ArrayType::String
    }

    fn column_type() -> sea_orm::ColumnType {
        sea_orm::ColumnType::Char(Some(3))
    }
}

#[cfg(feature = "sea-orm")]
impl sea_orm::sea_query::Nullable for TerritoryId {
    fn null() -> sea_orm::Value {
        sea_orm::Value::String(None)
    }
}

impl TerritoryId {
    pub fn info(&self) -> &TerritoryInfo {
        TERRITORY_INFO.get(self).unwrap()
    }
}

pub struct TerritoryInfo {
    pub shape: &'static [svgtypes::SimplePathSegment],
    pub sector: u8,
    pub db_id: i32,
    pub slots: u16,
    pub neighbors: &'static [TerritoryId],
}

include!(concat!(env!("OUT_DIR"), "/codegen.rs"));

pub const MAP_WIDTH: u32 = 6_256;
pub const MAP_HEIGHT: u32 = 3_648;
pub const MAP_BBOX: image::math::Rect = image::math::Rect {
    x: 0,
    y: 0,
    width: MAP_WIDTH,
    height: MAP_HEIGHT,
};
const TILE_WIDTH: u32 = 600;
const TILE_HEIGHT: u32 = 400;

#[derive(RustEmbed)]
#[folder = "static/map_tiles"]
#[include = "*.tiff"]
struct MapTiles;

pub fn path_for_territory(id: TerritoryId) -> Option<usvg::tiny_skia_path::Path> {
    let instructions = id.info().shape;

    let mut builder = usvg::tiny_skia_path::PathBuilder::new();

    for inst in instructions {
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
    id: TerritoryId,
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
    let width = (bbox.width() as u32).min(MAP_WIDTH);
    let height = (bbox.height() as u32).min(MAP_HEIGHT);
    let x = (bbox.x() as u32).clamp(0, MAP_WIDTH - width);
    let y = (bbox.y() as u32).clamp(0, MAP_HEIGHT - height);

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

#[derive(Debug, Clone, Copy)]
pub enum RenderScale {
    X1,
    X4,
}

pub fn render_territories(
    view_port: image::math::Rect,
    fill: HashMap<TerritoryId, RenderInstruction>,
    mut stroke: HashMap<TerritoryId, RenderInstruction>,
    scale: RenderScale,
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

        let path = element_for_territory(*id, fill, border).unwrap();
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

        let path = element_for_territory(id, None, border).unwrap();
        root.append_kind(usvg::NodeKind::Path(path));
    }

    let scale_factor = match scale {
        RenderScale::X1 => 1,
        RenderScale::X4 => 4,
    };

    let scaled_width = view_port.width / scale_factor;
    let scaled_height = view_port.height / scale_factor;

    let tree = resvg::Tree::from_usvg(&usvg::Tree {
        size: usvg::Size::from_wh(scaled_width as f32, scaled_height as f32).unwrap(),
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

    let mut pixmap = resvg::tiny_skia::Pixmap::new(scaled_width, scaled_height).unwrap();
    tree.render(
        resvg::tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );

    let shapes = image::RgbaImage::from_raw(scaled_width, scaled_height, pixmap.take()).unwrap();
    let mut background = match scale {
        RenderScale::X1 => {
            load_map_segment(view_port.x, view_port.y, view_port.width, view_port.height).convert()
        }
        RenderScale::X4 => crop(
            &mut load_map_x4(),
            view_port.x / 4,
            view_port.y / 4,
            scaled_width,
            scaled_height,
        )
        .to_image()
        .convert(),
    };

    overlay(&mut background, &shapes, 0, 0);

    background
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

        if cursor.0 + width >= x + w {
            cursor = (x, cursor.1 + height);
        } else {
            cursor.0 += width;
        }
    }

    image
}

pub fn load_map_x4() -> GrayImage {
    let img = MapTiles::get("map_x4.tiff").unwrap();

    let decoder = TiffDecoder::new(Cursor::new(img.data)).unwrap();
    assert!(decoder.color_type() == ColorType::L8);
    let mut buf = vec![0; decoder.total_bytes() as usize];
    let (d_x, d_y) = decoder.dimensions();
    decoder.read_image(&mut buf).unwrap();

    GrayImage::from_raw(d_x, d_y, buf).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_id() {
        let _id: TerritoryId = "XOD".parse().unwrap();
    }

    #[cfg(feature = "serde")]
    #[test]
    fn test_json_value() {
        use std::str::FromStr;

        let value = serde_json::to_value(TerritoryId::from_str("GVE").unwrap()).unwrap();
        let _id: TerritoryId = serde_json::from_value(value).unwrap();
    }
}
