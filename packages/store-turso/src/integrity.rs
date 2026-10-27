//! Cold, owner-excluded integrity checks on the pinned engine. Turso 0.8.3 accepts
//! `foreign_key_check` but returns no violations for a dangling row; use actual FK metadata
//! and bounded anti-joins instead. Unknown/implicit parent-key forms refuse rather than pass.
use std::collections::BTreeMap;

use turso::{Connection, Value};

use crate::{error::StoreFault, sql};

struct ForeignKey {
    parent: String,
    columns: Vec<(String, String)>,
}

fn quoted(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

async fn foreign_keys(conn: &Connection, table: &str) -> Result<Vec<ForeignKey>, StoreFault> {
    let mut rows = conn
        .query(&format!("PRAGMA foreign_key_list({})", quoted(table)), ())
        .await?;
    let mut keys: BTreeMap<i64, ForeignKey> = BTreeMap::new();
    let mut count = 0;
    while let Some(row) = rows.next().await? {
        count += 1;
        let id = sql::integer(&row, 0)?;
        let sequence = sql::integer(&row, 1)?;
        if count > 4096 || id < 0 || sql::text(&row, 7)? != "NONE" {
            return Err(StoreFault::Misuse);
        }
        let parent = sql::text(&row, 2)?;
        let key = keys.entry(id).or_insert_with(|| ForeignKey {
            parent: parent.clone(),
            columns: Vec::new(),
        });
        if key.parent != parent || key.columns.len() >= 64 || sequence != key.columns.len() as i64 {
            return Err(StoreFault::Corrupt);
        }
        let parent_column = match row.get_value(4)? {
            Value::Text(name) if !name.is_empty() => name,
            Value::Text(_) | Value::Null => return Err(StoreFault::Misuse),
            _ => return Err(StoreFault::Corrupt),
        };
        // The reviewed native registries name parent columns explicitly. An implicit PK
        // is valid SQL but outside this cold-checker's supported forms, so it refuses.
        key.columns.push((sql::text(&row, 3)?, parent_column));
    }
    Ok(keys.into_values().collect())
}

pub(crate) async fn check(conn: &Connection) -> Result<(), StoreFault> {
    if sql::scalar_text(conn, "PRAGMA integrity_check").await? != "ok" {
        return Err(StoreFault::Corrupt);
    }
    let mut rows = conn
        .query("SELECT name FROM sqlite_schema WHERE type='table' AND substr(name,1,7)<>'sqlite_' ORDER BY name", ())
        .await?;
    let mut tables = Vec::new();
    while let Some(row) = rows.next().await? {
        if tables.len() >= 1024 {
            return Err(StoreFault::Misuse);
        }
        tables.push(sql::text(&row, 0)?);
    }
    drop(rows);
    for table in tables {
        for key in foreign_keys(conn, &table).await? {
            let non_null = key
                .columns
                .iter()
                .map(|(child, _)| format!("c.{} IS NOT NULL", quoted(child)))
                .collect::<Vec<_>>()
                .join(" AND ");
            // Parent column on the left preserves the parent key's affinity/collation.
            // MATCH SIMPLE exempts the entire composite key if any child column is NULL.
            let equal = key
                .columns
                .iter()
                .map(|(child, parent)| format!("p.{}=c.{}", quoted(parent), quoted(child)))
                .collect::<Vec<_>>()
                .join(" AND ");
            let query = format!("SELECT EXISTS(SELECT 1 FROM {} c WHERE {non_null} AND NOT EXISTS(SELECT 1 FROM {} p WHERE {equal}))", quoted(&table), quoted(&key.parent));
            if sql::scalar_integer(conn, &query, ()).await? != 0 {
                return Err(StoreFault::Corrupt);
            }
        }
    }
    Ok(())
}
