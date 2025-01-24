// handlers.rs
use crate::config::{
    check_methods, find_route, found_links, is_page_found, verify_cookie, ConfigServer,
};
use http::httpresponse::get_status_code_text_n_message;
use http::{httprequest::HttpRequest, httpresponse::HttpResponse};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs::{self};
use std::path::Path;
use std::process::Command;

pub trait Handler {
    fn handle(&self, req: &HttpRequest, config: &ConfigServer) -> HttpResponse;
    fn load_file(file_name: &str, root: &str) -> Option<String> {
        let public_path = env::var("PUBLIC_PATH").unwrap_or(root.to_string());
        let full_path = format!("{}{}", public_path, file_name);
        let contents = fs::read_to_string(full_path);
        contents.ok()
    }

    fn load_default_file() -> Option<String> {
        let default_path = format!("{}/public/", env!("CARGO_MANIFEST_DIR"));
        // println!("corgo manifest: {}", env!("CARGO_MANIFEST_DIR"));
        let public_path = env::var("PUBLIC_PATH").unwrap_or(default_path);
        let full_path = format!("{}{}", public_path, "index.html");
        let contents = fs::read_to_string(full_path);
        contents.ok()
    }
}

#[derive(Serialize, Deserialize)]
pub struct OrderStatus {
    order_id: i32,
    order_date: String,
    order_status: String,
}
pub struct StaticPageHandler;
pub struct PageErrorHandler<'a> {
    pub status_code: &'a str,
}

