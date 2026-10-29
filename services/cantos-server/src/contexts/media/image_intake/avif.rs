//! Bounded ISOBMFF + AV1 still-image inspection for `image/avif`.
//!
//! Enforces the same dimension, pixel-product, decoded-byte (`MAX_DECODED_IMAGE_BYTES`),
//! non-animated (`avis`, `moov`, multi-frame OBU) and identity-orientation (`irot`, `imir`,
//! `clap`, `Exif`) barriers as the PNG and JPEG inspectors while preserving original bytes.
use image::metadata::Orientation;

use super::{
    super::domain::image::{check_image_bounds, MAX_IMAGE_DIMENSION},
    ImageIntakeError,
};

#[derive(Clone, Copy)]
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
    Irot { angle: u8 },
    NonIdentityTransform,
    Other,
}

#[derive(Clone, Copy)]
struct ItemExtent {
    offset: usize,
    length: usize,
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
            b"iprp" => set_once(&mut iprp_payload, payload)?,
            _ => {}
        }
        offset = next_offset;
    }

    let pitm = primary_item_id.ok_or(ImageIntakeError::MalformedImage)?;
    let exif_items = parse_iinf(iinf_payload.ok_or(ImageIntakeError::MalformedImage)?, pitm)?;
    let locations = parse_iloc(iloc_payload.ok_or(ImageIntakeError::MalformedImage)?)?;
    let (width, height, av1c) =
        parse_iprp(iprp_payload.ok_or(ImageIntakeError::MalformedImage)?, pitm)?;

    let bounds = check_image_bounds(
        file_bytes.len() as u64,
        width,
        height,
        av1c.bytes_per_pixel(),
    )
    .map_err(|_| ImageIntakeError::ImageLimit)?;
    let mut decoded_probe: Vec<u8> = Vec::new();
    decoded_probe
        .try_reserve_exact(bounds.decoded_byte_length())
        .map_err(|_| ImageIntakeError::InspectionUnavailable)?;

    for exif_id in exif_items {
        let extent = locations
            .iter()
            .find(|(id, _)| *id == exif_id)
            .map(|(_, ext)| *ext)
            .ok_or(ImageIntakeError::MalformedImage)?;
        let exif_bytes = slice_in_mdat(file_bytes, extent, mdat_start, mdat_end)?;
        check_avif_exif(exif_bytes)?;
    }

    let primary_extent = locations
        .iter()
        .find(|(id, _)| *id == pitm)
        .map(|(_, ext)| *ext)
        .ok_or(ImageIntakeError::MalformedImage)?;
    let primary_bytes = slice_in_mdat(file_bytes, primary_extent, mdat_start, mdat_end)?;

    // Every byte in mdat must belong to a declared item extent in iloc; reject appended mdat data.
    let total_item_bytes: usize = locations.iter().map(|(_, ext)| ext.length).sum();
    if total_item_bytes != mdat_end - mdat_start {
        return Err(ImageIntakeError::MalformedImage);
    }

    inspect_av1_obus(primary_bytes, width, height, av1c)?;
    drop(decoded_probe);
    Ok((width, height))
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

fn parse_iinf(payload: &[u8], primary_item_id: u32) -> Result<Vec<u32>, ImageIntakeError> {
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
    let mut found_primary = false;
    let mut exif_items = Vec::new();
    for _ in 0..count {
        let (box_type, infe, next_offset, _) = next_box(payload, offset)?;
        if &box_type != b"infe" || infe.len() < 12 {
            return Err(ImageIntakeError::MalformedImage);
        }
        let infe_version = infe[0];
        let (item_id, item_type) = match infe_version {
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
        if item_id == primary_item_id {
            if item_type != b"av01" {
                return Err(ImageIntakeError::MalformedImage);
            }
            found_primary = true;
        } else if item_type == b"Exif" {
            exif_items.push(item_id);
        }
        offset = next_offset;
    }
    if !found_primary || offset != payload.len() {
        return Err(ImageIntakeError::MalformedImage);
    }
    Ok(exif_items)
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

fn parse_iprp(
    payload: &[u8],
    primary_item_id: u32,
) -> Result<(u32, u32, Av1Config), ImageIntakeError> {
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

    let associated = parse_ipma(ipma, primary_item_id, properties.len())?;
    let mut ispe_dims: Option<(u32, u32)> = None;
    let mut av1c_cfg: Option<Av1Config> = None;
    let mut pixi_depth: Option<u8> = None;
    for idx in associated {
        match properties[idx - 1] {
            Property::Ispe { width, height } => ispe_dims = Some((width, height)),
            Property::Av1C(cfg) => av1c_cfg = Some(cfg),
            Property::Pixi { depth } => pixi_depth = Some(depth),
            // Non-identity transforms were already refused while reading `ipco`.
            Property::Irot { .. } | Property::NonIdentityTransform | Property::Other => {}
        }
    }
    let (width, height) = ispe_dims.ok_or(ImageIntakeError::MalformedImage)?;
    let cfg = av1c_cfg.ok_or(ImageIntakeError::MalformedImage)?;
    if let Some(depth) = pixi_depth {
        if depth != cfg.bit_depth() {
            return Err(ImageIntakeError::MalformedImage);
        }
    }
    Ok((width, height, cfg))
}

fn parse_ipma(
    payload: &[u8],
    primary_item_id: u32,
    property_count: usize,
) -> Result<Vec<usize>, ImageIntakeError> {
    if payload.len() < 8 {
        return Err(ImageIntakeError::MalformedImage);
    }
    let version = payload[0];
    let flags = u32::from_be_bytes([0, payload[1], payload[2], payload[3]]);
    let entry_count = u32::from_be_bytes(payload[4..8].try_into().unwrap()) as usize;
    let mut cursor = 8_usize;
    let mut result = None;
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
        if item_id == primary_item_id {
            result = Some(indices);
        }
    }
    if cursor != payload.len() {
        return Err(ImageIntakeError::MalformedImage);
    }
    result.ok_or(ImageIntakeError::MalformedImage)
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
