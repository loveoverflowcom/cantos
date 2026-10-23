//! Thin fixture access to the real Server Turso owner. SQL is native at each call site.
//! No parser, dialect rewriting, alternate database or simulated persistence lives here.
#![allow(dead_code)]
use cantos_server::{
    platform::store::{FromColumn, Params, Row, RowIndex, Store, StoreError},
    test_support::ServerFixture,
};

pub struct NativeSql(pub ServerFixture);
pub struct NativeRow(Row);
impl NativeRow {
    pub fn get<I: RowIndex, T: FromColumn>(&self, index: I) -> T {
        self.0
            .try_get(index)
            .expect("fixture column has its declared type")
    }
}
impl NativeSql {
    pub async fn new() -> Self {
        Self(ServerFixture::new().await)
    }
    pub fn store(&self) -> Store {
        self.0.store().clone()
    }
    pub async fn execute(&self, sql: &str, params: Params<'_>) -> Result<u64, StoreError> {
        self.0.exec(sql, params).await
    }
    pub async fn query(&self, sql: &str, params: Params<'_>) -> Result<Vec<NativeRow>, StoreError> {
        Ok(self
            .0
            .rows(sql, params)
            .await?
            .into_iter()
            .map(NativeRow)
            .collect())
    }
    pub async fn query_one(&self, sql: &str, params: Params<'_>) -> Result<NativeRow, StoreError> {
        let mut rows = self.query(sql, params).await?;
        assert_eq!(rows.len(), 1, "fixture exact query cardinality");
        Ok(rows.remove(0))
    }
}

/// Media suites historically ran one case at a time. Keep that resource boundary in default
/// Cargo discovery: production intentionally refuses more than two concurrent inspectors.
/// Tasks raced inside one case remain concurrent and still exercise admission/commit ordering.
pub async fn media_lane() -> tokio::sync::OwnedSemaphorePermit {
    static LANE: std::sync::OnceLock<std::sync::Arc<tokio::sync::Semaphore>> =
        std::sync::OnceLock::new();
    LANE.get_or_init(|| std::sync::Arc::new(tokio::sync::Semaphore::new(1)))
        .clone()
        .acquire_owned()
        .await
        .unwrap()
}
