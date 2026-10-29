//! Bounded ISOBMFF + AV1 still-image inspection for `image/avif`.
//!
//! Enforces the same dimension, pixel-product, decoded-byte (`MAX_DECODED_IMAGE_BYTES`),
//! non-animated (`avis`, `moov`, multi-frame OBU) and identity-orientation (`irot`, `imir`,
//! `clap`, `Exif`) barriers as the PNG and JPEG inspectors while preserving original bytes.
//!
//! Profile: one primary image that is either a single `av01` item or an ImageGrid of `av01`
//! tiles, optionally with one monochrome alpha image (single or grid) linked by `auxl` and the
//! MPEG alpha `auxC`. Every AV1 item of the profile is fully decoded, one picture at a time.
//! Anything else (overlays, thumbnails, depth, nested or shared tiles, premultiplied links,
//! transforms) is refused rather than admitted unverified.
use image::metadata::Orientation;

use super::{
    super::domain::image::{check_image_bounds, MAX_IMAGE_DIMENSION, MAX_IMAGE_PIXELS},
    ImageIntakeError,
};

/// Tiles of one grid; each is decoded alone and dropped before the next.
const MAX_GRID_TILES: usize = 64;
/// Primary, alpha and the tiles of two grids, plus metadata items.
const MAX_ITEMS: usize = 2 * MAX_GRID_TILES + 8;
const MAX_REFERENCES: usize = 8;
const ALPHA_AUX_TYPE: &[u8] = b"urn:mpeg:mpegB:cicp:systems:auxiliary:alpha";

#[derive(Clone, Copy, PartialEq, Eq)]
struct Av1Config {
    seq_profile: u8,
    high_bitdepth: bool,
    twelve_bit: bool,
    monochrome: bool,
}

impl Av1Config {
    fn bit_depth(self) -> u8 {
        if self.seq_profile == 2 && self.high_bitdepth {
            if self.twelve_bit {
                12
            } else {
                10
            }
        } else if self.high_bitdepth {
            10
        } else {
            8
        }
    }

    fn bytes_per_pixel(self) -> u8 {
        if self.bit_depth() > 8 {
            8
        } else {
            4
        }
    }
}

#[derive(Clone, Copy)]
enum Property {
    Ispe { width: u32, height: u32 },
    Av1C(Av1Config),
    Pixi { depth: u8 },
    AuxC { alpha: bool },
    Irot { angle: u8 },
    NonIdentityTransform,
    Other,
}

#[derive(Clone, Copy)]
struct ItemExtent {
    offset: usize,
    length: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ItemKind {
    Av01,
    Grid,
    Exif,
}

struct Item {
    id: u32,
    kind: ItemKind,
}

struct Reference {
    kind: [u8; 4],
    from: u32,
    to: Vec<u32>,
}

pub(super) fn inspect_avif(bytes: &[u8]) -> Result<(u32, u32), ImageIntakeError> {
    let mut offset = 0_usize;
    let mut saw_ftyp = false;
    let mut meta_payload: Option<&[u8]> = None;
    let mut mdat_range: Option<(usize, usize)> = None;

    while offset < bytes.len() {
        let (box_type, payload, next_offset, payload_offset) = next_box(bytes, offset)?;
        if offset == 0 && &box_type != b"ftyp" {
            return Err(ImageIntakeError::MalformedImage);
        }
        match &box_type {
            b"ftyp" => {
                if saw_ftyp {
                    return Err(ImageIntakeError::MalformedImage);
                }
                check_ftyp(payload)?;
                saw_ftyp = true;
            }
            b"meta" => {
                if meta_payload.is_some() {
                    return Err(ImageIntakeError::MalformedImage);
                }
                meta_payload = Some(payload);
            }
            b"mdat" => {
                if mdat_range.is_some() || payload.is_empty() {
                    return Err(ImageIntakeError::MalformedImage);
                }
                mdat_range = Some((payload_offset, next_offset));
            }
            b"moov" | b"moof" | b"trak" => return Err(ImageIntakeError::AnimatedImage),
            _ => return Err(ImageIntakeError::MalformedImage),
        }
        offset = next_offset;
    }

    let meta = meta_payload.ok_or(ImageIntakeError::MalformedImage)?;
    let (mdat_start, mdat_end) = mdat_range.ok_or(ImageIntakeError::MalformedImage)?;
    parse_meta_and_bitstream(bytes, meta, mdat_start, mdat_end)
}

/// One parsed box: its type, payload, end offset and payload offset.
type BoxView<'a> = ([u8; 4], &'a [u8], usize, usize);

/// Stores a unique box payload; a repeated box is malformed.
fn set_once<'a>(slot: &mut Option<&'a [u8]>, payload: &'a [u8]) -> Result<(), ImageIntakeError> {
    match slot.replace(payload) {
        Some(_) => Err(ImageIntakeError::MalformedImage),
        None => Ok(()),
    }
}

fn next_box(bytes: &[u8], offset: usize) -> Result<BoxView<'_>, ImageIntakeError> {
    let header = bytes
        .get(
            offset
                ..offset
                    .checked_add(8)
                    .ok_or(ImageIntakeError::MalformedImage)?,
        )
        .ok_or(ImageIntakeError::MalformedImage)?;
    let size32 = u32::from_be_bytes(header[..4].try_into().unwrap());
    let box_type: [u8; 4] = header[4..8].try_into().unwrap();
    let (header_len, box_size) = match size32 {
        0 => return Err(ImageIntakeError::MalformedImage),
        1 => {
            let ext = bytes
                .get(
                    offset + 8
                        ..offset
                            .checked_add(16)
                            .ok_or(ImageIntakeError::MalformedImage)?,
                )
                .ok_or(ImageIntakeError::MalformedImage)?;
            let size64 = u64::from_be_bytes(ext.try_into().unwrap());
            let size = usize::try_from(size64).map_err(|_| ImageIntakeError::MalformedImage)?;
            if size < 16 {
                return Err(ImageIntakeError::MalformedImage);
            }
            (16_usize, size)
        }
        size => {
            let size = size as usize;
            if size < 8 {
                return Err(ImageIntakeError::MalformedImage);
            }
            (8_usize, size)
        }
    };
    let end = offset
        .checked_add(box_size)
        .filter(|end| *end <= bytes.len())
        .ok_or(ImageIntakeError::MalformedImage)?;
    let payload_offset = offset + header_len;
    Ok((box_type, &bytes[payload_offset..end], end, payload_offset))
}

