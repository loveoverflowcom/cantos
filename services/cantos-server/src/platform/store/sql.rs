//! Statement facade over the one Server Turso owner.
//!
//! Context adapters keep their shape: a use case opens one [`Tx`] from a [`Conn`], passes it to
//! repository functions and commits it. This module is where the engine's vocabulary ends:
//! positional parameters are [`Param`]s, rows are decoded through [`FromColumn`] with the column
//! type checked (a mismatch is corruption, never a default), and every failure is the closed
//! [`Error`] type. No SQL text, row data or file path crosses it.
//!
//! What a [`Tx`] is. [`Conn::transaction`] is the only write path: `BEGIN IMMEDIATE` on the
//! owner's serialized writer with a bounded wait, so facts read inside it are decided against
//! exactly the state it then writes. That replaces PostgreSQL row locks, advisory locks and
//! `FOR UPDATE` by ordering; a conditional `UPDATE … WHERE revision = ?` still has to prove its
//! affected-row count. [`Conn::read`] is a pinned read snapshot that cannot write and never waits
//! for the writer. The writer is never held across a call that is not the database (RPC, password
//! hashing, mail or object storage): such work happens before or after the transaction.
use std::{fmt, sync::Arc, time::Duration};

use cantos_store_turso::{
    error::{CommitFault, ConstraintKind, StoreFault},
    params_from_iter, ReadConnection, Value, WriteTransaction,
};

use crate::platform::turso::{Acknowledgement, ServerDatabase};

/// How long a request may queue for the serialized writer before it is refused as unavailable.
pub(crate) const WRITER_WAIT: Duration = Duration::from_secs(5);

/// The closed failure vocabulary of a statement. It never carries SQL, rows or paths.
#[derive(Debug)]
pub enum Error {
    /// The engine refused or failed a statement; nothing in the statement took effect.
    Fault(StoreFault),
    /// A `COMMIT` failed. [`CommitFault::Unknown`] is not a rollback: the unit may be durable.
    Commit(CommitFault),
    /// A column held another type than the decoder asked for, or an index did not exist.
    Column,
    /// A write was attempted on a read snapshot.
    ReadOnly,
    /// `query_one` saw no row or more than one.
    Cardinality,
}

impl Error {
    /// The stable `cantos:<code>` identifier of a guard trigger that rejected the statement.
    pub fn guard(&self) -> Option<&str> {
        match self {
            Self::Fault(StoreFault::Guard(code)) => Some(code.as_str()),
            _ => None,
        }
    }

    pub fn constraint(&self) -> Option<ConstraintKind> {
        match self {
            Self::Fault(StoreFault::Constraint { kind, .. }) => Some(*kind),
            Self::Commit(fault) => match fault.fault() {
                StoreFault::Constraint { kind, .. } => Some(*kind),
                _ => None,
            },
            _ => None,
        }
    }

    /// A constraint or guard rejected the statement: stored data or the caller's value broke an
    /// invariant, as opposed to the database being unavailable.
    pub fn is_integrity_violation(&self) -> bool {
        self.constraint().is_some() || self.guard().is_some()
    }

    /// True when the commit may have become durable although it was not acknowledged.
    pub fn outcome_unknown(&self) -> bool {
        matches!(self, Self::Commit(CommitFault::Unknown(_)))
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fault(fault) => fault.fmt(f),
            Self::Commit(fault) => fault.fmt(f),
            Self::Column => f.write_str("a stored column has an unexpected type"),
            Self::ReadOnly => f.write_str("a read snapshot cannot write"),
            Self::Cardinality => f.write_str("the query did not return exactly one row"),
        }
    }
}

impl std::error::Error for Error {}

impl From<StoreFault> for Error {
    fn from(fault: StoreFault) -> Self {
        Self::Fault(fault)
    }
}

impl From<CommitFault> for Error {
    fn from(fault: CommitFault) -> Self {
        Self::Commit(fault)
    }
}

/// A value bound to a positional `?N` parameter.
pub trait Param {
    fn to_value(&self) -> Value;
}

