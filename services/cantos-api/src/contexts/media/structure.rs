//! Strict finite container framing before a demuxer sees hostile bytes.
use super::audio_intake::AudioIntakeError;

#[derive(Debug, PartialEq)]
pub(super) struct ContainerFacts {
    pub name: &'static str,
    pub duration_seconds: f64,
}

pub(super) fn check(bytes: &[u8]) -> Result<ContainerFacts, AudioIntakeError> {
    if bytes.starts_with(b"OggS") {
        Ok(ContainerFacts {
            name: "ogg",
            duration_seconds: ogg(bytes)?,
        })
    } else {
        Err(AudioIntakeError::UnsupportedContainer)
    }
}

fn ogg(bytes: &[u8]) -> Result<f64, AudioIntakeError> {
    let mut offset = 0;
    let mut sequence = 0_u32;
    let mut serial = None;
    let mut pending = Vec::new();
    let mut packets = 0;
    let mut ended = false;
    let mut last_granule = 0;
    let mut pre_skip = 0_u64;
    let mut coded_samples = 0_u64;
    let mut last_packet_samples = 0_u64;
    let table = crc_table();
    while offset < bytes.len() {
        let header = bytes
            .get(offset..offset + 27)
            .ok_or(AudioIntakeError::MalformedAudio)?;
        if ended || &header[..4] != b"OggS" || header[4] != 0 || header[5] & !7 != 0 {
            return Err(AudioIntakeError::MalformedAudio);
        }
        let flags = header[5];
        let page_serial = u32::from_le_bytes(
            header[14..18]
                .try_into()
                .map_err(|_| AudioIntakeError::MalformedAudio)?,
        );
        let page_sequence = u32::from_le_bytes(
            header[18..22]
                .try_into()
                .map_err(|_| AudioIntakeError::MalformedAudio)?,
        );
        if page_sequence != sequence
            || serial.is_some_and(|value| value != page_serial)
            || (flags & 2 != 0) != (sequence == 0)
            || (flags & 1 != 0) == pending.is_empty()
        {
            return Err(AudioIntakeError::MalformedAudio);
        }
        serial = Some(page_serial);
        sequence = sequence
            .checked_add(1)
            .ok_or(AudioIntakeError::MalformedAudio)?;
        let laces = bytes
            .get(offset + 27..offset + 27 + usize::from(header[26]))
            .ok_or(AudioIntakeError::MalformedAudio)?;
        let payload_size: usize = laces.iter().map(|size| usize::from(*size)).sum();
        let end = offset + 27 + laces.len() + payload_size;
        let page = bytes
            .get(offset..end)
            .ok_or(AudioIntakeError::MalformedAudio)?;
        let expected_crc = u32::from_le_bytes(
            header[22..26]
                .try_into()
                .map_err(|_| AudioIntakeError::MalformedAudio)?,
        );
        let mut crc = 0_u32;
        for (index, byte) in page.iter().enumerate() {
            let byte = if (22..26).contains(&index) { 0 } else { *byte };
            crc = (crc << 8) ^ table[usize::from(((crc >> 24) as u8) ^ byte)];
        }
        if crc != expected_crc {
            return Err(AudioIntakeError::MalformedAudio);
        }
        let granule = u64::from_le_bytes(
            header[6..14]
                .try_into()
                .map_err(|_| AudioIntakeError::MalformedAudio)?,
        );
        if granule != u64::MAX {
            if granule < last_granule {
                return Err(AudioIntakeError::MalformedAudio);
            }
            last_granule = granule;
        }
        let mut cursor = offset + 27 + laces.len();
        for size in laces {
            let size = usize::from(*size);
            pending.extend_from_slice(&bytes[cursor..cursor + size]);
            cursor += size;
            if pending.len() > 65_536 {
                return Err(AudioIntakeError::MalformedAudio);
            }
            if size < 255 {
                match packets {
                    0 if pending.len() == 19
                        && pending.starts_with(b"OpusHead")
                        && pending[8] == 1
                        && pending[18] == 0 =>
                    {
                        pre_skip = u64::from(u16::from_le_bytes([pending[10], pending[11]]));
                    }
                    0 => return Err(AudioIntakeError::CodecMismatch),
                    1 if pending.starts_with(b"OpusTags") => (),
                    1 => return Err(AudioIntakeError::MalformedAudio),
                    _ if pending.is_empty() => return Err(AudioIntakeError::MalformedAudio),
                    _ => {
                        last_packet_samples = opus_packet_samples(&pending)?;
                        coded_samples = coded_samples
                            .checked_add(last_packet_samples)
                            .ok_or(AudioIntakeError::MalformedAudio)?;
                    }
                }
                packets += 1;
                pending.clear();
            }
        }
        ended = flags & 4 != 0;
        if coded_samples > 0 && granule != u64::MAX {
            let valid_granule = if ended {
                granule <= coded_samples && granule > coded_samples - last_packet_samples
            } else {
                granule == coded_samples
            };
            if !valid_granule {
                return Err(AudioIntakeError::MalformedAudio);
            }
        }
        if ended && (!pending.is_empty() || granule == u64::MAX || granule == 0) {
            return Err(AudioIntakeError::MalformedAudio);
        }
        offset = end;
    }
    if !ended || packets < 3 || !pending.is_empty() {
        return Err(AudioIntakeError::MalformedAudio);
    }
    if last_granule <= pre_skip {
        return Err(AudioIntakeError::MalformedAudio);
    }
    let duration = (last_granule - pre_skip) as f64 / 48_000.0;
    if duration > 3600.0 {
        return Err(AudioIntakeError::DurationLimit);
    }
    Ok(duration)
}

fn opus_packet_samples(packet: &[u8]) -> Result<u64, AudioIntakeError> {
    let toc = *packet.first().ok_or(AudioIntakeError::MalformedAudio)?;
    let config = toc >> 3;
    let frame_samples = if config >= 16 {
        120_u64 << (config & 3)
    } else if config >= 12 {
        480_u64 << (config & 1)
    } else {
        [480, 960, 1920, 2880][usize::from(config & 3)]
    };
    let frames = match toc & 3 {
        0 => 1,
        1 | 2 => 2,
        _ => u64::from(*packet.get(1).ok_or(AudioIntakeError::MalformedAudio)? & 0x3f),
    };
    let samples = frame_samples * frames;
    if frames == 0 || samples > 5760 {
        return Err(AudioIntakeError::MalformedAudio);
    }
    Ok(samples)
}

fn crc_table() -> [u32; 256] {
    let mut table = [0; 256];
    for (index, entry) in table.iter_mut().enumerate() {
        let mut value = (index as u32) << 24;
        for _ in 0..8 {
            value = if value & 0x8000_0000 == 0 {
                value << 1
            } else {
                (value << 1) ^ 0x04c1_1db7
            };
        }
        *entry = value;
    }
    table
}

#[cfg(test)]
mod tests {
    use super::check;
    use crate::contexts::media::audio_intake::AudioIntakeError;

    #[test]
    fn external_references_and_incomplete_containers_are_rejected() {
        assert_eq!(check(b"OggS"), Err(AudioIntakeError::MalformedAudio));
        assert_eq!(
            check(b"RIFF fake opus"),
            Err(AudioIntakeError::UnsupportedContainer)
        );
        assert_eq!(
            check(&[0, 0, 0, 32, b'f', b't', b'y', b'p']),
            Err(AudioIntakeError::UnsupportedContainer)
        );
    }
}