fn check_ftyp(payload: &[u8]) -> Result<(), ImageIntakeError> {
    if payload.len() < 8 || !payload.len().is_multiple_of(4) {
        return Err(ImageIntakeError::MalformedImage);
    }
    let major = &payload[..4];
    if major == b"avis" {
        return Err(ImageIntakeError::AnimatedImage);
    }
    let mut has_avif = major == b"avif";
    for brand in payload[8..].as_chunks::<4>().0 {
        if brand == b"avis" {
            return Err(ImageIntakeError::AnimatedImage);
        }
        if brand == b"avif" {
            has_avif = true;
        }
    }
    if !has_avif {
        return Err(ImageIntakeError::UnsupportedFormat);
    }
    Ok(())
}

fn parse_meta_and_bitstream(
    file_bytes: &[u8],
    meta: &[u8],
    mdat_start: usize,
    mdat_end: usize,
) -> Result<(u32, u32), ImageIntakeError> {
    if meta.len() < 4 || meta[0] != 0 || meta[1..4] != [0, 0, 0] {
        return Err(ImageIntakeError::MalformedImage);
    }
    let body = &meta[4..];
    let mut offset = 0_usize;
    let mut saw_hdlr = false;
    let mut primary_item_id: Option<u32> = None;
    let mut iloc_payload: Option<&[u8]> = None;
    let mut iinf_payload: Option<&[u8]> = None;
    let mut iref_payload: Option<&[u8]> = None;
    let mut iprp_payload: Option<&[u8]> = None;

    while offset < body.len() {
        let (box_type, payload, next_offset, _) = next_box(body, offset)?;
        if offset == 0 && &box_type != b"hdlr" {
            return Err(ImageIntakeError::MalformedImage);
        }
        match &box_type {
            b"hdlr" => {
                if saw_hdlr || payload.len() < 12 || payload[0] != 0 || &payload[8..12] != b"pict" {
                    return Err(ImageIntakeError::MalformedImage);
                }
                saw_hdlr = true;
            }
            b"pitm" => {
                if primary_item_id.is_some() || payload.len() < 6 {
                    return Err(ImageIntakeError::MalformedImage);
                }
                let version = payload[0];
                let id = match version {
                    0 => u32::from(u16::from_be_bytes(payload[4..6].try_into().unwrap())),
                    1 if payload.len() >= 8 => {
                        u32::from_be_bytes(payload[4..8].try_into().unwrap())
                    }
                    _ => return Err(ImageIntakeError::MalformedImage),
                };
                primary_item_id = Some(id);
            }
            b"iloc" => set_once(&mut iloc_payload, payload)?,
            b"iinf" => set_once(&mut iinf_payload, payload)?,
            b"iref" => set_once(&mut iref_payload, payload)?,
            b"iprp" => set_once(&mut iprp_payload, payload)?,
            _ => {}
        }
        offset = next_offset;
    }

    let pitm = primary_item_id.ok_or(ImageIntakeError::MalformedImage)?;
    let items = parse_iinf(iinf_payload.ok_or(ImageIntakeError::MalformedImage)?)?;
    let locations = parse_iloc(iloc_payload.ok_or(ImageIntakeError::MalformedImage)?)?;
    let references = iref_payload
        .map(parse_iref)
        .transpose()?
        .unwrap_or_default();
    let properties = parse_iprp(iprp_payload.ok_or(ImageIntakeError::MalformedImage)?)?;

    // Every declared item owns exactly one extent, and the extents tile `mdat` with no gaps.
    if locations.len() != items.len() {
        return Err(ImageIntakeError::UnsupportedFormat);
    }
    for (index, (id, _)) in locations.iter().enumerate() {
        if locations[..index].iter().any(|(other, _)| other == id)
            || !items.iter().any(|item| item.id == *id)
        {
            return Err(ImageIntakeError::MalformedImage);
        }
    }
    let mut extents: Vec<_> = locations.iter().map(|(_, extent)| *extent).collect();
    extents.sort_by_key(|extent| extent.offset);
    let mut expected_offset = mdat_start;
    for extent in extents {
        if extent.offset != expected_offset {
            return Err(ImageIntakeError::MalformedImage);
        }
        expected_offset = expected_offset
            .checked_add(extent.length)
            .ok_or(ImageIntakeError::MalformedImage)?;
    }
    if expected_offset != mdat_end {
        return Err(ImageIntakeError::MalformedImage);
    }
    let container = Container {
        file: file_bytes,
        locations: &locations,
        mdat: (mdat_start, mdat_end),
    };

    for item in items.iter().filter(|item| item.kind == ItemKind::Exif) {
        check_avif_exif(container.item_bytes(item.id)?)?;
    }
    let mut grids = Vec::new();
    for item in items.iter().filter(|item| item.kind == ItemKind::Grid) {
        grids.push((
            item.id,
            parse_grid_descriptor(container.item_bytes(item.id)?)?,
        ));
    }
    let plans = resolve_plans(&items, &references, &properties, &grids, pitm)?;

    let color = verify_plan(&plans.color, &properties, &container)?;
    let (width, height) = plans.color.dimensions(&properties)?;
    let mut bytes_per_pixel = color.config.bytes_per_pixel();
    if let Some(alpha_plan) = &plans.alpha {
        let alpha = verify_plan(alpha_plan, &properties, &container)?;
        // The alpha plane is one monochrome picture covering exactly the color image.
        if alpha_plan.dimensions(&properties)? != (width, height)
            || alpha.layout != dav1d::PixelLayout::I400
        {
            return Err(ImageIntakeError::MalformedImage);
        }
        bytes_per_pixel = bytes_per_pixel.max(alpha.config.bytes_per_pixel());
    }
    check_image_bounds(file_bytes.len() as u64, width, height, bytes_per_pixel)
        .map_err(|_| ImageIntakeError::ImageLimit)?;
    Ok((width, height))
}