impl<'a> PageErrorHandler<'a> {
    pub fn new(code: &'a str) -> Self {
        PageErrorHandler { status_code: code }
    }
    pub fn load_file_error(code: &str) -> Option<String> {
        let (code, error, message) = get_status_code_text_n_message(code);
        let default_path = format!("{}/public", env!("CARGO_MANIFEST_DIR"));
        let public_path = env::var("PUBLIC_PATH").unwrap_or(default_path);
        let full_path = format!("{}/{}", public_path, "error.html");
        let contents = fs::read_to_string(full_path);
        let contents = contents.ok()?;

        let contents = contents.replace("{code}", code);
        let contents = contents.replace("{text}", error);
        let contents = contents.replace("{message}", message);

        return Some(contents);
    }
    pub fn load_file_error_client(file_error: &str, root: &str) -> Option<String> {
        let paths = root
            .split(env!("CARGO_MANIFEST_DIR"))
            .collect::<Vec<&str>>();
        let path = paths
            .last()
            .expect("msg: could not find 'CARGO_MANIFEST_DIR'");
        let default_path = format!("{}/{}", env!("CARGO_MANIFEST_DIR"), path);
        let public_path = env::var("PUBLIC_PATH").unwrap_or(default_path);
        let full_path = format!("{}/{}", public_path, file_error);
        let contents = fs::read_to_string(full_path);
        let contents = contents.ok()?;

        return Some(contents);
    }
    pub fn error_response(config: &ConfigServer, status_code: &'a str) -> HttpResponse<'a> {
        if let Some(error_page) = config.error_pages.get(status_code) {
            return HttpResponse::new(
                status_code,
                config.host_name.clone(),
                None,
                PageErrorHandler::load_file_error_client(&error_page, &config.root),
            );
        } else {
            return HttpResponse::new(
                status_code,
                config.host_name.clone(),
                None,
                PageErrorHandler::load_file_error(status_code),
            );
        }
    }
}

pub struct WebServiceHandler;

impl<'a> Handler for PageErrorHandler<'a> {
    fn handle(&self, _req: &HttpRequest, config: &ConfigServer) -> HttpResponse {
        let content = if let Some(error_page) = config.error_pages.get(self.status_code) {
            Self::load_file_error_client(&error_page, &config.root)
        } else {
            Self::load_file_error(&self.status_code)
        };
        HttpResponse::new(&self.status_code, config.host_name.clone(), None, content)
    }
}

impl Handler for StaticPageHandler {
    fn handle(&self, req: &HttpRequest, config: &ConfigServer) -> HttpResponse {
        // Get the path of static page resource being requested
        let http::httprequest::Resource::Path(paths) = &req.resource;
        let route: Vec<&str> = paths.split("/").collect();
        let mut alias = String::new();
        let mut is_alias = false;

        // println!("url complet: {}", paths);
        // let file: &str = route.last().expect("faild to take last");

        let dir = format!("{}{}{}", config.root, "/", paths);
        let path_dir = Path::new(&dir);
        if config.directory_listing {
            let routes = find_route(config, "/").1;
            if routes.check_cookie {
                if let Some(cookie_header) = req.headers.get("Cookie") {
                    if !verify_cookie(cookie_header) {
                        return PageErrorHandler::error_response(config, "401");
                    }
                } else {
                    return PageErrorHandler::error_response(config, "401");
                }
            }
            if path_dir.exists() {
                // println!("exit");
                if path_dir.is_dir() {
                    // println!("is dir {}", path_dir.display());
                    let mut url = paths.clone();
                    if !paths.ends_with('/') {
                        url.push('/');
                    }
                    return match list_directory_contents(&config.root, &url) {
                        Some(contents) => {
                            HttpResponse::new("200", config.host_name.clone(), None, Some(contents))
                        }
                        None => PageErrorHandler::error_response(config, "404"),
                    };
                }
                // println!("is file {} paths: {}", file, paths);
                // let root = format!("{}{}", config.root, paths.replace(file, ""));
                return HttpResponse::new(
                    "200",
                    config.host_name.clone(),
                    None,
                    Self::load_file(paths, &config.root),
                );
            }
        }
        if route[0] == "" && route.len() == 2 {
            alias.push_str("/");
        } else {
            alias = format!("/{}/", route[1]);
            is_alias = true;
        }
        let method = format!("{:?}", req.method);

        if &alias == paths {
            if !check_methods(config, &method.to_ascii_uppercase(), &alias) {
                return PageErrorHandler::error_response(config, "405");
            }
            let (is_match, route) = find_route(config, &paths);
            if is_match {
                if route.check_cookie {
                    if let Some(cookie_header) = req.headers.get("Cookie") {
                        if !verify_cookie(cookie_header) {
                            // println!("Not cookie in file");
                            return PageErrorHandler::error_response(config, "401");
                        }
                    } else {
                        // println!("Not cookie in headers");
                        return PageErrorHandler::error_response(config, "401");
                    }
                }
                if let Some(redirect_page) = route.redirect {
                    return redirection(&alias, redirect_page, config);
                }

                // Load default page if directory listing is disabled
                if route.default_page.is_empty() {
                    return PageErrorHandler::error_response(config, "404");
                }

                return HttpResponse::new(
                    "200",
                    config.host_name.clone(),
                    None,
                    Self::load_file(&route.default_page, &config.root),
                );
            } else {
                if path_dir.exists() {
                    return PageErrorHandler::error_response(config, "403");
                }
                return PageErrorHandler::error_response(config, "404");
            }
        }

        let page = paths.replacen(&alias, "", 1);
        let file = route.last().expect("faild to take last");

        if file.ends_with(".php") || file.ends_with(".py") {
            let mut path = String::from("/");
            if route.len() > 2 {
                path = format!("/{}/", route[1]);
            }
            let (is_match, route) = find_route(config, &path);
            if is_match {
                if route.check_cookie {
                    if let Some(cookie_header) = req.headers.get("Cookie") {
                        if !verify_cookie(cookie_header) {
                            return PageErrorHandler::error_response(config, "401");
                        }
                    } else {
                        return PageErrorHandler::error_response(config, "401");
                    }
                }
                let output = StaticPageHandler::handle_cgi_request(&paths, &config);
                if output.is_empty() {
                    return PageErrorHandler::error_response(config, "404");
                }
                return HttpResponse::new("200", config.host_name.clone(), None, Some(output));
            }
        }

        match *file {
            "" => {
                if !check_methods(config, &method, &alias) {
                    return PageErrorHandler::error_response(config, "405");
                }
                let (is_match, route) = find_route(config, "/");
                if is_match {
                    if route.check_cookie {
                        if let Some(cookie_header) = req.headers.get("Cookie") {
                            if !verify_cookie(cookie_header) {
                                return PageErrorHandler::error_response(config, "401");
                            }
                        } else {
                            return PageErrorHandler::error_response(config, "401");
                        }
                    }
                    if let Some(redirect_page) = route.redirect {
                        return redirection(&alias, redirect_page, config);
                    }

                    // If no directory listing and no default page, return 404
                    if route.default_page.is_empty() {
                        return PageErrorHandler::error_response(config, "404");
                    }

                    HttpResponse::new(
                        "200",
                        config.host_name.clone(),
                        None,
                        Self::load_default_file(),
                    )
                } else {
                    if path_dir.exists() {
                        return PageErrorHandler::error_response(config, "403");
                    }
                    PageErrorHandler::error_response(config, "404")
                }
            }
            other => {
                let mut full_path = format!("/{}", other);
                let (is_match, route) = find_route(config, &alias);
                if is_alias {
                    full_path = format!("/{}", paths.replacen(&alias, "", 1))
                }

                if found_links(config, &full_path) {
                    match Self::load_file(&full_path, &config.root) {
                        Some(contents) => {
                            let mut map: HashMap<&str, &str> = HashMap::new();
                            if full_path.ends_with(".css") {
                                map.insert("Content-Type", "text/css");
                            } else if full_path.ends_with(".js") {
                                map.insert("Content-Type", "text/javascript");
                            } else {
                                map.insert("Content-Type", "text/html");
                            }
                            HttpResponse::new(
                                "200",
                                config.host_name.clone(),
                                Some(map),
                                Some(contents),
                            )
                        }
                        None => PageErrorHandler::error_response(config, "404"),
                    }
                } else if is_match {
                    if route.check_cookie {
                        if let Some(cookie_header) = req.headers.get("Cookie") {
                            if !verify_cookie(cookie_header) {
                                return PageErrorHandler::error_response(config, "401");
                            }
                        } else {
                            return PageErrorHandler::error_response(config, "401");
                        }
                    }
                    let file_path = format!("/{}", file);
                    if !check_methods(config, &method.to_ascii_uppercase(), &alias) {
                        return PageErrorHandler::error_response(config, "405");
                    }
                    if !is_page_found(config, &page, alias.clone()) {
                        return PageErrorHandler::error_response(config, "404");
                    }
                    if let Some(redirect_page) = route.redirect {
                        return redirection(&alias, redirect_page, config);
                    }
                    HttpResponse::new(
                        "200",
                        config.host_name.clone(),
                        None,
                        Self::load_file(&file_path, &config.root),
                    )
                } else {
                    let dir = format!("{}/{}", config.root, other);
                    let path_dir = Path::new(&dir);
                    if path_dir.exists() {
                        return PageErrorHandler::error_response(config, "403");
                    }
                    PageErrorHandler::error_response(config, "404")
                }
            }
        }
    }
}

impl WebServiceHandler {
    fn load_json() -> Vec<OrderStatus> {
        let default_path = format!("{}/data", env!("CARGO_MANIFEST_DIR"));
        let data_path = env::var("DATA_PATH").unwrap_or(default_path);
        let full_path = format!("{}/{}", data_path, "orders.json");
        let json_contents = fs::read_to_string(full_path);
        let orders: Vec<OrderStatus> =
            serde_json::from_str(json_contents.unwrap().as_str()).unwrap();
        orders
    }
}

// Implement the Handler trait
impl Handler for WebServiceHandler {
    fn handle(&self, req: &HttpRequest, config: &ConfigServer) -> HttpResponse {
        let http::httprequest::Resource::Path(s) = &req.resource;
        // Parse the URI
        let route: Vec<&str> = s.split("/").collect();
        // if route if /api/shipping/orders, return json
        match route[2] {
            "shipping" if route.len() > 2 && route[3] == "orders" => {
                let body =
                    Some(serde_json::to_string(&Self::load_json()).expect("Error loading json"));
                let mut headers: HashMap<&str, &str> = HashMap::new();
                headers.insert("Content-Type", "application/json");
                HttpResponse::new("200", config.host_name.clone(), Some(headers), body)
            }
            _ => PageErrorHandler::error_response(config, "404"),
        }
    }
}

impl StaticPageHandler {
    pub fn handle_cgi_request(path: &str, server: &ConfigServer) -> String {
        let keys: Vec<&str> = path.split('.').collect();
        let k = keys.last().expect("faild to take last");
        if let Some(script_path) = server.cgi_extensions.get(*k) {
            let url_cgi = format!("{}{}", server.root, script_path);
            if !Path::new(&url_cgi).exists() {
                return "".to_string();
            }
            let output = if script_path.ends_with(".php") {
                Command::new(url_cgi)
                    .output()
                    .expect("Failed to execute PHP CGI script")
            } else if script_path.ends_with(".py") {
                Command::new("python3")
                    .arg(url_cgi)
                    .output()
                    .expect("Failed to execute Python CGI script")
            } else {
                eprintln!("Unsupported CGI extension");
                return "Unsupported CGI extension".to_string();
            };
            if output.status.success() {
                return String::from_utf8_lossy(&output.stdout).to_string();
            }

            return String::from_utf8_lossy(&output.stderr).to_string();
        }
        "".to_string()
    }
}

// Fonction pour gerer la redirection
pub fn redirection<'a>(
    alias: &str,
    redirect_page: HashMap<String, String>,
    config: &ConfigServer,
) -> HttpResponse<'a> {
    if let Some((new_alias, new_file)) = redirect_page.into_iter().next() {
        if alias == new_alias || is_cycle(alias, &new_alias, config) {
            return PageErrorHandler::error_response(config, "500");
        }
        let new_url = format!("{}{}", new_alias, new_file);
        let new_url_str: &'a str = Box::leak(new_url.into_boxed_str());

        let mut headers = HashMap::new();
        headers.insert("Location", new_url_str);

        return HttpResponse::new("302", config.host_name.clone(), Some(headers), None);
    }
    HttpResponse::new("", "".to_string(), None, None)
}

