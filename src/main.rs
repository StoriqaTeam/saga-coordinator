extern crate saga_coordinator_lib as lib;

fn main() {
    let config = lib::Config {
        users_addr: std::env::var("STQ_USERS_ADDR").expect("Users service address is required"),
        stores_addr: std::env::var("STQ_STORES_ADDR").expect("Stores service address is required"),
    };
    lib::start_server(config);
}
