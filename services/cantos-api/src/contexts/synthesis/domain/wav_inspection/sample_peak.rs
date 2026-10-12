//! Per-channel sample peak over stored integer codes. This is not true (inter-sample) peak.
use serde::Serialize;

use super::fmt_chunk::{PcmLayout, SampleWidth};

/// `full_scale` is the magnitude of the most negative code of the container (the 0 dBFS
/// reference); `dbfs` is `None` for digital silence, whose level is minus infinity.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SamplePeak {
    pub full_scale: u32,
    pub overall: PeakLevel,
    pub channels: Vec<ChannelPeak>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct PeakLevel {
    pub magnitude: u32,
    pub dbfs: Option<f64>,
}

/// `first_frame` is the first frame whose sample in this channel reaches `magnitude`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct ChannelPeak {
    pub channel: u16,
    pub magnitude: u32,
    pub dbfs: Option<f64>,
    pub first_frame: u64,
}

/// `payload` holds whole frames of `pcm`; `None` means there was no sample to measure.
pub(super) fn measure(payload: &[u8], pcm: PcmLayout) -> Option<SamplePeak> {
    if payload.is_empty() {
        return None;
    }
    let channels = usize::from(pcm.channels());
    let peaks = match pcm.width() {
        SampleWidth::U8 => channel_peaks(payload, channels, |[code]: [u8; 1]| {
            u32::from(code.abs_diff(128))
        }),
        SampleWidth::I16 => channel_peaks(payload, channels, |code: [u8; 2]| {
            u32::from(i16::from_le_bytes(code).unsigned_abs())
        }),
        SampleWidth::I24 => channel_peaks(payload, channels, |[low, middle, high]: [u8; 3]| {
            let sign = if high & 0x80 == 0 { 0x00 } else { 0xff };
            i32::from_le_bytes([low, middle, high, sign]).unsigned_abs()
        }),
        SampleWidth::I32 => channel_peaks(payload, channels, |code: [u8; 4]| {
            i32::from_le_bytes(code).unsigned_abs()
        }),
    };
    let full_scale = pcm.width().full_scale();
    let channels: Vec<ChannelPeak> = (0..=u16::MAX)
        .zip(peaks)
        .map(|(channel, (magnitude, first_frame))| ChannelPeak {
            channel,
            magnitude,
            dbfs: dbfs(magnitude, full_scale),
            first_frame,
        })
        .collect();
    let magnitude = channels
        .iter()
        .map(|peak| peak.magnitude)
        .max()
        .unwrap_or(0);
    Some(SamplePeak {
        full_scale,
        overall: PeakLevel {
            magnitude,
            dbfs: dbfs(magnitude, full_scale),
        },
        channels,
    })
}

/// Returns `(magnitude, first_frame)` per channel; ties keep the earliest frame.
fn channel_peaks<const N: usize>(
    payload: &[u8],
    channels: usize,
    magnitude: impl Fn([u8; N]) -> u32,
) -> Vec<(u32, u64)> {
    let mut peaks = vec![(0, 0); channels];
    for (frame, samples) in (0_u64..).zip(payload.chunks_exact(N * channels)) {
        for (peak, sample) in peaks.iter_mut().zip(samples.chunks_exact(N)) {
            let Ok(code) = <[u8; N]>::try_from(sample) else {
                continue;
            };
            let level = magnitude(code);
            if level > peak.0 {
                *peak = (level, frame);
            }
        }
    }
    peaks
}

fn dbfs(magnitude: u32, full_scale: u32) -> Option<f64> {
    (magnitude > 0).then(|| 20.0 * (f64::from(magnitude) / f64::from(full_scale)).log10())
}