/// A decoded image item: the grid of tiles or the single `av01` item of one picture.
enum ImagePlan {
    Single {
        item: u32,
    },
    Grid {
        item: u32,
        descriptor: GridDescriptor,
        tiles: Vec<u32>,
    },
}

impl ImagePlan {
    fn dimensions(&self, properties: &Properties) -> Result<(u32, u32), ImageIntakeError> {
        match self {
            Self::Single { item } => properties.ispe(*item),
            Self::Grid { descriptor, .. } => Ok((descriptor.width, descriptor.height)),
        }
    }
}

struct Plans {
    color: ImagePlan,
    alpha: Option<ImagePlan>,
}

/// The only auxiliary profile: one alpha image (single item or grid) of the primary image.
/// Any other item, reference or auxiliary type is refused; it would need its own decoder.
fn resolve_plans(
    items: &[Item],
    references: &[Reference],
    properties: &Properties,
    grids: &[(u32, GridDescriptor)],
    primary: u32,
) -> Result<Plans, ImageIntakeError> {
    let kind_of = |id: u32| {
        items
            .iter()
            .find(|item| item.id == id)
            .map(|item| item.kind)
    };
    let mut alpha_root: Option<u32> = None;
    for reference in references {
        match &reference.kind {
            b"auxl" => {
                if reference.to != [primary] || reference.from == primary {
                    return Err(ImageIntakeError::UnsupportedFormat);
                }
                if alpha_root.replace(reference.from).is_some() {
                    return Err(ImageIntakeError::UnsupportedFormat);
                }
            }
            b"dimg" => {}
            _ => return Err(ImageIntakeError::UnsupportedFormat),
        }
        if kind_of(reference.from).is_none() || reference.to.iter().any(|id| kind_of(*id).is_none())
        {
            return Err(ImageIntakeError::MalformedImage);
        }
    }

    let mut claimed_tiles: Vec<u32> = Vec::new();
    let mut plan_for = |root: u32| -> Result<ImagePlan, ImageIntakeError> {
        let dimg: Vec<&Reference> = references
            .iter()
            .filter(|reference| &reference.kind == b"dimg" && reference.from == root)
            .collect();
        match kind_of(root) {
            Some(ItemKind::Av01) if dimg.is_empty() => Ok(ImagePlan::Single { item: root }),
            Some(ItemKind::Grid) => {
                let [reference] = dimg.as_slice() else {
                    return Err(ImageIntakeError::MalformedImage);
                };
                let descriptor = grids
                    .iter()
                    .find(|(id, _)| *id == root)
                    .map(|(_, descriptor)| *descriptor)
                    .ok_or(ImageIntakeError::MalformedImage)?;
                if reference.to.len() != descriptor.rows * descriptor.columns {
                    return Err(ImageIntakeError::MalformedImage);
                }
                for tile in &reference.to {
                    // Nested grids and shared or repeated tiles are not part of the profile.
                    if kind_of(*tile) != Some(ItemKind::Av01) || claimed_tiles.contains(tile) {
                        return Err(ImageIntakeError::UnsupportedFormat);
                    }
                    claimed_tiles.push(*tile);
                }
                Ok(ImagePlan::Grid {
                    item: root,
                    descriptor,
                    tiles: reference.to.clone(),
                })
            }
            Some(ItemKind::Av01) | Some(ItemKind::Exif) | None => {
                Err(ImageIntakeError::MalformedImage)
            }
        }
    };
    let color = plan_for(primary)?;
    let alpha = alpha_root.map(&mut plan_for).transpose()?;

    // Everything declared must be accounted for: no unchecked extra pictures or orphan grids.
    for item in items {
        let accounted = match item.kind {
            ItemKind::Exif => true,
            _ => {
                item.id == primary
                    || Some(item.id) == alpha_root
                    || claimed_tiles.contains(&item.id)
            }
        };
        let aux = properties.auxiliary(item.id)?;
        let is_alpha_root = Some(item.id) == alpha_root;
        if !accounted || (aux == Auxiliary::Alpha) != is_alpha_root || aux == Auxiliary::Other {
            return Err(ImageIntakeError::UnsupportedFormat);
        }
    }
    Ok(Plans { color, alpha })
}

/// Facts about one fully decoded plane set.
struct PlaneFacts {
    config: Av1Config,
    layout: dav1d::PixelLayout,
}

fn verify_plan(
    plan: &ImagePlan,
    properties: &Properties,
    container: &Container<'_>,
) -> Result<PlaneFacts, ImageIntakeError> {
    match plan {
        ImagePlan::Single { item } => verify_av1_item(*item, properties, container),
        ImagePlan::Grid {
            item,
            descriptor,
            tiles,
        } => {
            let (ispe_width, ispe_height) = properties.ispe(*item)?;
            if (ispe_width, ispe_height) != (descriptor.width, descriptor.height) {
                return Err(ImageIntakeError::MalformedImage);
            }
            let first = properties.av1_spec(tiles[0])?;
            // The tile grid covers the output with no whole row or column of surplus tiles.
            let covered_width = first.width as u64 * descriptor.columns as u64;
            let covered_height = first.height as u64 * descriptor.rows as u64;
            if covered_width < u64::from(descriptor.width)
                || covered_height < u64::from(descriptor.height)
                || covered_width - first.width as u64 >= u64::from(descriptor.width)
                || covered_height - first.height as u64 >= u64::from(descriptor.height)
                || covered_width * covered_height > MAX_IMAGE_PIXELS
            {
                return Err(ImageIntakeError::MalformedImage);
            }
            // One decoder and one picture at a time: tiles never coexist in memory.
            let mut layout = None;
            for tile in tiles {
                if properties.av1_spec(*tile)? != first {
                    return Err(ImageIntakeError::MalformedImage);
                }
                let facts = verify_av1_item(*tile, properties, container)?;
                if *layout.get_or_insert(facts.layout) != facts.layout {
                    return Err(ImageIntakeError::MalformedImage);
                }
            }
            Ok(PlaneFacts {
                config: first.config,
                layout: layout.ok_or(ImageIntakeError::MalformedImage)?,
            })
        }
    }
}

