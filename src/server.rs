use crate::common::*;
use crate::chunk::*;
use std::collections::{HashSet, HashMap};
use std::sync::mpsc::*;
use std::thread;
use std::rc::Rc;

struct Player {
    pos: Vec3,
    conn: Rc<Connection>,
    //to_send: Vec<IVec3>,
}

pub struct Server {
    chunks: HashMap<(i32,i32,i32), Chunk>,
    refs: HashMap<(i32,i32,i32), usize>,
    players: Vec<Player>,
    orders: Vec<(Vec<IVec3>, Rc<Connection>)>,
    ch: (Sender<ChunkMessage>, Receiver<ChunkMessage>),
}

impl Server {
    /// Creates and starts a chunk thread, and creates a Server
    pub fn new() -> Self {
        let (to, from_them) = channel();
        let (to_them, from) = channel();
        thread::spawn(move ||{
            ChunkThread::new(to_them, from_them).run()
        });

        Server {
            chunks: HashMap::new(),
            refs: HashMap::new(),
            players: Vec::new(),
            orders: Vec::new(),
            ch: (to, from),
        }
    }

    /// Add a player to the game
    pub fn join(&mut self, conn: Connection, pos: Vec3) {
        let new_player = Player {
            pos,
            conn: Rc::new(conn),
        };
        let (wait, load) = self.load_chunks_around(pos);
        //p.to_send.append(&mut wait);
        if wait.len() > 0 {
            self.orders.push((wait, Rc::clone(&new_player.conn)));
        }
        if load.len() > 0 {
            new_player.conn.send(Message::Chunks(load)).unwrap();
        }
        self.players.push(new_player);
    }

    /// Runs an infinite tick loop. It's infinite, start as a new thread!
    pub fn run(mut self) {
        let mut running = true;
        while running {
            let mut p = Vec::new();
            std::mem::swap(&mut p, &mut self.players);
            self.players = p.into_iter().filter_map(|mut p| {
                let mut np = p.pos;
                while let Some(m) = p.conn.recv() {
                    match m {
                        Message::PlayerMove(n_pos) => {
                            np = n_pos;
                        },
                        Message::Leave => match *p.conn {
                            Connection::Local(_,_) => { running = false; break; },
                            _ => return None,
                        },
                        _ => panic!("Hey, a client sent a message {:?}", m),
                    }
                }
                let (wait, load) = self.load_chunk_diff(p.pos, np);
                //p.to_send.append(&mut wait);
                if wait.len() > 0 {
                    self.orders.push((wait, Rc::clone(&p.conn)));
                }
                if load.len() > 0 {
                    p.conn.send(Message::Chunks(load)).unwrap();
                }
                p.pos = np;
                Some(p)
            }).collect();

            while let Ok(m) = self.ch.1.try_recv() {
                match m {
                    ChunkMessage::Chunks(x) => {
                        let locs: Vec<IVec3> = x.iter().map(|y| y.0).collect();

                        let mut to_remove = 12345678;
                        for (i, (order, conn)) in self.orders.iter().enumerate() {
                            if order == &locs {
                                for l in locs.iter() {
                                    match self.refs.get_mut(&as_tuple(*l)) {
                                        Some(x) => *x += 1, // This is indeed possible but ugly; see Todo below
                                        None    => { self.refs.insert(as_tuple(*l),1); },
                                    }
                                }
                                conn.send(Message::Chunks(x.clone())).unwrap();
                                to_remove = i; // We don't need this order anymore
                            }
                        }
                        for (loc, chunk) in x.into_iter() {
                            self.chunks.insert(as_tuple(loc), chunk); // There's a very small chance we're replacing a chunk; see Todo below
                        }
                        if to_remove != 12345678 {
                            self.orders.remove(to_remove);
                        }
                    },
                    _ => panic!("Chunk thread sent {:?}", m),
                }
            }

        }
    }

