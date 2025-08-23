use std::rc::Rc;
use std::path::Path;

use once_cell::sync::OnceCell;
use slint::{Image, SharedString, VecModel};

use crate::{Drone, Client, Server};

static LOGGER: OnceCell<Box<dyn Fn(SharedString) + Send + Sync + 'static>> = OnceCell::new();

// Register the logger callback (called from main.rs).
pub fn set_logger(cb: Box<dyn Fn(SharedString) + Send + Sync + 'static>) {
    let _ = LOGGER.set(cb);
}

//TODO log events
pub fn log<S: Into<SharedString>>(msg: S) {
    if let Some(cb) = LOGGER.get() {
        cb(msg.into());
    }
}

pub fn validate_node_id(input: &str) -> bool {

    // Try to parse as a number
    if let Ok(val) = input.parse::<isize>() {

        //TODO check if node exists

        //TODO add sender

        return true;
    }

    return false;
}

pub fn validate_pdr(input: &str, current: &str) -> String {

    // Try to parse as a number
    if let Ok(val) = input.parse::<f64>() {

        // Range check
        if val >= 0.0 && val <= 100.0 {

            // If different from current
            if input != current {

                //TODO set PDR

                return input.to_string();
            }

        }
    }

    // Fallback: keep current value
    current.to_string()
}

pub fn remove_node(input: &str) -> () {

    // Try to parse as a number
    if let Ok(val) = input.parse::<isize>() {

        //TODO check if node exists

        //TODO remove node
    }
} 

pub fn crash_node(input: &str) -> () {

    // Try to parse as a number
    if let Ok(val) = input.parse::<isize>() {

        //TODO check if node exists

        //TODO crash node
    }
} 

pub fn update_graph(
    drones: Rc<VecModel<Drone>>,
    clients: Rc<VecModel<Client>>,
    servers: Rc<VecModel<Server>>) -> Image {
    
    //TODO replace this with the actual graph
    Image::load_from_path(Path::new("assets/images/placeholder.png")).unwrap_or_default()
}