/// Parameters of one statement, in `?1`, `?2`, … order.
pub type Params<'a> = &'a [&'a (dyn Param + Sync)];

impl<T: Param + ?Sized> Param for &T {
    fn to_value(&self) -> Value {
        (**self).to_value()
    }
}

impl Param for str {
    fn to_value(&self) -> Value {
        Value::Text(self.to_owned())
    }
}

impl Param for String {
    fn to_value(&self) -> Value {
        Value::Text(self.clone())
    }
}

impl Param for [u8] {
    fn to_value(&self) -> Value {
        Value::Blob(self.to_vec())
    }
}

impl Param for Vec<u8> {
    fn to_value(&self) -> Value {
        Value::Blob(self.clone())
    }
}

macro_rules! integer_param {
    ($($type:ty),*) => {$(
        impl Param for $type {
            fn to_value(&self) -> Value {
                Value::Integer(i64::from(*self))
            }
        }
    )*};
}
integer_param!(i64, i32, i16, u32, u16, u8);

impl Param for bool {
    fn to_value(&self) -> Value {
        Value::Integer(i64::from(*self))
    }
}

impl<T: Param> Param for Option<T> {
    fn to_value(&self) -> Value {
        self.as_ref().map_or(Value::Null, Param::to_value)
    }
}

/// A set of identifiers for `x IN (SELECT value FROM json_each(?N))`, replacing PostgreSQL's
/// `= ANY($N)`. Identifiers are quoted by the JSON encoder, never spliced into SQL.
impl Param for [String] {
    fn to_value(&self) -> Value {
        Value::Text(serde_json::to_string(self).unwrap_or_else(|_| "[]".to_owned()))
    }
}

impl Param for [i64] {
    fn to_value(&self) -> Value {
        Value::Text(serde_json::to_string(self).unwrap_or_else(|_| "[]".to_owned()))
    }
}

impl Param for Vec<i64> {
    fn to_value(&self) -> Value {
        self.as_slice().to_value()
    }
}

impl Param for Vec<String> {
    fn to_value(&self) -> Value {
        self.as_slice().to_value()
    }
}

/// How one column is read. A stored value of another type is [`Error::Column`].
pub trait FromColumn: Sized {
    fn from_column(value: Value) -> Result<Self, Error>;
}

impl FromColumn for String {
    fn from_column(value: Value) -> Result<Self, Error> {
        match value {
            Value::Text(text) => Ok(text),
            _ => Err(Error::Column),
        }
    }
}

impl FromColumn for Vec<u8> {
    fn from_column(value: Value) -> Result<Self, Error> {
        match value {
            Value::Blob(bytes) => Ok(bytes),
            _ => Err(Error::Column),
        }
    }
}

impl FromColumn for i64 {
    fn from_column(value: Value) -> Result<Self, Error> {
        match value {
            Value::Integer(number) => Ok(number),
            _ => Err(Error::Column),
        }
    }
}

macro_rules! narrow_column {
    ($($type:ty),*) => {$(
        impl FromColumn for $type {
            fn from_column(value: Value) -> Result<Self, Error> {
                <$type>::try_from(i64::from_column(value)?).map_err(|_| Error::Column)
            }
        }
    )*};
}
narrow_column!(i32, i16, u32, u16, u8);

/// Only `0` and `1` are booleans; anything else is a row written outside the typed path.
impl FromColumn for bool {
    fn from_column(value: Value) -> Result<Self, Error> {
        match i64::from_column(value)? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(Error::Column),
        }
    }
}

impl<T: FromColumn> FromColumn for Option<T> {
    fn from_column(value: Value) -> Result<Self, Error> {
        match value {
            Value::Null => Ok(None),
            other => T::from_column(other).map(Some),
        }
    }
}

/// A column selector: its position or its name.
pub trait RowIndex {
    fn position(&self, row: &Row) -> Option<usize>;
}

impl RowIndex for usize {
    fn position(&self, row: &Row) -> Option<usize> {
        (*self < row.values.len()).then_some(*self)
    }
}

impl RowIndex for &str {
    fn position(&self, row: &Row) -> Option<usize> {
        row.names.iter().position(|name| name == self)
    }
}

/// One fully read result row. Cursors are drained before a statement returns, so no statement is
/// ever left open on the writer.
#[derive(Clone, Debug)]
pub struct Row {
    names: Arc<[String]>,
    values: Vec<Value>,
}

impl Row {
    /// Checked access also serves the file-backed fixture's public SQL result rows.
    pub fn try_get<I: RowIndex, T: FromColumn>(&self, index: I) -> Result<T, Error> {
        let position = index.position(self).ok_or(Error::Column)?;
        T::from_column(self.values[position].clone())
    }

    /// For a column the query itself selected with a known type: a mismatch is a programming
    /// error in the adapter, not stored data, so it is not silently defaulted.
    pub(crate) fn get<I: RowIndex, T: FromColumn>(&self, index: I) -> T {
        match self.try_get(index) {
            Ok(value) => value,
            Err(_) => panic!("selected column does not have the requested type"),
        }
    }
}

enum Mode<'a> {
    Write(WriteTransaction<'a>),
    Read(ReadConnection),
}

/// A transaction borrowed by repository functions. Use cases open, pass and commit it but never
/// issue SQL through it themselves. Dropping it without [`Tx::commit`] abandons its writes; the
/// owner rolls the open transaction back before the next write begins.
pub(crate) struct Tx<'a> {
    mode: Mode<'a>,
    now: i64,
    acknowledgement: Acknowledgement,
}

async fn collect(mut rows: cantos_store_turso::Rows) -> Result<Vec<Row>, Error> {
    let names: Arc<[String]> = rows.column_names().into();
    let mut out = Vec::new();
    while let Some(row) = rows.next().await? {
        let mut values = Vec::with_capacity(names.len());
        for index in 0..names.len() {
            values.push(row.get_value(index).map_err(|_| Error::Column)?);
        }
        out.push(Row {
            names: names.clone(),
            values,
        });
    }
    Ok(out)
}

