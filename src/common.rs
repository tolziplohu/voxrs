/// #GUIDE TO TYPES
/// In order to not confuse different spaces, we always use the same types for coordinates:
/// - `Vec3` is a position in world-space, in increments of 1 meter
/// - `IVec3` is a chunk location in world-space, in increments of 1 chunk

pub use nalgebra as na;
pub use nphysics3d as np;
pub use ncollide3d as nc;
pub use np::object::Body;
pub use nalgebra::{Vector3, Point3, Isometry3, Scalar, Unit};
use std::sync::mpsc::*;

const RD: u32 = 16;
pub const CHUNK_NUM: (u32, u32, u32) = (RD, 16, RD);
pub const CHUNK_NUM_I: (i32, i32, i32) = (CHUNK_NUM.0 as i32 / 2, CHUNK_NUM.1 as i32 / 2, CHUNK_NUM.2 as i32 / 2);

pub const CHUNK_SIZE: f32 = 16.0;
pub const DRAW_DIST: f32 = CHUNK_SIZE * RD as f32 * 0.5;

// Shorthands to match GLSL
pub type IVec3 = Vector3<i32>;
pub type Vec3 = Vector3<f32>;

pub fn radians(degrees: f32) -> f32 {
    std::f32::consts::PI / 180.0 * degrees
}

pub fn as_tuple<T: Scalar>(x: Vector3<T>) -> (T, T, T) {
    (x.x, x.y, x.z)
}
pub fn as_vec<T: Scalar>(x: (T,T,T)) -> Vector3<T> {
    Vector3::new(
        x.0,
        x.1,
        x.2,
    )
}

pub fn chunk_to_world(chunk: IVec3) -> Vec3 {
    chunk.map(|x| x as f32 + 0.5) * CHUNK_SIZE
}
pub fn world_to_chunk(world: Vec3) -> IVec3 {
    (world / CHUNK_SIZE).map(|x| x as i32)
}


//pub type Material = u16;
pub use crate::material::*;
//pub const AIR: Material = 0;
pub type Chunk = Vec<Vec<Vec<Material>>>;

pub enum Connection {
    Local(Sender<Message>, Receiver<Message>),
    // TODO some sort of buffered TCP stream inplementation of Connection
}

impl Connection {
    /// Create a two new Local connections - (client, server)
    pub fn local() -> (Connection, Connection) {
        let (cto, sfrom) = channel();
        let (sto, cfrom) = channel();
        let client = Connection::Local(cto, cfrom);
        let server = Connection::Local(sto, sfrom);
        (client, server)
    }

    /// Equivalent to Sender::send() but as an option
    pub fn send(&self, m: Message) -> Option<()> {
        match self {
            Connection::Local(to, _from) => to.send(m).ok(),
        }
    }

    /// Equivalent to Receiver::try_recv() but as an option - doesn't block
    pub fn recv(&self) -> Option<Message> {
        match self {
            Connection::Local(_to, from) => from.try_recv().ok(),
        }
    }
}

#[derive(Debug)]
pub enum Message {
    PlayerMove(Vec3),
    Chunks(Vec<(IVec3, Chunk)>),
    Leave,
}

#[derive(Debug)]
pub enum ChunkMessage {
    LoadChunks(Vec<IVec3>),
    Chunks(Vec<(IVec3, Chunk)>),
    UnloadChunk(IVec3, Chunk),
}