fn verify_av1_item(
    item: u32,
    properties: &Properties,
    container: &Container<'_>,
) -> Result<PlaneFacts, ImageIntakeError> {
    let spec = properties.av1_spec(item)?;
    check_image_bounds(
        container.file.len() as u64,
        spec.width,
        spec.height,
        spec.config.bytes_per_pixel(),
    )
    .map_err(|_| ImageIntakeError::ImageLimit)?;
    let bytes = container.item_bytes(item)?;
    inspect_av1_obus(bytes, spec.width, spec.height, spec.config)?;
    let layout = decode_av1_pixels(bytes, spec.width, spec.height, spec.config)?;
    Ok(PlaneFacts {
        config: spec.config,
        layout,
    })
}

// Structural parsing alone cannot establish that entropy-coded tiles contain pixels.
// Set native limits before sending any data; one still frame, one thread, no queued frames.
fn decode_av1_pixels(
    bytes: &[u8],
    width: u32,
    height: u32,
    config: Av1Config,
) -> Result<dav1d::PixelLayout, ImageIntakeError> {
    let mut settings = dav1d::Settings::new();
    settings.set_n_threads(1);
    settings.set_max_frame_delay(1);
    settings.set_frame_size_limit(16_777_216);
    settings.set_strict_std_compliance(true);
    let mut decoder = dav1d::Decoder::with_settings(&settings)
        .map_err(|_| ImageIntakeError::InspectionUnavailable)?;
    decoder
        .send_data(bytes.to_vec(), None, None, None)
        .map_err(|_| ImageIntakeError::MalformedImage)?;
    let picture = decoder
        .get_picture()
        .map_err(|_| ImageIntakeError::MalformedImage)?;
    let layout = picture.pixel_layout();
    if picture.width() != width
        || picture.height() != height
        || picture.bit_depth() != usize::from(config.bit_depth())
        || (layout == dav1d::PixelLayout::I400) != config.monochrome
    {
        return Err(ImageIntakeError::MalformedImage);
    }
    match decoder.get_picture() {
        Err(dav1d::Error::Again) => Ok(layout),
        Ok(_) => Err(ImageIntakeError::AnimatedImage),
        Err(_) => Err(ImageIntakeError::MalformedImage),
    }
}

struct Container<'a> {
    file: &'a [u8],
    locations: &'a [(u32, ItemExtent)],
    mdat: (usize, usize),
}

impl Container<'_> {
    fn item_bytes(&self, id: u32) -> Result<&[u8], ImageIntakeError> {
        let extent = self
            .locations
            .iter()
            .find(|(item, _)| *item == id)
            .map(|(_, extent)| *extent)
            .ok_or(ImageIntakeError::MalformedImage)?;
        slice_in_mdat(self.file, extent, self.mdat.0, self.mdat.1)
    }
}

#[derive(Clone, Copy)]
struct GridDescriptor {
    rows: usize,
    columns: usize,
    width: u32,
    height: u32,
}

/// ISO 14496-12 `ImageGrid`: version 0, a 16- or 32-bit field-length flag, then the grid.
fn parse_grid_descriptor(bytes: &[u8]) -> Result<GridDescriptor, ImageIntakeError> {
    let [version, flags, rows_minus_one, columns_minus_one, fields @ ..] = bytes else {
        return Err(ImageIntakeError::MalformedImage);
    };
    if *version != 0 || flags & !1 != 0 {
        return Err(ImageIntakeError::MalformedImage);
    }
    let field_size = if flags & 1 == 1 { 4 } else { 2 };
    if fields.len() != 2 * field_size {
        return Err(ImageIntakeError::MalformedImage);
    }
    let mut cursor = 0_usize;
    let width = read_uint(fields, &mut cursor, field_size)?;
    let height = read_uint(fields, &mut cursor, field_size)?;
    let rows = usize::from(*rows_minus_one) + 1;
    let columns = usize::from(*columns_minus_one) + 1;
    if width == 0 || height == 0 {
        return Err(ImageIntakeError::MalformedImage);
    }
    if rows * columns > MAX_GRID_TILES
        || width > MAX_IMAGE_DIMENSION as usize
        || height > MAX_IMAGE_DIMENSION as usize
    {
        return Err(ImageIntakeError::ImageLimit);
    }
    Ok(GridDescriptor {
        rows,
        columns,
        width: width as u32,
        height: height as u32,
    })
}

fn slice_in_mdat(
    file_bytes: &[u8],
    extent: ItemExtent,
    mdat_start: usize,
    mdat_end: usize,
) -> Result<&[u8], ImageIntakeError> {
    let end = extent
        .offset
        .checked_add(extent.length)
        .ok_or(ImageIntakeError::MalformedImage)?;
    if extent.length == 0 || extent.offset < mdat_start || end > mdat_end {
        return Err(ImageIntakeError::MalformedImage);
    }
    file_bytes
        .get(extent.offset..end)
        .ok_or(ImageIntakeError::MalformedImage)
}

