use super::*;
use proptest::prelude::*;
use std::time::Duration;

const WEBP: &[u8] = include_bytes!("fixtures/red-2x3.webp");
const WEBP_HASH: &str = "94b66a3567ab9231ce33865038bcfb42a2d62a2add6ee871dbc4e00b77169d75";

fn lanes() -> Arc<Semaphore> {
    Arc::new(Semaphore::new(2))
}

fn inspect(bytes: &[u8], mime: ImageMime) -> Result<VerifiedImage, ImageIntakeError> {
    let body = AdmittedBody {
        bytes: bytes.to_vec(),
        admission: Arc::new(ImageAdmission::from_lanes(lanes()).unwrap()),
    };
    inspect_body(body, mime).map(|(image, _)| image)
}

#[test]
fn source_uses_actual_opaque_vp8_bytes_and_dimensions() {
    let source = inspect(WEBP, ImageMime::Webp).unwrap();
    assert_eq!(source.bytes(), WEBP);
    assert_eq!(source.metadata().sha256(), WEBP_HASH);
    assert_eq!(
        (source.metadata().width_px(), source.metadata().height_px()),
        (2, 3)
    );
    assert!(inspect(WEBP, ImageMime::Avif).is_err());

    for bytes in [
        include_bytes!("fixtures/ffmpeg-yuv420p-2x3.avif").as_slice(),
        include_bytes!("fixtures/grid-alpha-2x2-120x100.avif").as_slice(),
        include_bytes!("fixtures/grid-2x2-120x100.avif").as_slice(),
        include_bytes!("fixtures/pillow-alpha-64x48.avif").as_slice(),
        include_bytes!("fixtures/pillow-rgb-2x3.avif").as_slice(),
        include_bytes!("fixtures/python-rgb-2x3.avif").as_slice(),
        include_bytes!("fixtures/python-rgba16-2x3.avif").as_slice(),
    ] {
        assert_eq!(
            inspect(bytes, ImageMime::Webp).unwrap_err(),
            ImageIntakeError::FormatMismatch
        );
    }
}

#[test]
fn malformed_animated_alpha_lossless_and_oversized_inputs_fail_closed() {
    for end in 1..WEBP.len() {
        assert!(
            inspect(&WEBP[..end], ImageMime::Webp).is_err(),
            "prefix {end}"
        );
    }
    let mut corrupt = WEBP.to_vec();
    corrupt[20..23].copy_from_slice(&[0xf0, 0xff, 0xff]);
    assert_eq!(
        inspect(&corrupt, ImageMime::Webp).unwrap_err(),
        ImageIntakeError::MalformedImage
    );
    let mut over = WEBP.to_vec();
    over[26..28].copy_from_slice(&4097u16.to_le_bytes());
    assert_eq!(
        inspect(&over, ImageMime::Webp).unwrap_err(),
        ImageIntakeError::ImageLimit
    );
    assert_eq!(
        inspect(&vec![0; MAX_IMAGE_BYTES as usize + 1], ImageMime::Webp).unwrap_err(),
        ImageIntakeError::ImageTooLarge
    );

    let fixtures: std::collections::BTreeMap<String, String> =
        serde_json::from_str(include_str!("fixtures/webp-rejections.json")).unwrap();
    for (name, hex) in fixtures {
        let bytes: Vec<u8> = hex
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect();
        let error = if name == "animation" {
            ImageIntakeError::AnimatedImage
        } else {
            ImageIntakeError::UnsupportedFormat
        };
        assert_eq!(
            inspect(&bytes, ImageMime::Webp).unwrap_err(),
            error,
            "{name}"
        );
    }
}

#[tokio::test]
async fn variants_are_distinct_checked_objects_with_bounded_geometry() {
    let rgb = RgbImage::from_fn(1800, 2700, |x, y| {
        image::Rgb([(x % 255) as u8, (y % 255) as u8, ((x + y) % 255) as u8])
    });
    let bytes = webp::Encoder::from_rgb(rgb.as_raw(), 1800, 2700)
        .encode_simple(false, 86.0)
        .unwrap()
        .to_vec();
    let admission = ImageAdmission::from_lanes(lanes()).unwrap();
    let (source, [thumbnail, full]) = admission
        .inspect_cover(bytes, ImageMime::Webp)
        .await
        .unwrap();
    for (image, width, height) in [(&thumbnail, 384, 576), (&full, 800, 1200)] {
        let decoded = webp::Decoder::new(image.bytes()).decode().unwrap();
        assert_eq!((decoded.width(), decoded.height()), (width, height));
        assert!(!decoded.is_alpha());
        assert_eq!(image.metadata().byte_length(), image.bytes().len() as u64);
        assert_eq!(
            image.metadata().sha256(),
            format!("{:x}", Sha256::digest(image.bytes()))
        );
        assert_ne!(image.metadata().sha256(), source.metadata().sha256());
    }
    assert_ne!(thumbnail.metadata().sha256(), full.metadata().sha256());
    assert!(thumbnail.bytes().len() < full.bytes().len());
    println!(
        "encoded bytes, 1800x2700 synthetic RGB: source {}, thumbnail {}, full {}",
        source.bytes().len(),
        thumbnail.bytes().len(),
        full.bytes().len()
    );
}

