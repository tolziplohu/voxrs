// This is the client auxilary thread, which is in charge of recieving chunks, meshing them, and sending them to the client thread.

use crate::common::*;
use crate::mesh::Vertex;
use crate::mesh::*;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::mpsc::*;
use std::sync::{Arc, RwLock};

pub type ClientMessage = Vec<(
    IVec3,
    Vec<Vertex>,
    Vec<Vertex>, // Phase2
    Option<nc::shape::ShapeHandle<f32>>,
    Arc<RwLock<Chunk>>,
)>;

pub fn client_aux_thread(
    server: Connection,
    // A None ClientMessage is a response to Message::Leave, saying we're ready to go.
    client: (Sender<Option<ClientMessage>>, Receiver<Message>),
    mut player: Vec3,
    config: Arc<ClientConfig>,
) {
    // This is a timer for sending player movement to the server. We don't want to do it too often, just around 20 times per second.
    // So, we only send it when this timer is past 50ms
    let mut timer = stopwatch::Stopwatch::start_new();

    let mut chunk_map: HashMap<IVec3, Arc<RwLock<Chunk>>> = HashMap::new();
    let mut indices: Vec<IVec3> = Vec::new();
    let mut counter = 0;

    loop {
        if let Ok(mut m) = client.1.try_recv() {
            loop {
                match m {
                    Message::PlayerMove(p) => player = p,
                    Message::SetBlock(p, b) => {
                        server
                            .send(Message::SetBlock(p, b))
                            .expect("Disconnected from server");
                    }
                    Message::Leave => {
                        server
                            .send(Message::Leave)
                            .expect("Disconnected from server");
                        loop {
                            if let Some(Message::Leave) = server.recv() {
                                break;
                            }
                        }
                        return;
                    }
                    x => panic!("Aux thread recieved {:?} from the client thread!", x),
                }
                if let Ok(q) = client.1.try_recv() {
                    m = q;
                } else {
                    break;
                }
            }
        } else {
            // Sync up with the client; we don't want to send more than one batch per frame
            continue;
        }
        if timer.elapsed_ms() > 50 {
            server
                .send(Message::PlayerMove(player))
                .expect("Disconnected from server");
            timer.restart();
        }

        if !indices.is_empty() && counter >= 10 {
            let c = world_to_chunk(player);
            // println!("Rechunking");
            // let timer = stopwatch::Stopwatch::start_new();

            indices.sort_by_key(|x| ((chunk_to_world(*x) - player).norm() * 10.0) as i32);
            while !indices.is_empty()
                && (indices.last().unwrap() - c).map(|x| x as f32).norm()
                    > config.game_config.draw_chunks as f32
            {
                let r = indices.pop().unwrap();
                chunk_map.remove(&r);
            }

            // println!("Rechunking took {} ms", timer.elapsed_ms());

            counter = 0;
        }

        if !indices.is_empty() {
            // let timer = stopwatch::Stopwatch::start_new();
            let meshed: Vec<_> = indices
                .iter()
                .take(config.batch_size.min(indices.len()))
                .cloned()
                .map(|loc| (loc, chunk_map.get(&loc).unwrap()))
                .filter_map(|(loc, chunk)| {
                    let neighbors = neighbors(loc).into_iter().map(|x| chunk_map.get(&x));
                    if neighbors.clone().all(|x| x.is_some()) {
                        let neighbors: Vec<Arc<RwLock<Chunk>>> =
                            neighbors.map(|x| x.unwrap()).cloned().collect();
                        Some((loc, chunk, neighbors))
                    } else {
                        None
                    }
                })
                .map(|(loc, chunk, neighbors)| {
                    (
                        loc,
                        config
                            .mesher
                            .mesh(&chunk.read().unwrap(), neighbors.clone(), false),
                        config.mesher.mesh(&chunk.read().unwrap(), neighbors, true),
                        Arc::clone(&chunk),
                    )
                })
                .map(|(loc, mesh, mesh_p2, chunk)| {
                    if !mesh.is_empty() {
                        let v_physics: Vec<_> =
                            mesh.iter().map(|x| na::Point3::from(x.pos)).collect();
                        let i_physics: Vec<_> = (0..v_physics.len() / 3)
                            .map(|x| na::Point3::new(x * 3, x * 3 + 1, x * 3 + 2))
                            .collect();
                        let chunk_shape = nc::shape::ShapeHandle::new(nc::shape::TriMesh::new(
                            v_physics, i_physics, None,
                        ));
                        (loc, mesh, mesh_p2, Some(chunk_shape), chunk)
                    } else {
                        (loc, mesh, mesh_p2, None, chunk)
                    }
                })
                .collect();
            let r = meshed.iter().map(|x| x.0).collect::<HashSet<_>>();
            indices.retain(|x| !r.contains(x));
            client.0.send(Some(meshed)).unwrap();
            // println!("Meshing took {} ms/chunk", timer.elapsed_ms() as f64 / r.len() as f64);
            counter += 1;
        }
        if let Some(m) = server.recv() {
            match m {
                Message::Chunks(chunks) => {
                    /*
                    println!(
                        "Requested load of {} chunks: \n{:?}",
                        chunks.len(),
                        chunks.iter().map(|x| x.0).collect::<Vec<IVec3>>()
                    );
                    */
                    indices.extend(chunks.iter().map(|x| x.0));
                    chunk_map.extend(
                        chunks
                            .into_iter()
                            .map(|(x, y)| (x, Arc::new(RwLock::new(y)))),
                    );
                    counter = 100; // Trigger a re-sort
                }
                _ => (),
            }
        }
    }
}