fn check_avif_exif(exif_item: &[u8]) -> Result<(), ImageIntakeError> {
    if exif_item.len() < 4 {
        return Err(ImageIntakeError::MalformedImage);
    }
    let tiff_offset = u32::from_be_bytes(exif_item[..4].try_into().unwrap()) as usize;
    let start = 4_usize
        .checked_add(tiff_offset)
        .filter(|idx| *idx <= exif_item.len())
        .ok_or(ImageIntakeError::MalformedImage)?;
    let tail = &exif_item[start..];
    let tiff = tail.strip_prefix(b"Exif\0\0").unwrap_or(tail);
    match Orientation::from_exif_chunk(tiff) {
        Some(Orientation::NoTransforms) | None => Ok(()),
        Some(_) => Err(ImageIntakeError::NonIdentityOrientation),
    }
}

fn parse_iinf(payload: &[u8]) -> Result<Vec<Item>, ImageIntakeError> {
    if payload.len() < 6 {
        return Err(ImageIntakeError::MalformedImage);
    }
    let version = payload[0];
    let (count, mut offset) = match version {
        0 => (
            usize::from(u16::from_be_bytes(payload[4..6].try_into().unwrap())),
            6_usize,
        ),
        1 if payload.len() >= 8 => (
            u32::from_be_bytes(payload[4..8].try_into().unwrap()) as usize,
            8_usize,
        ),
        _ => return Err(ImageIntakeError::MalformedImage),
    };
    // Primary, alpha and every tile of both grids, plus metadata: more is not this profile.
    if count > MAX_ITEMS {
        return Err(ImageIntakeError::UnsupportedFormat);
    }
    let mut items: Vec<Item> = Vec::with_capacity(count);
    for _ in 0..count {
        let (box_type, infe, next_offset, _) = next_box(payload, offset)?;
        if &box_type != b"infe" || infe.len() < 12 {
            return Err(ImageIntakeError::MalformedImage);
        }
        let infe_version = infe[0];
        let (id, item_type) = match infe_version {
            2 => (
                u32::from(u16::from_be_bytes(infe[4..6].try_into().unwrap())),
                &infe[8..12],
            ),
            3 if infe.len() >= 14 => (
                u32::from_be_bytes(infe[4..8].try_into().unwrap()),
                &infe[10..14],
            ),
            _ => return Err(ImageIntakeError::MalformedImage),
        };
        let kind = match item_type {
            b"av01" => ItemKind::Av01,
            b"grid" => ItemKind::Grid,
            b"Exif" => ItemKind::Exif,
            // Derived overlays, thumbnails, depth, tone maps and other media need their own
            // complete decoding profile. Never admit unchecked pixels as a verified image.
            _ => return Err(ImageIntakeError::UnsupportedFormat),
        };
        if items.iter().any(|item| item.id == id) {
            return Err(ImageIntakeError::MalformedImage);
        }
        items.push(Item { id, kind });
        offset = next_offset;
    }
    if offset != payload.len() {
        return Err(ImageIntakeError::MalformedImage);
    }
    Ok(items)
}

/// `iref`: typed, directed references between items; types are screened by the caller.
fn parse_iref(payload: &[u8]) -> Result<Vec<Reference>, ImageIntakeError> {
    if payload.len() < 4 || payload[0] > 1 || payload[1..4] != [0, 0, 0] {
        return Err(ImageIntakeError::MalformedImage);
    }
    let id_size = if payload[0] == 0 { 2 } else { 4 };
    let mut references = Vec::new();
    let mut offset = 4_usize;
    while offset < payload.len() {
        let (kind, body, next_offset, _) = next_box(payload, offset)?;
        if references.len() == MAX_REFERENCES {
            return Err(ImageIntakeError::UnsupportedFormat);
        }
        let mut cursor = 0_usize;
        let from = read_uint(body, &mut cursor, id_size)? as u32;
        let count = read_uint(body, &mut cursor, 2)?;
        if count == 0 || count > MAX_GRID_TILES {
            return Err(ImageIntakeError::MalformedImage);
        }
        let mut to = Vec::with_capacity(count);
        for _ in 0..count {
            to.push(read_uint(body, &mut cursor, id_size)? as u32);
        }
        if cursor != body.len() {
            return Err(ImageIntakeError::MalformedImage);
        }
        references.push(Reference { kind, from, to });
        offset = next_offset;
    }
    Ok(references)
}

fn parse_iloc(payload: &[u8]) -> Result<Vec<(u32, ItemExtent)>, ImageIntakeError> {
    if payload.len() < 8 {
        return Err(ImageIntakeError::MalformedImage);
    }
    let version = payload[0];
    let offset_size = usize::from(payload[4] >> 4);
    let length_size = usize::from(payload[4] & 0x0f);
    let base_offset_size = usize::from(payload[5] >> 4);
    let index_size = if version == 1 || version == 2 {
        usize::from(payload[5] & 0x0f)
    } else {
        0
    };
    let (item_count, mut cursor) = match version {
        0 | 1 => (
            usize::from(u16::from_be_bytes(payload[6..8].try_into().unwrap())),
            8_usize,
        ),
        2 if payload.len() >= 10 => (
            u32::from_be_bytes(payload[6..10].try_into().unwrap()) as usize,
            10_usize,
        ),
        _ => return Err(ImageIntakeError::MalformedImage),
    };
    let mut items = Vec::with_capacity(item_count.min(16));
    for _ in 0..item_count {
        let item_id = if version < 2 {
            read_uint(payload, &mut cursor, 2)? as u32
        } else {
            read_uint(payload, &mut cursor, 4)? as u32
        };
        if version == 1 || version == 2 {
            let construction_method = read_uint(payload, &mut cursor, 2)? & 0x0f;
            if construction_method != 0 {
                return Err(ImageIntakeError::MalformedImage);
            }
        }
        let _data_reference_index = read_uint(payload, &mut cursor, 2)?;
        let base_offset = read_uint(payload, &mut cursor, base_offset_size)?;
        let extent_count = read_uint(payload, &mut cursor, 2)?;
        if extent_count != 1 {
            return Err(ImageIntakeError::MalformedImage);
        }
        if index_size > 0 {
            let _extent_index = read_uint(payload, &mut cursor, index_size)?;
        }
        let extent_offset = read_uint(payload, &mut cursor, offset_size)?;
        let extent_length = read_uint(payload, &mut cursor, length_size)?;
        let offset = base_offset
            .checked_add(extent_offset)
            .ok_or(ImageIntakeError::MalformedImage)?;
        items.push((
            item_id,
            ItemExtent {
                offset,
                length: extent_length,
            },
        ));
    }
    if cursor != payload.len() {
        return Err(ImageIntakeError::MalformedImage);
    }
    Ok(items)
}