#[tokio::test]
async fn two_admissions_refuse_a_third_and_success_keeps_its_body_lane_until_drop() {
    let lanes = lanes();
    let first = ImageAdmission::from_lanes(lanes.clone()).unwrap();
    let second = ImageAdmission::from_lanes(lanes.clone()).unwrap();
    assert_eq!(
        ImageAdmission::from_lanes(lanes.clone()).unwrap_err(),
        ImageIntakeError::AdmissionBusy
    );
    let verified = first.inspect(WEBP.to_vec(), ImageMime::Webp).await.unwrap();
    assert_eq!(lanes.available_permits(), 0);
    drop(second);
    assert_eq!(lanes.available_permits(), 1);
    drop(verified);
    assert_eq!(lanes.available_permits(), 2);
}

#[tokio::test]
async fn unpolled_inspection_future_owns_the_body_and_releases_its_lane_on_drop() {
    let lanes = lanes();
    let admission = ImageAdmission::from_lanes(lanes.clone()).unwrap();
    let future = admission.inspect(WEBP.to_vec(), ImageMime::Webp);
    assert_eq!(lanes.available_permits(), 1);
    drop(future);
    assert_eq!(lanes.available_permits(), 2);
}

#[tokio::test]
async fn cancellation_after_blocking_start_retains_admission_until_worker_and_body_exit() {
    let lanes = lanes();
    let first = ImageAdmission::from_lanes(lanes.clone()).unwrap();
    let _second = ImageAdmission::from_lanes(lanes.clone()).unwrap();
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let task = tokio::spawn(first.run_blocking(WEBP.to_vec(), move |body| {
        started_tx.send(()).unwrap();
        release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        inspect_body(body, ImageMime::Webp)
    }));
    started_rx.await.unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert_eq!(lanes.available_permits(), 0);
    assert_eq!(
        ImageAdmission::from_lanes(lanes.clone()).unwrap_err(),
        ImageIntakeError::AdmissionBusy
    );
    release_tx.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while lanes.available_permits() != 1 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn rejected_bytes_and_panicking_blocking_worker_release_admission() {
    let lanes = lanes();
    let admission = ImageAdmission::from_lanes(lanes.clone()).unwrap();
    assert_eq!(
        admission
            .inspect(vec![1, 2], ImageMime::Webp)
            .await
            .unwrap_err(),
        ImageIntakeError::FormatMismatch
    );
    assert_eq!(lanes.available_permits(), 2);
    let admission = ImageAdmission::from_lanes(lanes.clone()).unwrap();
    assert_eq!(
        admission
            .run_blocking::<VerifiedImage>(WEBP.to_vec(), |_body| panic!("synthetic decoder panic"))
            .await
            .unwrap_err(),
        ImageIntakeError::InspectionUnavailable
    );
    assert_eq!(lanes.available_permits(), 2);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]
    #[test]
    fn arbitrary_bounded_bytes_never_panic_and_success_has_checked_original_facts(
        bytes in proptest::collection::vec(any::<u8>(), 0..8192),
    ) {
        let mime = ImageMime::Webp;
        if let Ok(verified) = inspect(&bytes, mime) {
            prop_assert_eq!(verified.bytes(), bytes.as_slice());
            prop_assert_eq!(verified.metadata().byte_length(), bytes.len() as u64);
            prop_assert_eq!(verified.metadata().content_type(), mime);
            prop_assert!(verified.metadata().width_px() > 0 && verified.metadata().width_px() <= MAX_IMAGE_DIMENSION);
            prop_assert!(verified.metadata().height_px() > 0 && verified.metadata().height_px() <= MAX_IMAGE_DIMENSION);
        }
    }
}
