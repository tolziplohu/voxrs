#[macro_use]
extern crate glium;
extern crate glm;
extern crate glsl_include;
extern crate num_traits;
#[macro_use]
extern crate num_derive;

use glium::glutin;
use glium::Surface;
use glm::*;
use glsl_include::Context as ShaderContext;
use num_traits::identities::*;

mod common;
use common::*;
mod chunk;
mod client;
mod input;
mod mesh;
mod server;
mod terrain;

use client::*;
use server::*;

/// Load a shader, replacing any ``#include` declarations to files in `includes`
fn shader(path: String, includes: &[String]) -> String {
    use std::fs::File;
    use std::io::Read;
    let mut file = File::open("src/".to_owned() + &path).unwrap();
    let mut string = String::new();
    file.read_to_string(&mut string).unwrap();
    let mut c = ShaderContext::new();
    for i in includes {
        let mut file = File::open("src/".to_owned() + &i).unwrap();
        let mut string = String::new();
        file.read_to_string(&mut string).unwrap();
        c.include(i.clone(), string);
    }
    c.expand(string).unwrap()
}

fn main() {
    // Wayland doesn't allow cursor grabbing
    let mut events_loop = glutin::os::unix::EventsLoopExt::new_x11().unwrap();
    let wb = glutin::WindowBuilder::new().with_title("Vox.rs 2");
    let cb = glutin::ContextBuilder::new().with_depth_buffer(24);
    let display = glium::Display::new(wb, cb, &events_loop).unwrap();

    let mut camera_pos = vec3(4.0, 16.0, 4.0);

    let (conn_client, conn_server) = Connection::local();
    let mut client = Client::new(display, events_loop, conn_client, camera_pos);
    std::thread::spawn(move || {
        let mut server = Server::new();
        server.join(conn_server, camera_pos);
        server.run();
    });

    let mut timer = stopwatch::Stopwatch::start_new();

    let mut open = true;
    while open {
        let delta = timer.elapsed_ms() as f64 / 1000.0;
        // println!("{:.1} FPS", 1.0 / delta);
        timer.restart();

        let mut target = client.display().draw();

        client.draw(&mut target);

        // Most computation should go after this point, while the GPU is rendering

        open = client.update(delta);
        //camera_pos = client.pos();

        target.finish().unwrap();
    }
}
