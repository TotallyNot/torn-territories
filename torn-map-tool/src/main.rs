use std::{collections::HashMap, io::Write};

use clap::{Args, Parser, Subcommand, ValueEnum};
use image::{
    buffer::ConvertBuffer, codecs::png::PngEncoder, DynamicImage, GenericImageView, ImageEncoder,
    ImageFormat,
};
use torn_territories::fit_view_box;

#[derive(Parser)]
#[command(author, about, version)]
struct Cli {
    /// name of the file that the resulting image should be written to
    #[arg(short, long)]
    output_file: Option<String>,

    #[arg(short, long, value_enum, default_value_t = OutputFormat::Png)]
    format: OutputFormat,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    MapSegment(MapSegmentArgs),
    TerritoryView(TerritoryViewArgs),
}

#[derive(Args)]
struct MapSegmentArgs {
    /// X position of the upper left corner of the selected rectangle
    #[arg(short, long, value_parser = clap::value_parser!(u32).range(0..(torn_territories::MAP_WIDTH as i64)))]
    x_position: u32,

    /// Y position of the upper left corner of the selected rectangle
    #[arg(short, long, value_parser = clap::value_parser!(u32).range(0..(torn_territories::MAP_HEIGHT as i64)))]
    y_position: u32,

    #[command(flatten)]
    x2_spec: X2Spec,

    #[command(flatten)]
    y2_spec: Y2Spec,
}

#[derive(Args)]
#[group(required = true, multiple = false)]
struct X2Spec {
    /// X position of the lower right corner of the selected rectangle
    #[arg(long, value_parser = clap::value_parser!(u32).range(0..(torn_territories::MAP_WIDTH as i64)))]
    x2_position: Option<u32>,

    /// Width of the selected rectangle
    #[arg(long, value_parser = clap::value_parser!(u32).range(0..(torn_territories::MAP_WIDTH as i64)))]
    width: Option<u32>,
}

#[derive(Args)]
#[group(required = true, multiple = false)]
struct Y2Spec {
    /// Y position of the lower right corner of the selected rectangle
    #[arg(long, value_parser = clap::value_parser!(u32).range(0..(torn_territories::MAP_WIDTH as i64)))]
    y2_position: Option<u32>,

    /// Height of the selected rectangle
    #[arg(long, value_parser = clap::value_parser!(u32).range(0..(torn_territories::MAP_WIDTH as i64)))]
    height: Option<u32>,
}

#[derive(Args)]
struct TerritoryViewArgs {
    #[arg(short, long, default_value_t = 1f32)]
    factor: f32,

    #[arg(short, long, default_value_t = 4f32/3f32)]
    aspect_ratio: f32,

    #[arg(long, num_args(0..), value_parser = parse_rendering_instructions)]
    fill: Vec<HashMap<String, torn_territories::RenderInstruction>>,

    #[arg(long, num_args(0..), value_parser = parse_rendering_instructions)]
    border: Vec<HashMap<String, torn_territories::RenderInstruction>>,

    territory: String,
}

fn parse_rendering_instructions(
    s: &str,
) -> Result<HashMap<String, torn_territories::RenderInstruction>, String> {
    let (colour, rest) = s
        .split_once(':')
        .ok_or("invalid rendering instruction. Expected <colour>:<opacity>:<territory ids>")?;

    let colour = if colour.starts_with('#') {
        if colour.len() != 7 {
            return Err(format!("invalid hex colour '{colour}'"));
        }

        let r = u8::from_str_radix(&colour[1..=2], 16)
            .map_err(|why| format!("invalid hex colour '{colour}': {why}"))?;
        let g = u8::from_str_radix(&colour[3..=4], 16)
            .map_err(|why| format!("invalid hex colour '{colour}': {why}"))?;
        let b = u8::from_str_radix(&colour[5..=6], 16)
            .map_err(|why| format!("invalid hex colour '{colour}': {why}"))?;

        usvg::Color::new_rgb(r, g, b)
    } else {
        return Err(format!("invalid colour format '{colour}'"));
    };

    let (opacity, terts) = rest
        .split_once(':')
        .ok_or("invalid rendering instruction. Expected <colour>:<opacity>:<territory ids>")?;

    let opacity: f32 = opacity
        .parse()
        .map_err(|why| format!("invalid opacity '{opacity}': {why}"))?;
    if !(0f32..=1f32).contains(&opacity) {
        return Err(format!(
            "invlaid opacity {opacity}. Needs to a value between 0.0 and 1.0"
        ));
    }

    let inst = torn_territories::RenderInstruction { colour, opacity };

    Ok(HashMap::from_iter(
        terts.split(',').map(|id| (id.to_owned(), inst.clone())),
    ))
}

#[derive(Debug, Clone, PartialEq, Eq, ValueEnum)]
enum OutputFormat {
    Png,
    Tiff,
}

fn load_map_segment(args: MapSegmentArgs) -> DynamicImage {
    let w = args
        .x2_spec
        .width
        .unwrap_or_else(|| args.x2_spec.x2_position.unwrap() - args.x_position);
    let h = args
        .y2_spec
        .height
        .unwrap_or_else(|| args.y2_spec.y2_position.unwrap() - args.y_position);

    let image = torn_territories::load_map_segment(args.x_position, args.y_position, w, h);

    DynamicImage::ImageLuma8(image)
}

fn load_territory_view(args: TerritoryViewArgs) -> DynamicImage {
    let path = torn_territories::path_for_territory(&args.territory)
        .unwrap_or_else(|| panic!("Territory with id '{}' does not exist!", args.territory));
    let bbox = torn_territories::bbox_for_path(&path, args.factor, args.aspect_ratio);

    let fitted_box = fit_view_box(bbox);

    let mut result: image::RgbaImage = torn_territories::load_map_segment(
        fitted_box.x,
        fitted_box.y,
        fitted_box.width,
        fitted_box.height,
    )
    .convert();

    let shapes = torn_territories::render_territories(
        fitted_box,
        HashMap::from([(
            args.territory,
            torn_territories::RenderInstruction {
                colour: usvg::Color::new_rgb(255, 0, 0),
                opacity: 0.5,
            },
        )]),
        HashMap::new(),
    );

    image::imageops::overlay(&mut result, &shapes, 0, 0);

    DynamicImage::ImageRgba8(result)
}

fn main() {
    let cli = Cli::parse();

    let image = match cli.command {
        Commands::MapSegment(args) => load_map_segment(args),
        Commands::TerritoryView(args) => load_territory_view(args),
    };

    if let Some(out_file) = cli.output_file {
        let mut file = std::fs::File::create(out_file).unwrap();
        match cli.format {
            OutputFormat::Tiff => image.write_to(&mut file, ImageFormat::Tiff).unwrap(),
            OutputFormat::Png => image.write_to(&mut file, ImageFormat::Png).unwrap(),
        };
    } else {
        match cli.format {
            OutputFormat::Tiff => {
                let mut buf = vec![];
                image
                    .write_to(&mut std::io::Cursor::new(&mut buf), ImageFormat::Tiff)
                    .unwrap();
                std::io::stdout().write_all(&buf).unwrap();
            }
            OutputFormat::Png => {
                let out_stream = std::io::BufWriter::new(std::io::stdout());
                let (width, height) = image.dimensions();
                PngEncoder::new(out_stream)
                    .write_image(image.as_bytes(), width, height, image.color())
                    .unwrap();
            }
        }
    }
}