    /// Loads initial chunks around a player
    /// Returns `(chunks_to_wait_for, chunks_already_loaded)`
    /// Doesn't update `orders`
    fn load_chunks_around(&mut self, pos: Vec3) -> (Vec<IVec3>, Vec<(IVec3, Chunk)>) {
        let chunk_pos = world_to_chunk(pos);

        let mut to_load = Vec::new();

        let draw_chunks = DRAW_DIST/CHUNK_SIZE;

        for x in -CHUNK_NUM_I.x..CHUNK_NUM_I.x {
            for y in -CHUNK_NUM_I.y..CHUNK_NUM_I.y {
                for z in -CHUNK_NUM_I.z..CHUNK_NUM_I.z {
                    let p = ivec3(x,y,z);
                    if length(to_vec3(p)) <= draw_chunks {
                        to_load.push(as_tuple(chunk_pos+p));
                    }
                }
            }
        }

        let mut to_send = Vec::new();
        let mut to_pass = Vec::new();
        for p in to_load {
            let p2 = as_vec(p);
            match self.chunks.get(&p) {
                Some(chunk) => to_pass.push((p2,chunk.clone())),
                None        => to_send.push(p2),
            }
        }

        // TODO: What if the chunks we need are already being loaded, but they're not done yet so they're not in chunks?
        //  In that case, we'll need to change how we figure out who to send new chunks to as well

        self.ch.0.send(ChunkMessage::LoadChunks(to_send.clone())).unwrap();
        (to_send, to_pass)
    }

    /// Figures out what chunks need to be loaded, and either returns them or sends them to the chunk thread
    /// Returns `(chunks_to_wait_for, chunks_already_loaded)`
    /// Doesn't update `orders`
    fn load_chunk_diff(&mut self, old: Vec3, new: Vec3) -> (Vec<IVec3>, Vec<(IVec3, Chunk)>) {
        let chunk_old = world_to_chunk(old);
        let chunk_new = world_to_chunk(new);

        if chunk_old == chunk_new {
            return (Vec::new(), Vec::new());
        }

        println!("Loading chunks around {:?}", new);

        let draw_chunks = DRAW_DIST/CHUNK_SIZE;

        let mut around_old = HashSet::new();
        let mut around_new = HashSet::new();
        for x in -CHUNK_NUM_I.x..CHUNK_NUM_I.x {
            for y in -CHUNK_NUM_I.y..CHUNK_NUM_I.y {
                for z in -CHUNK_NUM_I.z..CHUNK_NUM_I.z {
                    let p = ivec3(x,y,z);
                    if length(to_vec3(p)) <= draw_chunks {
                        around_old.insert(as_tuple(chunk_old+p));
                        around_new.insert(as_tuple(chunk_new+p));
                    }
                }
            }
        }
        let to_load = &around_new - &around_old;
        let to_unload = &around_old - &around_new;

        for i in to_unload {
            if self.refs.contains_key(&i) {
                let r = {
                    // Lower the refcount on this chunk by one
                    let q = self.refs.get_mut(&i).expect("Tried to unload a chunk that isn't loaded");
                    let r = *q - 1;
                    *q = r;
                    r
                };
                // If the refcount is zero, nobody's using it so we can unload it
                if r <= 0 {
                    // TODO tell chunk thread to unload this chunk
                    self.chunks.remove(&i);
                    self.refs.remove(&i);
                }
            } else {
                println!("Tried to unload a chunk that isn't loaded: {:?}", i);
            }
        }

        let mut to_send = Vec::new();
        let mut to_pass = Vec::new();
        for p in to_load {
            let p2 = as_vec(p);
            match self.chunks.get(&p) {
                Some(chunk) => to_pass.push((p2,chunk.clone())),
                None        => to_send.push(p2),
            }
        }

        // TODO: What if the chunks we need are already being loaded, but they're not done yet so they're not in chunks?
        //  In that case, we'll need to change how we figure out who to send new chunks to as well

        self.ch.0.send(ChunkMessage::LoadChunks(to_send.clone())).unwrap();
        (to_send, to_pass)
    }
}