// Fonction pour verifier qu'il n'y a pas de cycle infie entre les redirection
fn is_cycle(past_alias: &str, new_alias: &str, config: &ConfigServer) -> bool {
    let (is_match, route) = find_route(config, &new_alias);
    if is_match {
        if let Some(redirect_page) = route.redirect {
            let mut seen: HashSet<String> = HashSet::new();
            seen.insert(past_alias.to_string());

            for (futur_alias, _) in redirect_page {
                if seen.contains(futur_alias.as_str()) {
                    return true;
                }
                seen.insert(futur_alias);
            }
        }
    }
    false
}

// Helper function to list directory contents
fn list_directory_contents(root: &str, url: &str) -> Option<String> {
    let mut entries = Vec::new();
    let dir = format!("{}{}", root, url);
    if let Ok(paths) = fs::read_dir(dir) {
        for path in paths {
            if let Ok(entry) = path {
                let file_name = entry.file_name().into_string().ok()?;
                let file_type = entry.file_type().ok()?;
                if file_type.is_dir() {
                    entries.push(format!(
                        "<li><a href=\"{}{}/\"><strong>{}/</strong></a></li>",
                        url, file_name, file_name
                    ));
                } else if file_type.is_file() {
                    entries.push(format!(
                        "<li><a href=\"{}{}\">{}</a></li>",
                        url, file_name, file_name
                    ));
                }
            }
        }
    }
    parse_html(&entries.join(""))
}

