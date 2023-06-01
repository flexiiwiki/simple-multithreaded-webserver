use simple_multithreaded_webserver::ServerConfig;

fn main() {
    let config = ServerConfig::new("192.168.0.48", 80, 5, true, None, None).unwrap();

    config.start();
}
