// router.rs
use super::handler::{Handler, PageErrorHandler, StaticPageHandler};
use crate::config::*;
use http::httprequest::Resource;
use http::{httprequest, httprequest::HttpRequest, httpresponse::HttpResponse};
use std::io::prelude::*;

pub struct Router;

impl Router {
    pub fn route(
        req: HttpRequest,
        stream: &mut impl Write,
        config: &ConfigServer,
        parsed_msg_body: Vec<u8>,
        addr: &str,
    ) -> () {
        match req.method {
            httprequest::Method::Get => {
                Self::handle_get(req, stream, config);
            }
            httprequest::Method::Post => {
                Self::handle_post(req, stream, config, parsed_msg_body, addr);
            }
            httprequest::Method::Delete => {
                Self::handle_delete(req, stream, config);
            }
            _ => {
                let _ = PageErrorHandler::new("405")
                    .handle(&req, config)
                    .send_response(stream);
            }
        }
    }

    fn handle_get(req: HttpRequest, stream: &mut impl Write, config: &ConfigServer) {
        match &req.resource {
            httprequest::Resource::Path(s) => {
                // Parse the URI
                let route: Vec<&str> = s.split("/").collect();
                let path = route[1];

                if path.is_empty() {
                    let resp = StaticPageHandler.handle(&req, config);
                    let _ = resp.send_response(stream);
                    return;
                }

                let resp: HttpResponse = StaticPageHandler.handle(&req, config);
                resp.send_response(stream)
                    .expect("msg: faild to serve static file");
            }
        }
    }

    fn handle_post(
        req: HttpRequest,
        stream: &mut impl Write,
        config: &ConfigServer,
        parsed_msg_body: Vec<u8>,
        addr: &str,
    ) {
        // verifier si la methode sur la route
        let Resource::Path(url) = &req.resource;
        if !url.ends_with("/") {
            respond_with_error(stream, config, "404");
            return;
        }
        // Parse the URI
        let route: Vec<&str> = url.split("/").collect();

        let mut alias = String::new();

        if route[0] == "" && route.len() == 2 {
            alias.push_str("/");
        } else {
            alias = format!("/{}/", route[1]);
        }
        let method = format!("{:?}", req.method);

        let is_match = find_route(config, &alias).0;
        if is_match {
            if !check_methods(config, &method.to_ascii_uppercase(), &alias) {
                respond_with_error(stream, config, "405");
                return;
            }
        } else {
            respond_with_error(stream, config, "404");
            return;
        }

        // Implement something if you want to render any page after deleting

        if let Some(content_type) = req.headers.get("Content-Type") {
            if content_type.trim_start().starts_with("multipart/form-data") {
                if let Some(boundary) = content_type.split("boundary=").nth(1) {
                    let parts = parse_multipart_body(&parsed_msg_body, boundary);

                    if parts.is_empty() {
                        respond_with_error(stream, config, "400");
                        return;
                    }

                    // Vérifier la présence du cookie avant de traiter les champs
                    let cookie_present = req.headers.get("Cookie").is_some();
                    let Resource::Path(s) = &req.resource;
                    if !cookie_present {
                        println!("set cookie no present s: {}", s);
                        if s == "/" {
                            println!("Setting cookie...");
                            set_cookie(&req, stream, "secret".to_string(), config);
                        } else {
                            respond_with_redirect(stream, addr, "/login/");
                            return;
                        }
                    }

                    for (key, (field_name, data)) in parts.iter() {
                        if key == "filename" {
                            handle_file_upload(req, stream, config, addr, field_name, data);
                            return;
                        } else {
                            handle_text_field(&req, stream, config, addr, key, field_name, data);
                        }
                    }
                } else {
                    eprintln!("Boundary not specified in Content-Type");
                    respond_with_error(stream, config, "400");
                }
            } else {
                eprintln!("Unsupported content type:{}", content_type);
                respond_with_error(stream, config, "400");
            }
        } else {
            respond_with_error(stream, config, "400");
        }
    }

    fn handle_delete(req: HttpRequest, stream: &mut impl Write, config: &ConfigServer) {
        match &req.resource {
            httprequest::Resource::Path(s) => {
                // Parse the URI
                let route: Vec<&str> = s.split("/").collect();
                let path = route[1];
                let mut alias = String::new();

                if route[0] == "" && route.len() == 2 {
                    alias.push_str("/");
                } else {
                    alias = format!("/{}/", route[1]);
                }
                let method = format!("{:?}", req.method);

                if path.is_empty() {
                    respond_with_error(stream, config, "400");
                }

                let (is_match, route) = find_route(config, &alias);
                if is_match {
                    if !check_methods(config, &method.to_ascii_uppercase(), &alias) {
                        respond_with_error(stream, config, "405");
                        return;
                    }
                    handle_get_request(req, alias, stream, config, route);
                }
            }
        }
    }
}

fn handle_file_upload(
    req: HttpRequest,
    stream: &mut impl Write,
    config: &ConfigServer,
    addr: &str,
    field_name: &str,
    data: &[u8],
) {
    if let Some(cookie) = req.headers.get("Cookie") {
        if !verify_cookie(cookie) {
            eprintln!("invalid session cookie: {}", cookie);
            respond_with_error(stream, config, "403");
            return;
        }
        if let Err(e) = save_file(&config.root, field_name, data) {
            eprintln!("Error creating file: {}", e);
            respond_with_error(stream, config, "500");
            return;
        }
        respond_with_redirect(stream, addr, "/");
    } else {
        respond_with_redirect(stream, addr, "/login/");
    }
}

