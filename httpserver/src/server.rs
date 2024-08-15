//server.rs
use super::router::Router;
use std::collections::HashMap;
use std::io::{self, prelude::*, BufReader};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};

use crate::config::{ok_count_redirect, ok_same_port, Config, ConfigServer};
use crate::handler::PageErrorHandler;
use http::httprequest::{
    process_header_line, process_req_line, HttpRequest, Method, Resource, Version,
};
use libc::{epoll_create1, epoll_ctl, epoll_event, epoll_wait, EPOLLIN, EPOLL_CTL_ADD};
use std::os::unix::io::AsRawFd;
use std::time::Duration;

pub struct Server {
    config: Config,
}

impl Server {
    pub fn new(config: Config) -> Self {
        Server { config }
    }

    pub fn run(&self) {
        let epoll_fd = unsafe { epoll_create1(0) };

        let mut listeners = vec![];

        // Créer des instances de serveurs pour chaque configuration
        for server_config in &self.config.servers {
            if !ok_count_redirect(&server_config.routes) || !ok_same_port(server_config) {
                eprintln!("⚠️ Incorrect configuration {}⚠️", server_config.host_name);
                continue;
            }
            for port in &server_config.ports {
                let addr = format!("{}:{}", &server_config.host, port);
                let listener = TcpListener::bind(addr).expect("error to bind addr");
                listener
                    .set_nonblocking(true)
                    .expect("error to set true non blocking");
                println!("Server running on http://{}:{}", server_config.host, *port);

                // Ajouter le listener à epoll sans EPOLLET (Level-Triggered par défaut)
                let mut event = epoll_event {
                    events: EPOLLIN as u32,
                    u64: listener.as_raw_fd() as u64,
                };
                unsafe {
                    epoll_ctl(epoll_fd, EPOLL_CTL_ADD, listener.as_raw_fd(), &mut event);
                }
                listeners.push(listener);
            }
        }

        let mut events = vec![epoll_event { events: 0, u64: 0 }; 10];

        loop {
            let nfds =
                unsafe { epoll_wait(epoll_fd, events.as_mut_ptr(), events.len() as i32, -1) };
            if nfds == -1 {
                eprintln!("epoll_wait failed");
                continue;
            }

            for n in 0..nfds {
                for (_i, listener) in listeners.iter().enumerate() {
                    if events[n as usize].u64 == (listener.as_raw_fd() as u64) {
                        match listener.accept() {
                            Ok((stream, _)) => {
                                if let Some(addr) = listener.local_addr().ok() {
                                    let (config, addrs) = get_server(&self.config, addr);
                                    handle_client(stream, &config, &addrs);
                                }
                            }
                            Err(e) => {
                                eprintln!("Error accepting connection: {:?}", e);
                            }
                        }
                    }
                }
            }
        }
    }
}

fn get_server(config: &Config, addr: SocketAddr) -> (ConfigServer, String) {
    let host = addr.to_string();
    let ip = host.split(":").collect::<Vec<&str>>()[0];
    for server in config.servers.clone() {
        if server.host == ip {
            return (server, host);
        }
    }
    (
        ConfigServer {
            host_name: String::new(),
            host: String::new(),
            ports: Vec::new(),
            root: String::new(),
            error_pages: HashMap::new(),
            client_body_limit: 0,
            routes: Vec::new(),
            cgi_extensions: HashMap::new(),
            directory_listing: false,
        },
        host,
    )
}

