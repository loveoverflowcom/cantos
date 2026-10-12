//! Deterministic synthetic catalog adapter. No publication, HTTP, identity or media access.
use crate::{
    application::{
        catalog::{CatalogOutcome, CatalogRequest, CatalogResponse, Scenario},
        ports::CatalogPort,
    },
    core::{
        catalog::{Episode, Genre, Work},
        ids::WorkId,
    },
};

const MOUNTAIN: [Episode; 6] = [
    Episode {
        title: "Một tiếng chuông trong sương",
        duration: 1480,
        available: true,
    },
    Episode {
        title: "Bến cũ bên dòng Vọng",
        duration: 1604,
        available: true,
    },
    Episode {
        title: "Người giữ tiếng chuông",
        duration: 1722,
        available: true,
    },
    Episode {
        title: "Ngày mai, mình có diễn tiếp không?",
        duration: 1588,
        available: true,
    },
    Episode {
        title: "Ánh đèn cuối sân khấu — Một lời hẹn chưa bao giờ nói ra",
        duration: 1833,
        available: true,
    },
    Episode {
        title: "Cửa núi chưa mở",
        duration: 0,
        available: false,
    },
];
const RIVER: [Episode; 4] = [
    Episode {
        title: "Ngọn đèn đầu ngõ",
        duration: 1232,
        available: true,
    },
    Episode {
        title: "Người dẫn chuyện bên sông",
        duration: 1470,
        available: true,
    },
    Episode {
        title: "Một lá thư gửi mùa thu",
        duration: 1640,
        available: true,
    },
    Episode {
        title: "Trở về bến cũ",
        duration: 1320,
        available: true,
    },
];
const DREAM: [Episode; 5] = [
    Episode {
        title: "Cánh bướm trong vườn",
        duration: 1328,
        available: true,
    },
    Episode {
        title: "Giấc mơ của người kể chuyện",
        duration: 1560,
        available: true,
    },
    Episode {
        title: "Dấu chân bên hiên vắng",
        duration: 1474,
        available: true,
    },
    Episode {
        title: "Qua một miền tưởng tượng",
        duration: 1620,
        available: true,
    },
    Episode {
        title: "Khi bình minh trở lại",
        duration: 1510,
        available: true,
    },
];
pub fn work(id: WorkId) -> Work {
    match id {
        WorkId::Mountain => Work {
            id,
            title: "Sơn Hà Ký",
            genre: Genre::Fantasy,
            synopsis: "Một tiếng chuông lay động sơn hà. Một hành trình tìm lại điều đã lãng quên.",
            episodes: &MOUNTAIN,
        },
        WorkId::River => Work {
            id,
            title: "Đèn khuya bên sông",
            genre: Genre::Memory,
            synopsis: "Ánh đèn trên bến nhỏ, những lá thư và một câu chuyện đợi người trở về.",
            episodes: &RIVER,
        },
        WorkId::Dream => Work {
            id,
            title: "Mộng hồ điệp",
            genre: Genre::Dream,
            synopsis: "Theo một cánh bướm qua những miền ký ức, nơi giấc mơ gặp lời kể.",
            episodes: &DREAM,
        },
    }
}

/// Deterministic local fixture adapter. No URL, network request, account or backend DTO.
#[derive(Default)]
pub struct MockCatalog;
impl CatalogPort for MockCatalog {
    fn work(&self, id: WorkId) -> Work {
        work(id)
    }
    fn fetch(&mut self, request: CatalogRequest) -> CatalogResponse {
        let outcome = match request.scenario {
            Scenario::Ready => CatalogOutcome::Ready(WorkId::ALL.to_vec()),
            Scenario::Loading => CatalogOutcome::Pending,
            Scenario::Empty => CatalogOutcome::Empty,
            Scenario::Error => CatalogOutcome::Error,
        };
        CatalogResponse {
            generation: request.generation,
            outcome,
        }
    }
}