fn handle_text_field(
    req: &HttpRequest,
    stream: &mut impl Write,
    _config: &ConfigServer,
    addr: &str,
    key: &str,
    field_name: &str,
    data: &[u8],
) {
    println!("key: {}", key);
    println!("field_name: {}", field_name);
    println!("Value: {}", String::from_utf8_lossy(data));

    let Resource::Path(s) = &req.resource;
    if s == "/" {
        if req.headers.get("Cookie").is_some() {
            respond_with_redirect(stream, addr, "/singin/cookie.html");
        }
    }
}

pub fn respond_with_error(stream: &mut impl Write, config: &ConfigServer, status_code: &str) {
    let response = PageErrorHandler::error_response(config, status_code);
    response
        .send_response(stream)
        .expect("faild to send_response");
}

fn respond_with_redirect(stream: &mut impl Write, addr: &str, location: &str) {
    let response = format!(
        "HTTP/1.1 301 Found\r\nLocation: http://{}{}\r\n\r\n",
        addr, location
    );
    stream
        .write(response.as_bytes())
        .expect("faild to write response");
    stream.flush().expect("faild to send_response");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::io::Cursor;

    fn setup_config() -> ConfigServer {
        ConfigServer {
            host_name: String::from("localhost"),
            root: String::from("/public"),
            directory_listing: false,
            error_pages: HashMap::new(),
            cgi_extensions: HashMap::new(),
            host: String::from("127.0.0.1"),
            ports: vec![8080],
            client_body_limit: 1024,
            routes: vec![Route {
                alias: "/test".to_string(),
                pages: vec!["index.html".to_string()],
                default_page: "index.html".to_string(),
                check_cookie: false,
                redirect: None,
                links: vec!["/index.html".to_string()],
                methods: vec!["GET".to_string(), "POST".to_string(), "DELETE".to_string()],
            }],
        }
    }

    #[test]
    fn test_route_get() {
        let config = setup_config();
        let req = HttpRequest::new(
            httprequest::Method::Get,
            httprequest::Version::V1_1,
            Resource::Path("/test/index.html".to_string()),
            HashMap::new(),
            String::new(),
        );

        let mut stream = Cursor::new(Vec::new());
        Router::route(req, &mut stream, &config, Vec::new(), "localhost");

        let response = String::from_utf8(stream.into_inner()).expect("Response not valid UTF-8");
        assert!(response.contains("200 OK"));
    }

    #[test]
    fn test_route_post_with_multipart_form_data() {
        let config = setup_config();
        let req = HttpRequest::new(
            httprequest::Method::Post,
            httprequest::Version::V1_1,
            Resource::Path("/test/upload/".to_string()),
            {
                let mut headers = HashMap::new();
                headers.insert(
                    "Content-Type".to_string(),
                    "multipart/form-data; boundary=----WebKitFormBoundary7MA4YWxkTrZu0gW".to_string(),
                );
                headers
            },
            String::new(),
        );

        let body = format!(
            "------WebKitFormBoundary7MA4YWxkTrZu0gW\r\nContent-Disposition: form-data; name=\"file\"; filename=\"test.txt\"\r\n\r\nHello World\r\n------WebKitFormBoundary7MA4YWxkTrZu0gW--"
        ).into_bytes();

        let mut stream = Cursor::new(Vec::new());
        Router::route(req, &mut stream, &config, body, "localhost");

        let response = String::from_utf8(stream.into_inner()).expect("Response not valid UTF-8");
        assert!(response.contains("HTTP/1.1 301 Found")); // Redirect after file upload
    }

    #[test]
    fn test_route_delete() {
        let config = setup_config();
        let req = HttpRequest::new(
            httprequest::Method::Delete,
            httprequest::Version::V1_1,
            Resource::Path("/test/index.html".to_string()),
            HashMap::new(),
            String::new(),
        );

        let mut stream = Cursor::new(Vec::new());
        Router::route(req, &mut stream, &config, Vec::new(), "localhost");

        let response = String::from_utf8(stream.into_inner()).expect("Response not valid UTF-8");
        assert!(response.contains("200 OK") || response.contains("404 Not Found")); // Depending on file existence
    }

    #[test]
    fn test_route_unsupported_method() {
        let config = setup_config();
        let req = HttpRequest::new(
            httprequest::Method::Uninitialized,
            httprequest::Version::V1_1,
            Resource::Path("/test/index.html".to_string()),
            HashMap::new(),
            String::new(),
        );

        let mut stream = Cursor::new(Vec::new());
        Router::route(req, &mut stream, &config, Vec::new(), "localhost");

        let response = String::from_utf8(stream.into_inner()).expect("Response not valid UTF-8");
        assert!(response.contains("405 Method Not Allowed"));
    }

    #[test]
    fn test_respond_with_error() {
        let config = setup_config();
        let mut stream = Cursor::new(Vec::new());

        respond_with_error(&mut stream, &config, "404");

        let response = String::from_utf8(stream.into_inner()).expect("Response not valid UTF-8");
        assert!(response.contains("404 Not Found"));
    }

    #[test]
    fn test_respond_with_redirect() {
        let mut stream = Cursor::new(Vec::new());

        respond_with_redirect(&mut stream, "localhost", "/redirect-path");

        let response = String::from_utf8(stream.into_inner()).expect("Response not valid UTF-8");
        assert!(response.contains("HTTP/1.1 301 Found"));
        assert!(response.contains("Location: http://localhost/redirect-path"));
    }
}