fn read_uint(bytes: &[u8], cursor: &mut usize, size: usize) -> Result<usize, ImageIntakeError> {
    let end = cursor
        .checked_add(size)
        .ok_or(ImageIntakeError::MalformedImage)?;
    let slice = bytes
        .get(*cursor..end)
        .ok_or(ImageIntakeError::MalformedImage)?;
    *cursor = end;
    let value = match size {
        0 => 0_u64,
        1 => u64::from(slice[0]),
        2 => u64::from(u16::from_be_bytes(slice.try_into().unwrap())),
        4 => u64::from(u32::from_be_bytes(slice.try_into().unwrap())),
        8 => u64::from_be_bytes(slice.try_into().unwrap()),
        _ => return Err(ImageIntakeError::MalformedImage),
    };
    usize::try_from(value).map_err(|_| ImageIntakeError::MalformedImage)
}

fn parse_iprp(payload: &[u8]) -> Result<Properties, ImageIntakeError> {
    let (first_type, ipco, ipma_offset, _) = next_box(payload, 0)?;
    if &first_type != b"ipco" {
        return Err(ImageIntakeError::MalformedImage);
    }
    let (second_type, ipma, end_offset, _) = next_box(payload, ipma_offset)?;
    if &second_type != b"ipma" || end_offset != payload.len() {
        return Err(ImageIntakeError::MalformedImage);
    }

    let mut properties = Vec::new();
    let mut offset = 0_usize;
    while offset < ipco.len() {
        let (box_type, prop, next_offset, _) = next_box(ipco, offset)?;
        let parsed = match &box_type {
            b"ispe" => {
                if prop.len() != 12 || prop[0] != 0 {
                    return Err(ImageIntakeError::MalformedImage);
                }
                let width = u32::from_be_bytes(prop[4..8].try_into().unwrap());
                let height = u32::from_be_bytes(prop[8..12].try_into().unwrap());
                if width == 0 || height == 0 {
                    return Err(ImageIntakeError::MalformedImage);
                }
                if width > MAX_IMAGE_DIMENSION || height > MAX_IMAGE_DIMENSION {
                    return Err(ImageIntakeError::ImageLimit);
                }
                Property::Ispe { width, height }
            }
            b"av1C" => {
                if prop.len() < 4 || (prop[0] >> 7) != 1 || (prop[0] & 0x7f) != 1 {
                    return Err(ImageIntakeError::MalformedImage);
                }
                let seq_profile = prop[1] >> 5;
                let high_bitdepth = (prop[2] & 0x40) != 0;
                let twelve_bit = (prop[2] & 0x20) != 0;
                let monochrome = (prop[2] & 0x10) != 0;
                if seq_profile > 2 || (twelve_bit && (!high_bitdepth || seq_profile != 2)) {
                    return Err(ImageIntakeError::MalformedImage);
                }
                Property::Av1C(Av1Config {
                    seq_profile,
                    high_bitdepth,
                    twelve_bit,
                    monochrome,
                })
            }
            b"pixi" => {
                if prop.len() < 6 || prop[0] != 0 || prop[4] == 0 {
                    return Err(ImageIntakeError::MalformedImage);
                }
                let channels = usize::from(prop[4]);
                if prop.len() != 5 + channels {
                    return Err(ImageIntakeError::MalformedImage);
                }
                let depth = prop[5];
                if !prop[5..].iter().all(|&d| d == depth) {
                    return Err(ImageIntakeError::MalformedImage);
                }
                Property::Pixi { depth }
            }
            b"irot" => {
                if prop.len() != 1 {
                    return Err(ImageIntakeError::MalformedImage);
                }
                Property::Irot {
                    angle: prop[0] & 0x03,
                }
            }
            b"imir" | b"clap" => Property::NonIdentityTransform,
            // Full box, then a NUL-terminated URN; the alpha profile has no subtype bytes.
            b"auxC" => Property::AuxC {
                alpha: prop.len() == 4 + ALPHA_AUX_TYPE.len() + 1
                    && prop[0] == 0
                    && prop[4..].starts_with(ALPHA_AUX_TYPE)
                    && prop[prop.len() - 1] == 0,
            },
            _ => Property::Other,
        };
        if matches!(parsed, Property::Irot { angle } if angle != 0)
            || matches!(parsed, Property::NonIdentityTransform)
        {
            return Err(ImageIntakeError::NonIdentityOrientation);
        }
        properties.push(parsed);
        offset = next_offset;
    }

    let associations = parse_ipma(ipma, properties.len())?;
    Ok(Properties {
        list: properties,
        associations,
    })
}

