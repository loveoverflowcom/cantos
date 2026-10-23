//! Library request builders for tests: the exact wire values a Theatre client sends, derived from
//! a seeded publication. Pure constructors; no persistence and no authority.
use std::sync::Arc;

use crate::contract::theatre::library as dto;
use cantos_store_turso::testing::FixedClock;
use uuid::Uuid;

use super::{Chain, SeededUnit, SeededWork, ServerFixture, WorkSeed};
use crate::{
    contexts::listening::{auth::AuthenticatedListener, LibraryService},
    platform::identity::domain::ActorId,
};

/// 2030-03-17 in microseconds: far from any real clock, so a leaked wall-clock read shows.
pub const T0: i64 = 1_900_000_000_000_000;

/// The sealed principal a trusted host would vouch for (no Identity session behind it).
pub fn principal(actor: &str) -> AuthenticatedListener {
    AuthenticatedListener::from_verified_actor(ActorId::new(actor).expect("a valid actor"))
}

/// One published unit on a fixed clock, with two listener accounts, `alice` and `bob`.
pub struct LibraryWorld {
    pub fixture: ServerFixture,
    pub clock: Arc<FixedClock>,
    pub work: SeededWork,
    pub audio: dto::AudioIdentity,
}

impl LibraryWorld {
    pub async fn new() -> Self {
        let clock = FixedClock::new(T0);
        let fixture = ServerFixture::with_clock(clock.clone()).await;
        let work = fixture
            .seed_work(
                None,
                &WorkSeed::podcast(1, "Ánh đèn cuối sân khấu", &["Tập có âm thanh riêng tư"]),
            )
            .await;
        fixture.seed_actors(&["alice", "bob"]).await;
        let audio = audio(&work, &work.units[0]);
        Self {
            fixture,
            clock,
            work,
            audio,
        }
    }
    pub fn service(&self) -> LibraryService {
        LibraryService::new(self.fixture.store().clone())
    }
    /// Rows of one Library table owned by `who`.
    pub async fn count(&self, table: &str, who: &str) -> i64 {
        self.fixture.listener_rows(table, who).await
    }
}

/// Progress, Saved and one history start for `who` on `a`.
pub async fn remember_audio(
    service: &LibraryService,
    who: &AuthenticatedListener,
    a: &dto::AudioIdentity,
) {
    service
        .put_progress(
            who,
            &a.publication_id,
            progress(a, 1, 90, 0, 100, dto::ProgressKind::Checkpoint),
        )
        .await
        .expect("progress is accepted");
    service
        .set_saved(who, &a.work_id, saved(1, 90, 0, true))
        .await
        .expect("saved is accepted");
    service
        .playback_started(who, history_start(a, 1, 90, 99, 0))
        .await
        .expect("the start is accepted");
}

/// A canonical id for tests that need a stable device or session.
pub fn uid(value: u64) -> String {
    format!("00000000-0000-4000-8000-{value:012x}")
}

/// A fresh operation for `device` at `sequence`.
pub fn operation(sequence: u64, device: u64) -> dto::Operation {
    dto::Operation {
        operation_id: Uuid::new_v4().to_string(),
        device_id: uid(device),
        device_sequence: sequence,
        device_reported_at_ms: None,
    }
}

/// The exact audio identity of a seeded, published unit.
pub fn audio(work: &SeededWork, unit: &SeededUnit) -> dto::AudioIdentity {
    audio_of(work, unit, &unit.chain)
}

pub fn audio_of(work: &SeededWork, unit: &SeededUnit, chain: &Chain) -> dto::AudioIdentity {
    dto::AudioIdentity {
        work_id: work.work_id.clone(),
        unit_id: unit.unit_id.clone(),
        publication_id: chain
            .release_id
            .clone()
            .expect("the chain has a published release"),
        rendition_id: chain.rendition_id.clone(),
        sha256: chain.sha256.clone(),
        duration_ms: chain.duration_ms,
    }
}

pub fn progress(
    audio: &dto::AudioIdentity,
    sequence: u64,
    device: u64,
    base: u64,
    position: u64,
    kind: dto::ProgressKind,
) -> dto::ProgressWrite {
    dto::ProgressWrite {
        operation: operation(sequence, device),
        audio: audio.clone(),
        base_revision: base,
        position_ms: position,
        kind,
    }
}

pub fn saved(sequence: u64, device: u64, base: u64, saved: bool) -> dto::SavedWrite {
    dto::SavedWrite {
        operation: operation(sequence, device),
        base_revision: base,
        saved,
    }
}

pub fn history_start(
    audio: &dto::AudioIdentity,
    sequence: u64,
    device: u64,
    session: u64,
    epoch: u64,
) -> dto::HistoryStart {
    dto::HistoryStart {
        operation: operation(sequence, device),
        audio: audio.clone(),
        engine_session_id: uid(session),
        event: dto::PlaybackEvent::PlaybackStarted,
        history_epoch: epoch,
    }
}

pub fn history_clear(sequence: u64, device: u64, base: u64, epoch: u64) -> dto::HistoryClear {
    dto::HistoryClear {
        operation: operation(sequence, device),
        base_revision: base,
        history_epoch: epoch,
        confirmation: dto::ClearConfirmation::ClearHistory,
    }
}

pub fn bookmark(
    id: u64,
    audio: &dto::AudioIdentity,
    sequence: u64,
    device: u64,
    position: u64,
) -> dto::BookmarkWrite {
    dto::BookmarkWrite {
        operation: operation(sequence, device),
        bookmark_id: uid(0xb00 + id),
        audio: audio.clone(),
        position_ms: position,
    }
}

pub fn bookmark_delete(sequence: u64, device: u64) -> dto::BookmarkDelete {
    dto::BookmarkDelete {
        operation: operation(sequence, device),
    }
}