fn parse_html(files: &str) -> Option<String> {
    let default_path = format!("{}/public", env!("CARGO_MANIFEST_DIR"));
    let public_path = env::var("PUBLIC_PATH").unwrap_or(default_path);
    let full_path = format!("{}/{}", public_path, "dir.html");
    let contents = fs::read_to_string(full_path);
    let contents = contents.ok()?;
    let contents = contents.replace("{files}", files);
    Some(contents)
}

#[cfg(test)]
mod tests {
    use crate::config::Route;

    use super::*;
    use http::httprequest::{HttpRequest, Method, Resource, Version};
    use std::collections::HashMap;

    fn setup_config() -> ConfigServer {
        let mut error_pages = HashMap::new();
        error_pages.insert("404".to_string(), "/errors/404.html".to_string());

        let mut cgi_extensions = HashMap::new();
        cgi_extensions.insert("py".to_string(), "/cgi-bin/script.py".to_string());

        ConfigServer {
            host_name: String::from("localhost"),
            root: String::from("/public"),
            directory_listing: false,
            error_pages,
            cgi_extensions,
            host: String::from("127.0.0.1"),
            ports: vec![8080],
            client_body_limit: 1024,
            routes: vec![Route {
                alias: "/".to_string(),
                pages: vec!["index.html".to_string()],
                default_page: "index.html".to_string(),
                check_cookie: false,
                redirect: None,
                links: vec!["index.html".to_string()],
                methods: vec!["GET".to_string()],
            }],
        }
    }

