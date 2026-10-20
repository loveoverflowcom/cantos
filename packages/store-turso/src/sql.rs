//! Small typed readers over `turso::Rows`, shared by startup observation, backup verification
//! and the capability probes. A column of an unexpected type is corruption or misuse, never a
//! default value.

use turso::{Connection, IntoParams, Row, Value};

use crate::error::StoreFault;

pub(crate) fn integer(row: &Row, index: usize) -> Result<i64, StoreFault> {
    match row.get_value(index)? {
        Value::Integer(value) => Ok(value),
        _ => Err(StoreFault::Corrupt),
    }
}

pub(crate) fn text(row: &Row, index: usize) -> Result<String, StoreFault> {
    match row.get_value(index)? {
        Value::Text(value) => Ok(value),
        _ => Err(StoreFault::Corrupt),
    }
}

pub(crate) fn blob(row: &Row, index: usize) -> Result<Vec<u8>, StoreFault> {
    match row.get_value(index)? {
        Value::Blob(value) => Ok(value),
        _ => Err(StoreFault::Corrupt),
    }
}

/// The first column of the first row, drained so no statement stays open on the connection.
pub(crate) async fn scalar_integer(
    conn: &Connection,
    sql: &str,
    params: impl IntoParams,
) -> Result<i64, StoreFault> {
    let mut rows = conn.query(sql, params).await?;
    let value = match rows.next().await? {
        Some(row) => integer(&row, 0)?,
        None => return Err(StoreFault::NoRows),
    };
    while rows.next().await?.is_some() {}
    Ok(value)
}

pub(crate) async fn scalar_text(conn: &Connection, sql: &str) -> Result<String, StoreFault> {
    let mut rows = conn.query(sql, ()).await?;
    let value = match rows.next().await? {
        Some(row) => text(&row, 0)?,
        None => return Err(StoreFault::NoRows),
    };
    while rows.next().await?.is_some() {}
    Ok(value)
}
