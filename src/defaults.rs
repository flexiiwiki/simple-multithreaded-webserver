use crate::{HttpResponse, ServerConfig};

impl HttpResponse {
    pub fn not_found() -> Self {
        Self {
            path: String::from("404.html"),
            status: 404,
            status_text: String::from("NOT FOUND"),
            compress: true,
        }
    }
    pub fn forbidden() -> Self {
        Self {
            path: String::from("403.html"),
            status: 403,
            status_text: String::from("FORBIDDEN"),
            compress: true,
        }
    }
}

impl Default for HttpResponse {
    fn default() -> Self {
        Self {
            path: String::from("404.html"),
            status: 404,
            status_text: String::from("NOT FOUND"),
            compress: true,
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            ip_address: std::net::Ipv4Addr::new(127, 0, 0, 1),
            port: 80,
            thread_amount: 5,
            enable_gzip: true,
            not_found_response: HttpResponse::not_found(),
            forbidden_response: HttpResponse::forbidden(),
        }
    }
}
