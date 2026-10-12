//! Transactional synthesis use cases over the concrete PostgreSQL authority.
//! Decisions use the pure domain; persistence primitives never start another transaction.
mod accept;
mod control;
mod dispatch;
mod enqueue;
mod prepare;
mod reconcile;
mod settle;
mod status;