fn handle_client(mut stream: TcpStream, config: &ConfigServer, addr: &str) {
    stream.set_read_timeout(Some(Duration::new(10, 0))).expect("faild to set_read_timeout");
    stream
        .set_write_timeout(Some(Duration::new(10, 0)))
        .expect("faild to set_write_timeout");

    println!("Connection established");

    let mut parsed_headers: HashMap<String, String> = HashMap::new();
    let mut parsed_method = Method::Uninitialized;
    let mut parsed_version = Version::V1_1;
    let mut parsed_resource = Resource::Path("".to_string());
    let mut parsed_msg_body = Vec::new();

    let mut buff = BufReader::new(&stream);
    let mut read_buffer = String::new();

    // Lire les en-têtes
    loop {
        match buff.read_line(&mut read_buffer) {
            Ok(bytes_read) => {
                if bytes_read == 0 {
                    break;
                }
                if read_buffer.trim().is_empty() {
                    // Fin des en-têtes
                    break;
                }
                if read_buffer.contains("HTTP") {
                    let (method, resource, version) = process_req_line(&read_buffer);
                    parsed_method = method;
                    parsed_version = version;
                    parsed_resource = resource;
                } else if read_buffer.contains(":") {
                    let (key, value) = process_header_line(&read_buffer);
                    parsed_headers.insert(key, value);
                }
                read_buffer.clear();
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                eprintln!("Timeout reading from stream");
                let response = PageErrorHandler::error_response(config, "408");
                response.send_response(&mut stream).expect("faild to send_response");
                // Fermeture de la connexion
                match stream.shutdown(Shutdown::Both) {
                    Ok(_) => {}
                    Err(ref e) if e.kind() == io::ErrorKind::NotConnected => {
                        // Ignorer cette erreur spécifique
                        eprintln!("Stream not connected: {:?}", e);
                    }
                    Err(e) => {
                        // Gérer les autres erreurs
                        eprintln!("Shutdown failed: {:?}", e);
                    }
                }
                return;
            }
            Err(e) => {
                eprintln!("Error reading from stream: {:?}", e);
                // Fermeture de la connexion
                match stream.shutdown(Shutdown::Both) {
                    Ok(_) => {}
                    Err(ref e) if e.kind() == io::ErrorKind::NotConnected => {
                        // Ignorer cette erreur spécifique
                        eprintln!("Stream not connected: {:?}", e);
                    }
                    Err(e) => {
                        // Gérer les autres erreurs
                        eprintln!("Shutdown failed: {:?}", e);
                    }
                }
                return;
            }
        }
    }

    // Lire le corps de la requête
    if let Some(content_length_str) = parsed_headers.get("Content-Length") {
        if let Ok(content_length) = content_length_str.trim().parse::<usize>() {
            // Vérifier si le corps de la requête dépasse la limite définie dans la configuration
            if content_length > config.client_body_limit {
                eprintln!("Request body exceeds limit");
                let response = PageErrorHandler::error_response(config, "413");
                response.send_response(&mut stream).expect("faild to send_response");
                // Fermeture de la connexion
                match stream.shutdown(Shutdown::Both) {
                    Ok(_) => {}
                    Err(ref e) if e.kind() == io::ErrorKind::NotConnected => {
                        // Ignorer cette erreur spécifique
                        eprintln!("Stream not connected: {:?}", e);
                    }
                    Err(e) => {
                        // Gérer les autres erreurs
                        eprintln!("Shutdown failed: {:?}", e);
                    }
                }
                return;
            }
            let mut total_bytes_read = 0;
            while total_bytes_read < content_length {
                let mut buffer = vec![0; content_length];
                match buff.read(&mut buffer) {
                    Ok(bytes_read) => {
                        if bytes_read == 0 {
                            break;
                        }
                        total_bytes_read += bytes_read;
                        parsed_msg_body.extend_from_slice(&buffer[..bytes_read]);
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        eprintln!("Timeout reading from stream");
                        let response = PageErrorHandler::error_response(config, "408");
                        response.send_response(&mut stream).expect("faild to send_response");
                        // Fermeture de la connexion
                        match stream.shutdown(Shutdown::Both) {
                            Ok(_) => {}
                            Err(ref e) if e.kind() == io::ErrorKind::NotConnected => {
                                // Ignorer cette erreur spécifique
                                eprintln!("Stream not connected: {:?}", e);
                            }
                            Err(e) => {
                                // Gérer les autres erreurs
                                eprintln!("Shutdown failed: {:?}", e);
                            }
                        }
                        return;
                    }
                    Err(e) => {
                        eprintln!("Error reading from stream: {:?}", e);
                        // Fermeture de la connexion
                        match stream.shutdown(Shutdown::Both) {
                            Ok(_) => {}
                            Err(ref e) if e.kind() == io::ErrorKind::NotConnected => {
                                // Ignorer cette erreur spécifique
                                eprintln!("Stream not connected: {:?}", e);
                            }
                            Err(e) => {
                                // Gérer les autres erreurs
                                eprintln!("Shutdown failed: {:?}", e);
                            }
                        }
                        return;
                    }
                }
            }

            // Vérifier si le corps de la requête est complet
            if total_bytes_read < content_length {
                eprintln!("unexpected end of request body");
                let response = PageErrorHandler::error_response(config, "400");
                response.send_response(&mut stream).expect("faild to send_response");
                // Fermeture de la connexion
                match stream.shutdown(Shutdown::Both) {
                    Ok(_) => {}
                    Err(ref e) if e.kind() == io::ErrorKind::NotConnected => {
                        // Ignorer cette erreur spécifique
                        eprintln!("Stream not connected: {:?}", e);
                    }
                    Err(e) => {
                        // Gérer les autres erreurs
                        eprintln!("Shutdown failed: {:?}", e);
                    }
                }
                return;
            }
        } else {
            eprintln!("Invalid Content-Length header");
            let response = PageErrorHandler::error_response(config, "400");
            response.send_response(&mut stream).expect("faild to send_response");
            // Fermeture de la connexion
            match stream.shutdown(Shutdown::Both) {
                Ok(_) => {}
                Err(ref e) if e.kind() == io::ErrorKind::NotConnected => {
                    // Ignorer cette erreur spécifique
                    eprintln!("Stream not connected: {:?}", e);
                }
                Err(e) => {
                    // Gérer les autres erreurs
                    eprintln!("Shutdown failed: {:?}", e);
                }
            }
            return;
        }
    }

    let req = HttpRequest::new(
        parsed_method,
        parsed_version,
        parsed_resource,
        parsed_headers,
        String::from_utf8_lossy(&parsed_msg_body).into_owned(), // Convertir le corps de la requête en String
    );
    Router::route(req, &mut stream, config, parsed_msg_body, addr);
    // Fermeture de la connexion
    match stream.shutdown(Shutdown::Both) {
        Ok(_) => {}
        Err(ref e) if e.kind() == io::ErrorKind::NotConnected => {
            // Ignorer cette erreur spécifique
            eprintln!("Stream not connected: {:?}", e);
        }
        Err(e) => {
            // Gérer les autres erreurs
            eprintln!("Shutdown failed: {:?}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::config::Route;

    use super::*;
    use std::net::{SocketAddr, TcpStream};
    use std::io::Cursor;
    use std::thread;
    use std::time::Duration;

    fn setup_config() -> Config {
        Config {
            servers: vec![ConfigServer {
                host_name: String::from("localhost"),
                host: String::from("127.0.0.1"),
                ports: vec![8080],
                root: String::from("/public"),
                error_pages: HashMap::new(),
                client_body_limit: 1024,
                routes: vec![Route {
                    alias: "/test".to_string(),
                    pages: vec!["index.html".to_string()],
                    default_page: "index.html".to_string(),
                    check_cookie: false,
                    redirect: None,
                    links: vec!["/index.html".to_string()],
                    methods: vec!["GET".to_string(), "POST".to_string()],
                }],
                cgi_extensions: HashMap::new(),
                directory_listing: false,
            }],
        }
    }

    #[test]
    fn test_get_server() {
        let config = setup_config();
        let addr: SocketAddr = "127.0.0.1:8080".parse().expect("Invalid address");
        let (server_config, _) = get_server(&config, addr);

        assert_eq!(server_config.host, "127.0.0.1");
        assert_eq!(server_config.ports, vec![8080]);
    }

    #[test]
    fn test_handle_client_get_request() {
        let config = setup_config().servers[0].clone();
        let addr = "127.0.0.1:8080".to_string();

        let mut stream = Cursor::new(Vec::new());
        let request = b"GET /test/index.html HTTP/1.1\r\nHost: localhost\r\n\r\n";

        stream.write_all(request).unwrap();
        stream.set_position(0);

        let tcp_stream = TcpStream::connect("127.0.0.1:8080").unwrap();
        handle_client(tcp_stream, &config, &addr);

        stream.set_position(0);
        let response = String::from_utf8(stream.into_inner()).expect("Response not valid UTF-8");
        assert!(response.contains("200 OK"));
    }

    #[test]
    fn test_handle_client_post_request_exceeds_body_limit() {
        let config = setup_config().servers[0].clone();
        let addr = "127.0.0.1:8080".to_string();

        let mut stream = Cursor::new(Vec::new());
        let request = format!(
            "POST /test/ HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n{}",
            config.client_body_limit + 1,
            "a".repeat(config.client_body_limit + 1)
        );

        stream.write_all(request.as_bytes()).unwrap();
        stream.set_position(0);

        let tcp_stream = TcpStream::connect("127.0.0.1:8080").unwrap();
        handle_client(tcp_stream, &config, &addr);

        stream.set_position(0);
        let response = String::from_utf8(stream.into_inner()).expect("Response not valid UTF-8");
        assert!(response.contains("413 Payload Too Large"));
    }

    #[test]
    fn test_handle_client_timeout() {
        let config = setup_config().servers[0].clone();
        let addr = "127.0.0.1:8080".to_string();

        let mut stream = Cursor::new(Vec::new());
        let request = b"GET /test/index.html HTTP/1.1\r\nHost: localhost\r\n\r\n";

        stream.write_all(request).unwrap();
        stream.set_position(0);

        let tcp_stream = TcpStream::connect("127.0.0.1:8080").unwrap();
        tcp_stream
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        handle_client(tcp_stream, &config, &addr);

        stream.set_position(0);
        let response = String::from_utf8(stream.into_inner()).expect("Response not valid UTF-8");
        assert!(response.contains("408 Request Timeout"));
    }

    #[test]
    fn test_server_run() {
        let config = setup_config();
        let server = Server::new(config);

        // Démarrer le serveur dans un thread séparé
        thread::spawn(move || {
            server.run();
        });

        thread::sleep(Duration::from_secs(1)); // Attendre le démarrage du serveur

        // Vérifier que le serveur répond correctement à une requête
        let mut stream = TcpStream::connect("127.0.0.1:8080").expect("Failed to connect to server");
        let request = b"GET /test/index.html HTTP/1.1\r\nHost: localhost\r\n\r\n";
        stream.write_all(request).expect("Failed to send request");

        let mut response = String::new();
        stream
            .read_to_string(&mut response)
            .expect("Failed to read response");

        assert!(response.contains("200 OK"));
    }
}