fn bind(params: Params<'_>) -> impl cantos_store_turso::IntoParams {
    let values: Vec<Value> = params.iter().map(|param| param.to_value()).collect();
    params_from_iter(values)
}

impl Tx<'_> {
    /// The trusted time of this unit in microseconds since the Unix epoch. A write unit takes it
    /// after the writer is held, so timestamps follow commit order; SQL never reads the clock.
    pub(crate) fn now_micros(&self) -> i64 {
        self.now
    }

    /// Whole Unix seconds, the unit of `SessionPolicy` deadlines (a checked floor division).
    pub(crate) fn now_seconds(&self) -> i64 {
        self.now.div_euclid(1_000_000)
    }

    /// The engine's own cursor, for an adapter that decodes through its production types. Drain
    /// or drop it before the next statement on a write unit.
    pub(crate) async fn raw_query(
        &self,
        sql: &str,
        params: impl cantos_store_turso::IntoParams,
    ) -> Result<cantos_store_turso::Rows, Error> {
        Ok(match &self.mode {
            Mode::Write(tx) => tx.query(sql, params).await?,
            Mode::Read(read) => read.query(sql, params).await?,
        })
    }

    pub(crate) async fn query(&self, sql: &str, params: Params<'_>) -> Result<Vec<Row>, Error> {
        match &self.mode {
            Mode::Write(tx) => collect(tx.query(sql, bind(params)).await?).await,
            Mode::Read(read) => collect(read.query(sql, bind(params)).await?).await,
        }
    }

    pub(crate) async fn query_opt(
        &self,
        sql: &str,
        params: Params<'_>,
    ) -> Result<Option<Row>, Error> {
        let mut rows = self.query(sql, params).await?;
        match rows.len() {
            0 => Ok(None),
            1 => Ok(rows.pop()),
            _ => Err(Error::Cardinality),
        }
    }

    pub(crate) async fn query_one(&self, sql: &str, params: Params<'_>) -> Result<Row, Error> {
        self.query_opt(sql, params).await?.ok_or(Error::Cardinality)
    }

    /// The number of rows the statement changed. A caller that needs exactly one proves it.
    pub(crate) async fn execute(&self, sql: &str, params: Params<'_>) -> Result<u64, Error> {
        match &self.mode {
            Mode::Write(tx) => Ok(tx.execute(sql, bind(params)).await?),
            Mode::Read(_) => Err(Error::ReadOnly),
        }
    }

    pub(crate) async fn commit(self) -> Result<(), Error> {
        match self.mode {
            Mode::Write(tx) => Ok(self.acknowledgement.commit(tx).await?),
            Mode::Read(read) => Ok(read.end_snapshot().await?),
        }
    }

    pub(crate) async fn rollback(self) -> Result<(), Error> {
        match self.mode {
            Mode::Write(tx) => Ok(tx.rollback().await?),
            Mode::Read(read) => Ok(read.end_snapshot().await?),
        }
    }
}

/// The handle a use case obtains per operation. It owns no engine resource until a transaction
/// begins.
pub(crate) struct Conn {
    database: ServerDatabase,
    acknowledgement: Acknowledgement,
}

impl Conn {
    pub(crate) fn new(database: ServerDatabase, acknowledgement: Acknowledgement) -> Self {
        Self {
            database,
            acknowledgement,
        }
    }

    /// Begin the only kind of mutation. Waits at most [`WRITER_WAIT`] for the serialized writer.
    pub(crate) async fn transaction(&mut self) -> Result<Tx<'_>, Error> {
        let tx = self
            .database
            .owner()
            .begin_write_within(WRITER_WAIT)
            .await?;
        Ok(Tx {
            now: tx.now().get(),
            mode: Mode::Write(tx),
            acknowledgement: self.acknowledgement,
        })
    }

    /// A pinned read snapshot: every statement sees the same committed state, and it never waits
    /// for or blocks the writer.
    pub(crate) async fn read(&self) -> Result<Tx<'static>, Error> {
        let now = self.database.now().map_err(|_| StoreFault::Clock)?.get();
        let reader = self.database.owner().reader().await?;
        reader.begin_snapshot().await?;
        Ok(Tx {
            mode: Mode::Read(reader),
            now,
            acknowledgement: self.acknowledgement,
        })
    }

    /// A single read statement outside any pinned snapshot.
    pub(crate) async fn query_opt(
        &self,
        sql: &str,
        params: Params<'_>,
    ) -> Result<Option<Row>, Error> {
        let tx = self.read().await?;
        let row = tx.query_opt(sql, params).await;
        tx.rollback().await?;
        row
    }

    /// One auto-committed write statement.
    pub(crate) async fn execute(&mut self, sql: &str, params: Params<'_>) -> Result<u64, Error> {
        let tx = self.transaction().await?;
        match tx.execute(sql, params).await {
            Ok(changed) => {
                tx.commit().await?;
                Ok(changed)
            }
            Err(error) => {
                let _ = tx.rollback().await;
                Err(error)
            }
        }
    }
}
