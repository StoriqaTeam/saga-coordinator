extern crate saga_coordinator_lib as lib;

fn main() {
    let config = lib::config::Config::new().expect("Failed to load service configuration. Please check your 'config' folder");
    lib::start_server(config);
}
