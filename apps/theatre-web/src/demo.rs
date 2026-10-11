//! Synthetic presentation fixtures. These are neither wire DTOs nor publishable records.
pub struct DemoWork {
    pub title: &'static str,
    pub genre_vi: &'static str,
    pub genre_en: &'static str,
}

pub const WORKS: [DemoWork; 3] = [
    DemoWork {
        title: "Sơn Hà Ký",
        genre_vi: "Tiên hiệp",
        genre_en: "Cultivation fantasy",
    },
    DemoWork {
        title: "Đèn khuya bên sông",
        genre_vi: "Dã sử",
        genre_en: "Historical fiction",
    },
    DemoWork {
        title: "Mộng hồ điệp",
        genre_vi: "Huyền ảo",
        genre_en: "Fantasy",
    },
];

pub fn matching_works(query: &str) -> Vec<&'static DemoWork> {
    let query = query.trim().to_lowercase();
    WORKS
        .iter()
        .filter(|work| work.title.to_lowercase().contains(&query))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_search_handles_vietnamese_case_spaces_and_no_result() {
        assert_eq!(matching_works("  ĐÈN  ")[0].title, "Đèn khuya bên sông");
        assert_eq!(matching_works("").len(), 3);
        assert!(matching_works("không tồn tại").is_empty());
    }
}
