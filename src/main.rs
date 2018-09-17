extern crate saga_coordinator_lib as lib;
extern crate stq_logging;

fn main() {
    let config = lib::config::Config::new().expect("Failed to load service configuration. Please check your 'config' folder");

    // Prepare sentry integration
    let _sentry = lib::sentry_integration::init(config.sentry.as_ref());

    // Prepare logger
    stq_logging::init(config.graylog.as_ref());

    lib::start_server(config);
}
