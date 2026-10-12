//! Synthetic catalog read models. They convey no publication or media-delivery authority.
use super::ids::WorkId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Genre { All, Fantasy, Memory, Dream }
#[derive(Clone, Copy, Debug)]
pub struct Work {
    pub id: WorkId, pub title: &'static str, pub genre: Genre,
    pub synopsis: &'static str, pub episodes: &'static [Episode],
}
#[derive(Clone, Copy, Debug)]
pub struct Episode { pub title: &'static str, pub duration: u32, pub available: bool }
