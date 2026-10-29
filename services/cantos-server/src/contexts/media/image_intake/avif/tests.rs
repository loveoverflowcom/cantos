use super::*;

const ALPHA: &[u8] = include_bytes!("../fixtures/pillow-alpha-64x48.avif");

/// The fixture generator draws this alpha plane (`build_alpha_grid_fixtures.py`, 64x48).
fn expected_alpha(x: u32, y: u32) -> u8 {
    if x <= 10 {
        return 0;
    }
    let (dx, dy) = (x as i64 - 32, y as i64 - 24);
    (255 - (dx * dx + dy * dy) / 8).clamp(0, 255) as u8
}

fn item_bytes(file: &[u8], item: u32) -> (Vec<Reference>, Vec<u8>) {
    let (mut meta, mut mdat, mut offset) = (None, None, 0);
    while offset < file.len() {
        let (kind, payload, next, payload_offset) = next_box(file, offset).unwrap();
        match &kind {
            b"meta" => meta = Some(payload),
            b"mdat" => mdat = Some((payload_offset, next)),
            _ => {}
        }
        offset = next;
    }
    let (mut iloc, mut iref, mut offset) = (None, None, 0);
    let body = &meta.unwrap()[4..];
    while offset < body.len() {
        let (kind, payload, next, _) = next_box(body, offset).unwrap();
        match &kind {
            b"iloc" => iloc = Some(payload),
            b"iref" => iref = Some(payload),
            _ => {}
        }
        offset = next;
    }
    let locations = parse_iloc(iloc.unwrap()).unwrap();
    let extent = locations.iter().find(|(id, _)| *id == item).unwrap().1;
    let (start, end) = mdat.unwrap();
    let bytes = slice_in_mdat(file, extent, start, end).unwrap().to_vec();
    (parse_iref(iref.unwrap()).unwrap(), bytes)
}

#[test]
fn decoded_alpha_plane_matches_the_source_alpha_exactly() {
    let (references, alpha) = item_bytes(ALPHA, 2);
    assert!(references
        .iter()
        .any(|reference| &reference.kind == b"auxl" && reference.from == 2 && reference.to == [1]));
    let mut settings = dav1d::Settings::new();
    settings.set_n_threads(1);
    settings.set_max_frame_delay(1);
    let mut decoder = dav1d::Decoder::with_settings(&settings).unwrap();
    decoder.send_data(alpha, None, None, None).unwrap();
    let picture = decoder.get_picture().unwrap();
    assert_eq!(picture.pixel_layout(), dav1d::PixelLayout::I400);
    assert_eq!((picture.width(), picture.height()), (64, 48));
    let plane = picture.plane(dav1d::PlanarImageComponent::Y);
    let stride = picture.stride(dav1d::PlanarImageComponent::Y) as usize;
    for y in 0..48 {
        for x in 0..64 {
            assert_eq!(
                plane[y as usize * stride + x as usize],
                expected_alpha(x, y),
                "({x}, {y})"
            );
        }
    }
}