    #[test]
    fn test_static_page_handler_handle() {
        let handler = StaticPageHandler;
        let config = setup_config();
        let req = HttpRequest::new(Method::Get, Version::V1_1, Resource::Path("/index.html".to_string()), HashMap::new(), String::new());

        let response = handler.handle(&req, &config);
        assert_eq!(response.get_status_code(), "200");
    }

    #[test]
    fn test_page_error_handler_handle_404() {
        let handler = PageErrorHandler::new("404");
        let config = setup_config();
        let req = HttpRequest::new(Method::Get, Version::V1_1, Resource::Path("/nonexistent.html".to_string()), HashMap::new(), String::new());

        let response = handler.handle(&req, &config);
        assert_eq!(response.get_status_code(), "404");
    }

    #[test]
    fn test_web_service_handler_handle_shipping_orders() {
        let handler = WebServiceHandler;
        let config = setup_config();
        let req = HttpRequest::new(Method::Get, Version::V1_1, Resource::Path("/api/shipping/orders".to_string()), HashMap::new(), String::new());

        let response = handler.handle(&req, &config);
        assert_eq!(response.get_status_code(), "200");
        assert!(response.get_body().contains("order_id"));
    }

    #[test]
    fn test_page_error_handler_load_file_error() {
        let error_page = PageErrorHandler::load_file_error("404");
        assert!(error_page.is_some());
        let contents = error_page.unwrap();
        assert!(contents.contains("404"));
    }

    #[test]
    fn test_static_page_handler_load_file() {
        let file_contents = StaticPageHandler::load_file("index.html", "/public");
        assert!(file_contents.is_some());
    }

    #[test]
    fn test_static_page_handler_handle_cgi_request() {
        let server = setup_config();
        let output = StaticPageHandler::handle_cgi_request("script.py", &server);
        // Assurez-vous que le script CGI "script.py" renvoie bien "CGI output"
        assert!(output.contains("CGI output"));
    }
}

#[cfg(test)]
mod response_tests {
    use super::*;

    #[test]
    fn test_response_struct_creation_200() {
        let response_actual = HttpResponse::new(
            "200",
            "localhost".to_string(),
            None,
            Some("Item was shipped on 21st Dec 2020".into()),
        );
        let response_expected = HttpResponse {
            version: "HTTP/1.1",
            status_code: "200".to_owned(),
            status_text: "OK",
            headers: {
                let mut h = HashMap::new();
                h.insert("Content-Type", "text/html");
                h.insert("Server", "localhost");
                Some(h)
            },
            body: Some("Item was shipped on 21st Dec 2020".into()),
        };
        assert_eq!(response_actual, response_expected);
    }

    #[test]
    fn test_response_struct_creation_404() {
        let response_actual = HttpResponse::new(
            "404",
            "localhost".to_string(),
            None,
            Some("Item was shipped on 21st Dec 2020".into()),
        );
        let response_expected = HttpResponse {
            version: "HTTP/1.1",
            status_code: "404".to_owned(),
            status_text: "Not Found",
            headers: {
                let mut h = HashMap::new();
                h.insert("Content-Type", "text/html");
                h.insert("Server", "localhost");
                Some(h)
            },
            body: Some("Item was shipped on 21st Dec 2020".into()),
        };
        assert_eq!(response_actual, response_expected);
    }

    #[test]
    fn test_http_response_creation() {
        let response_expected = HttpResponse {
            version: "HTTP/1.1",
            status_code: "404".to_owned(),
            status_text: "Not Found",
            headers: {
                let mut h = HashMap::new();
                h.insert("Content-Type", "text/html");
                h.insert("Server", "localhost");
                Some(h)
            },
            body: Some("Item was shipped on 21st Dec 2020".into()),
        };
        let http_string: String = response_expected.clone().into();
        let response_actual =
            "HTTP/1.1 404 Not Found\r\nContent-Type:text/html\r\nServer:localhost\r\nContent-Length: 33\r\n\r\nItem was shipped on 21st Dec 2020";
        assert_eq!(http_string, response_actual);
    }
}