/// The `ipco` property list with each item's 1-based associations resolved.
struct Properties {
    list: Vec<Property>,
    associations: Vec<(u32, Vec<usize>)>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Auxiliary {
    None,
    Alpha,
    Other,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Av1Spec {
    width: u32,
    height: u32,
    config: Av1Config,
}

impl Properties {
    fn of(&self, item: u32) -> Result<impl Iterator<Item = Property> + '_, ImageIntakeError> {
        let (_, indices) = self
            .associations
            .iter()
            .find(|(id, _)| *id == item)
            .ok_or(ImageIntakeError::MalformedImage)?;
        Ok(indices.iter().map(|index| self.list[index - 1]))
    }

    fn ispe(&self, item: u32) -> Result<(u32, u32), ImageIntakeError> {
        let mut found = None;
        for property in self.of(item)? {
            if let Property::Ispe { width, height } = property {
                if found.replace((width, height)).is_some() {
                    return Err(ImageIntakeError::MalformedImage);
                }
            }
        }
        found.ok_or(ImageIntakeError::MalformedImage)
    }

    fn auxiliary(&self, item: u32) -> Result<Auxiliary, ImageIntakeError> {
        let mut found = Auxiliary::None;
        for property in self.of(item)? {
            if let Property::AuxC { alpha } = property {
                let kind = if alpha {
                    Auxiliary::Alpha
                } else {
                    Auxiliary::Other
                };
                if found != Auxiliary::None {
                    return Err(ImageIntakeError::MalformedImage);
                }
                found = kind;
            }
        }
        Ok(found)
    }

    /// Dimensions and codec configuration of an `av01` item, with `pixi` agreeing.
    fn av1_spec(&self, item: u32) -> Result<Av1Spec, ImageIntakeError> {
        let (width, height) = self.ispe(item)?;
        let mut config: Option<Av1Config> = None;
        let mut depth: Option<u8> = None;
        for property in self.of(item)? {
            match property {
                Property::Av1C(found) => {
                    if config.replace(found).is_some() {
                        return Err(ImageIntakeError::MalformedImage);
                    }
                }
                Property::Pixi { depth: found } => {
                    if depth.replace(found).is_some() {
                        return Err(ImageIntakeError::MalformedImage);
                    }
                }
                // Non-identity transforms were already refused while reading `ipco`.
                Property::Ispe { .. }
                | Property::AuxC { .. }
                | Property::Irot { .. }
                | Property::NonIdentityTransform
                | Property::Other => {}
            }
        }
        let config = config.ok_or(ImageIntakeError::MalformedImage)?;
        if depth.is_some_and(|depth| depth != config.bit_depth()) {
            return Err(ImageIntakeError::MalformedImage);
        }
        Ok(Av1Spec {
            width,
            height,
            config,
        })
    }
}

fn parse_ipma(
    payload: &[u8],
    property_count: usize,
) -> Result<Vec<(u32, Vec<usize>)>, ImageIntakeError> {
    if payload.len() < 8 {
        return Err(ImageIntakeError::MalformedImage);
    }
    let version = payload[0];
    let flags = u32::from_be_bytes([0, payload[1], payload[2], payload[3]]);
    let entry_count = u32::from_be_bytes(payload[4..8].try_into().unwrap()) as usize;
    let mut cursor = 8_usize;
    let mut entries: Vec<(u32, Vec<usize>)> = Vec::new();
    for _ in 0..entry_count {
        let item_id = if version < 1 {
            read_uint(payload, &mut cursor, 2)? as u32
        } else {
            read_uint(payload, &mut cursor, 4)? as u32
        };
        let assoc_count = read_uint(payload, &mut cursor, 1)?;
        let mut indices = Vec::with_capacity(assoc_count);
        for _ in 0..assoc_count {
            let raw = if (flags & 1) != 0 {
                read_uint(payload, &mut cursor, 2)? & 0x7fff
            } else {
                read_uint(payload, &mut cursor, 1)? & 0x7f
            };
            if raw == 0 || raw > property_count {
                return Err(ImageIntakeError::MalformedImage);
            }
            indices.push(raw);
        }
        if entries.iter().any(|(id, _)| *id == item_id) {
            return Err(ImageIntakeError::MalformedImage);
        }
        entries.push((item_id, indices));
    }
    if cursor != payload.len() {
        return Err(ImageIntakeError::MalformedImage);
    }
    Ok(entries)
}

fn inspect_av1_obus(
    data: &[u8],
    expected_width: u32,
    expected_height: u32,
    av1c: Av1Config,
) -> Result<(), ImageIntakeError> {
    let mut cursor = 0_usize;
    let mut reduced_still_header: Option<bool> = None;
    let mut frame_headers = 0_usize;
    let mut saw_tile_data = false;

    while cursor < data.len() {
        let header = *data.get(cursor).ok_or(ImageIntakeError::MalformedImage)?;
        cursor += 1;
        let forbidden = (header >> 7) & 1;
        let obu_type = (header >> 3) & 0x0f;
        let has_extension = ((header >> 2) & 1) != 0;
        let has_size = ((header >> 1) & 1) != 0;
        let reserved = header & 1;
        if forbidden != 0 || !has_size || reserved != 0 {
            return Err(ImageIntakeError::MalformedImage);
        }
        if has_extension {
            let _ext = *data.get(cursor).ok_or(ImageIntakeError::MalformedImage)?;
            cursor += 1;
        }
        let obu_size = read_leb128(data, &mut cursor)?;
        let end = cursor
            .checked_add(obu_size)
            .filter(|end| *end <= data.len())
            .ok_or(ImageIntakeError::MalformedImage)?;
        let payload = &data[cursor..end];
        cursor = end;

        match obu_type {
            1 => {
                let reduced =
                    parse_sequence_header(payload, expected_width, expected_height, av1c)?;
                reduced_still_header = Some(reduced);
            }
            3 | 6 => {
                let reduced = reduced_still_header.ok_or(ImageIntakeError::MalformedImage)?;
                frame_headers += 1;
                if frame_headers > 1 {
                    return Err(ImageIntakeError::AnimatedImage);
                }
                check_frame_header(payload, reduced)?;
                if obu_type == 6 {
                    if payload.len() < 4 || *payload.last().unwrap() == 0 {
                        return Err(ImageIntakeError::MalformedImage);
                    }
                    saw_tile_data = true;
                }
            }
            4 => {
                if frame_headers == 0 || payload.is_empty() || *payload.last().unwrap() == 0 {
                    return Err(ImageIntakeError::MalformedImage);
                }
                saw_tile_data = true;
            }
            2 | 5 => {}
            _ => return Err(ImageIntakeError::MalformedImage),
        }
    }

    if reduced_still_header.is_none() || frame_headers != 1 || !saw_tile_data {
        return Err(ImageIntakeError::MalformedImage);
    }
    Ok(())
}

fn read_leb128(bytes: &[u8], cursor: &mut usize) -> Result<usize, ImageIntakeError> {
    let mut value = 0_u64;
    for i in 0..8 {
        let byte = *bytes.get(*cursor).ok_or(ImageIntakeError::MalformedImage)?;
        *cursor += 1;
        value |= u64::from(byte & 0x7f) << (i * 7);
        if (byte & 0x80) == 0 {
            return usize::try_from(value).map_err(|_| ImageIntakeError::MalformedImage);
        }
    }
    Err(ImageIntakeError::MalformedImage)
}

struct BitReader<'a> {
    bytes: &'a [u8],
    bit_offset: usize,
}

impl<'a> BitReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            bit_offset: 0,
        }
    }

    fn read_bit(&mut self) -> Result<bool, ImageIntakeError> {
        let byte_idx = self.bit_offset / 8;
        let bit_idx = 7 - (self.bit_offset % 8);
        let byte = *self
            .bytes
            .get(byte_idx)
            .ok_or(ImageIntakeError::MalformedImage)?;
        self.bit_offset += 1;
        Ok(((byte >> bit_idx) & 1) != 0)
    }

    fn read_bits(&mut self, count: usize) -> Result<u32, ImageIntakeError> {
        let mut value = 0_u32;
        for _ in 0..count {
            value = (value << 1) | u32::from(self.read_bit()?);
        }
        Ok(value)
    }
}

fn parse_sequence_header(
    payload: &[u8],
    expected_width: u32,
    expected_height: u32,
    av1c: Av1Config,
) -> Result<bool, ImageIntakeError> {
    if payload.is_empty() || *payload.last().unwrap() == 0 {
        return Err(ImageIntakeError::MalformedImage);
    }
    let mut bits = BitReader::new(payload);
    let seq_profile = bits.read_bits(3)? as u8;
    let _still_picture = bits.read_bit()?;
    let reduced_still_picture_header = bits.read_bit()?;
    if seq_profile != av1c.seq_profile {
        return Err(ImageIntakeError::MalformedImage);
    }

    if reduced_still_picture_header {
        let _seq_level_idx_0 = bits.read_bits(5)?;
    } else {
        let timing_info_present = bits.read_bit()?;
        if timing_info_present {
            return Err(ImageIntakeError::AnimatedImage);
        }
        let initial_display_delay_present = bits.read_bit()?;
        let operating_points_cnt_minus_1 = bits.read_bits(5)?;
        for _ in 0..=operating_points_cnt_minus_1 {
            let _idc = bits.read_bits(12)?;
            let seq_level_idx = bits.read_bits(5)?;
            if seq_level_idx > 7 {
                let _seq_tier = bits.read_bit()?;
            }
            if initial_display_delay_present && bits.read_bit()? {
                let _delay_minus_1 = bits.read_bits(4)?;
            }
        }
    }

    let frame_width_bits = (bits.read_bits(4)? + 1) as usize;
    let frame_height_bits = (bits.read_bits(4)? + 1) as usize;
    let max_width = bits.read_bits(frame_width_bits)? + 1;
    let max_height = bits.read_bits(frame_height_bits)? + 1;
    if max_width != expected_width || max_height != expected_height {
        return Err(ImageIntakeError::MalformedImage);
    }

    if !reduced_still_picture_header && bits.read_bit()? {
        let _delta_frame_id_length_minus_2 = bits.read_bits(4)?;
        let _additional_frame_id_length_minus_1 = bits.read_bits(3)?;
    }
    let _use_128x128_superblock = bits.read_bit()?;
    let _enable_filter_intra = bits.read_bit()?;
    let _enable_intra_edge_filter = bits.read_bit()?;

    if !reduced_still_picture_header {
        let _enable_interintra_compound = bits.read_bit()?;
        let _enable_masked_compound = bits.read_bit()?;
        let _enable_warped_motion = bits.read_bit()?;
        let _enable_dual_filter = bits.read_bit()?;
        let enable_order_hint = bits.read_bit()?;
        if enable_order_hint {
            let _enable_jnt_comp = bits.read_bit()?;
            let _enable_ref_frame_mvs = bits.read_bit()?;
        }
        let seq_choose_screen_content_tools = bits.read_bit()?;
        let seq_force_screen_content_tools = if seq_choose_screen_content_tools {
            2
        } else {
            bits.read_bits(1)?
        };
        if seq_force_screen_content_tools > 0 && !bits.read_bit()? {
            let _seq_force_integer_mv = bits.read_bits(1)?;
        }
        if enable_order_hint {
            let _order_hint_bits_minus_1 = bits.read_bits(3)?;
        }
    }

    let _enable_superres = bits.read_bit()?;
    let _enable_cdef = bits.read_bit()?;
    let _enable_restoration = bits.read_bit()?;

    let high_bitdepth = bits.read_bit()?;
    let twelve_bit = if seq_profile == 2 && high_bitdepth {
        bits.read_bit()?
    } else {
        false
    };
    let monochrome = if seq_profile == 1 {
        false
    } else {
        bits.read_bit()?
    };
    if high_bitdepth != av1c.high_bitdepth
        || twelve_bit != av1c.twelve_bit
        || monochrome != av1c.monochrome
    {
        return Err(ImageIntakeError::MalformedImage);
    }
    Ok(reduced_still_picture_header)
}

fn check_frame_header(payload: &[u8], reduced_still_header: bool) -> Result<(), ImageIntakeError> {
    let mut bits = BitReader::new(payload);
    if !reduced_still_header {
        let show_existing_frame = bits.read_bit()?;
        if show_existing_frame {
            return Err(ImageIntakeError::AnimatedImage);
        }
        let frame_type = bits.read_bits(2)?;
        let show_frame = bits.read_bit()?;
        if frame_type != 0 || !show_frame {
            return Err(ImageIntakeError::AnimatedImage);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
