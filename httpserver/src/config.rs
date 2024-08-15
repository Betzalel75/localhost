// config.rs
use http::httprequest::Resource;
use serde::Deserialize;
use std::collections::HashSet;
use std::fs::{ File, OpenOptions };
use std::path::Path;
use std::{ collections::HashMap, fs };

#[derive(Deserialize, Debug, Clone)]
pub struct Config {
    pub servers: Vec<ConfigServer>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ConfigServer {
    pub host_name: String,
    pub host: String,
    pub ports: Vec<u16>,
    pub root: String,
    pub error_pages: HashMap<String, String>,
    pub client_body_limit: usize,
    pub routes: Vec<Route>,
    pub cgi_extensions: HashMap<String, String>,
    pub directory_listing: bool,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Route {
    pub alias: String,
    pub pages: Vec<String>,
    pub default_page: String,
    pub check_cookie: bool,
    pub redirect: Option<HashMap<String, String>>, // Champ rendu optionnel
    pub links: Vec<String>,
    pub methods: Vec<String>,
}

impl Route {
    fn new() -> Self {
        Self {
            alias: String::new(),
            pages: Vec::new(),
            default_page: String::new(),
            check_cookie: false,
            redirect: Some(HashMap::new()),
            links: Vec::new(),
            methods: Vec::new(),
        }
    }
}

pub fn read_config() -> Option<Config> {
    let config_str = match fs::read_to_string("config.toml") {
        Ok(s) => s,
        Err(_) => {
            return None;
        }
    };
    match toml::from_str(&config_str) {
        Ok(c) => Some(c),
        Err(_) => None,
    }
}

pub fn ok_count_redirect(routes: &Vec<Route>) -> bool {
    routes.iter().all(|route| {
        match &route.redirect {
            Some(map) => map.len() == 1, // Compte le nombre d'éléments dans le HashMap
            None => true, // Si l'option est None, retourne 0
        }
    })
}

pub fn ok_same_port(config: &ConfigServer) -> bool {
    let mut unique_ports = HashSet::new();
    config.ports.iter().all(|port| unique_ports.insert(port))
}

// returns the route to configserver
pub fn find_route(config: &ConfigServer, path: &str) -> (bool, Route) {
    for route in &config.routes {
        if route.alias == path {
            return (true, route.clone());
        }
    }
    (false, Route::new())
}
// checks if pages exists
pub fn is_page_found(config: &ConfigServer, page: &str, alias: String) -> bool {
    config.routes.iter().any(|route| {
        if route.alias == alias { route.pages.contains(&page.to_string()) } else { false }
    })
}

// check methods
pub fn check_methods(config: &ConfigServer, method: &str, alias: &str) -> bool {
    config.routes.iter().any(|route| {
        if route.alias == alias { route.methods.contains(&method.to_string()) } else { false }
    })
}

// checks if the route has a link
pub fn found_links(config: &ConfigServer, path: &str) -> bool {
    let path = path.to_string();
    config.routes.iter().any(|route| route.links.contains(&path))
}

// utils
use hmac::{ Hmac, Mac };
use http::{ httprequest::HttpRequest, httpresponse::HttpResponse };
use rand::Rng;
use sha2::Sha256;
use std::io::{ self, prelude::* };
type HmacSha256 = Hmac<Sha256>;
use crate::handler::{ redirection, Handler, PageErrorHandler, StaticPageHandler };
use hex;
const COOKIE_FILE: &str = "cookies.txt";

pub fn parse_multipart_body(body: &[u8], boundary: &str) -> HashMap<String, (String, Vec<u8>)> {
    let boundary_str = format!("--{}", boundary);
    let boundary_bytes = boundary_str.as_bytes();
    let mut parts = HashMap::new();
    let mut start = 0;

    while let Some(start_boundary) = find_bytes(&body[start..], boundary_bytes) {
        let start_pos = start + start_boundary + boundary_bytes.len();
        let next_boundary_pos = find_bytes(&body[start_pos..], boundary_bytes).map_or(
            body.len(),
            |p| start_pos + p
        );

        if start_pos >= body.len() || next_boundary_pos > body.len() {
            eprintln!("Calculated boundary indices are out of range");
            break;
        }

        let part = &body[start_pos..next_boundary_pos];

        if let Some(content_disposition_start) = find_bytes(part, b"Content-Disposition:") {
            let content_start = find_bytes(&part[content_disposition_start..], b"\r\n\r\n").map_or(
                part.len(),
                |p| content_disposition_start + p + 4
            );

            if content_start >= part.len() {
                eprintln!("Content start index is out of range");
                break;
            }

            let content = &part[content_start..];
            if let Some(filename) = extract_filename(part) {
                let file_content = extract_content(content);
                parts.insert("filename".to_string(), (filename, file_content));
            } else {
                let name = extract_name(part);
                let value = extract_content(content);
                parts.insert(name.clone(), (name, value));
            }
        } else {
            eprintln!("Content-Disposition not found");
        }

        start = next_boundary_pos;
    }

    parts
}

fn extract_filename(part: &[u8]) -> Option<String> {
    find_bytes(part, b"filename=").map(|filename_start| {
        let filename_end = find_bytes(&part[filename_start..], b"\r\n").map_or(
            part.len(),
            |p| filename_start + p
        );
        String::from_utf8_lossy(&part[filename_start + 9..filename_end])
            .trim_matches('"')
            .to_string()
    })
}

fn extract_name(part: &[u8]) -> String {
    let name_start = find_bytes(part, b"name=").unwrap_or(0) + 5;
    let name_end = find_bytes(&part[name_start..], b"\r\n").map_or(part.len(), |p| name_start + p);
    String::from_utf8_lossy(&part[name_start..name_end])
        .trim_matches('"')
        .to_string()
}

fn extract_content(content: &[u8]) -> Vec<u8> {
    let content_end = find_bytes(content, b"\r\n--").unwrap_or(content.len());
    content[..content_end].to_vec()
}

pub fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|window| window == needle)
}

pub fn save_file(root: &str, filename: &str, data: &[u8]) -> io::Result<()> {
    let path = format!("{}/{}",root, filename);
    let mut file = File::create(path)?;
    file.write_all(data)?;
    Ok(())
}

pub fn generate_session_id() -> String {
    // Générer un identifiant aléatoire
    let mut rng = rand::thread_rng();
    let session_id: String = (0..32)
        .map(|_| rng.sample(rand::distributions::Alphanumeric) as char)
        .collect();
    session_id
}

// set cookie
pub fn sign_cookie(value: &str, secret: String) -> String {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect(
        "HMAC can take key of any size"
    );
    mac.update(value.as_bytes());
    let result = mac.finalize();
    let code_bytes = result.into_bytes();
    let code = hex::encode(code_bytes);
    let signed_value = format!("{}|{}", value, code);
    signed_value
}

// set cookie
pub fn set_cookie(
    req: &HttpRequest,
    stream: &mut impl Write,
    secret: String,
    config: &ConfigServer
) {
    let session_id = generate_session_id();
    let signed_cookie = sign_cookie(&session_id, secret);

    let cookie_value = format!("sessionId={}", signed_cookie);
    // Enregistrer le cookie dans un fichier
    if let Err(e) = save_cookie_to_file(&cookie_value) {
        eprintln!("Failed to save cookie: {}", e);
        return;
    }
    // Set a cookie
    let mut headers: HashMap<&str, &str> = HashMap::new();
    let val = format!("{}; Path=/; HttpOnly;", cookie_value);

    headers.insert("Set-Cookie", val.as_str());
    let mut resp: HttpResponse = StaticPageHandler.handle(&req, config);
    resp.headers = Some(headers);
    // Proper error handling instead of unwrap_err()
    if let Err(e) = resp.send_response(stream) {
        eprintln!("Failed to send response: {}", e);
    }
}

fn save_cookie_to_file(cookie: &str) -> std::io::Result<()> {
    let mut file = OpenOptions::new().append(true).create(true).open(COOKIE_FILE)?;
    writeln!(file, "{}", cookie)?;
    Ok(())
}

pub fn verify_cookie(signed_cookie: &str) -> bool {
    if !Path::new(COOKIE_FILE).exists() {
        return false;
    }

    let contents = fs::read_to_string(COOKIE_FILE);
    let contents = contents.ok().expect("msg: Failed to open cookie file");
    let cookies = contents.split("\n").collect::<Vec<&str>>();

    cookies.contains(&signed_cookie.trim())
}

pub fn handle_get_request(
    req: HttpRequest,
    alias: String,
    stream: &mut impl Write,
    config: &ConfigServer,
    route: Route
) {
    if let Some(cookie_header) = req.headers.get("Cookie") {
        if verify_cookie(cookie_header) {
            let Resource::Path(s) = &req.resource;
            let rout: Vec<&str> = s.split("/").collect();
            let file: Option<&&str> = rout.last();
            let file = file.expect("invalid");
            let path = format!("{}/{}", config.root, file);
            match fs::remove_file(path) {
                Ok(_) => {
                    if let Some(redirect_page) = route.redirect {
                        println!("redirect: {} {:?}", alias, redirect_page);
                        let response = redirection(&alias, redirect_page, config);
                        response.send_response(stream).expect("faild to send_response");
                        return;
                    }

                    let response = HttpResponse::new(
                        "200",
                        config.host_name.clone(),
                        None,
                        Some("File Deleted".into())
                    );
                    response.send_response(stream).expect("faild to send_response");
                }
                Err(_) => {
                    let resp = PageErrorHandler::error_response(config, "404");
                    resp.send_response(stream).expect("faild to send_response");
                }
            }
        } else {
            let resp = PageErrorHandler::error_response(config, "401");
            resp.send_response(stream).expect("faild to send_response");
        }
    } else {
        let resp = PageErrorHandler::error_response(config, "401");
        resp.send_response(stream).expect("faild to send_response");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

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
                redirect: Some({
                    let mut m = HashMap::new();
                    m.insert("/old".to_string(), "/new".to_string());
                    m
                }),
                links: vec!["/index.html".to_string()],
                methods: vec!["GET".to_string()],
            }],
        }
    }

    #[test]
    fn test_ok_count_redirect() {
        let config = setup_config();
        assert!(ok_count_redirect(&config.routes));

        let mut routes = config.routes;
        routes[0].redirect = Some(HashMap::new()); // No redirects
        assert!(!ok_count_redirect(&routes)); // Should return false since map is empty
    }

    #[test]
    fn test_find_route() {
        let config = setup_config();
        let (found, route) = find_route(&config, "/test");
        assert!(found);
        assert_eq!(route.alias, "/test");

        let (found, _) = find_route(&config, "/nonexistent");
        assert!(!found);
    }

    #[test]
    fn test_is_page_found() {
        let config = setup_config();
        assert!(is_page_found(&config, "index.html", "/test".to_string()));
        assert!(!is_page_found(&config, "nonexistent.html", "/test".to_string()));
    }

    #[test]
    fn test_check_methods() {
        let config = setup_config();
        assert!(check_methods(&config, "GET", "/test"));
        assert!(!check_methods(&config, "POST", "/test"));
    }

    #[test]
    fn test_found_links() {
        let config = setup_config();
        assert!(found_links(&config, "/index.html"));
        assert!(!found_links(&config, "/nonexistent.html"));
    }

    #[test]
    fn test_parse_multipart_body() {
        let boundary = "----WebKitFormBoundary7MA4YWxkTrZu0gW";
        let body = format!(
            "--{}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"test.txt\"\r\n\r\nHello World\r\n--{}--",
            boundary, boundary
        )
        .into_bytes();
        
        let parts = parse_multipart_body(&body, boundary);
        assert!(parts.contains_key("filename"));
        let (filename, content) = &parts["filename"];
        assert_eq!(filename, "test.txt");
        assert_eq!(content, b"Hello World");
    }

    #[test]
    fn test_find_bytes() {
        let haystack = b"Hello, this is a test";
        let needle = b"test";
        assert_eq!(find_bytes(haystack, needle), Some(15));

        let needle = b"not_found";
        assert_eq!(find_bytes(haystack, needle), None);
    }

    #[test]
    fn test_generate_session_id() {
        let session_id = generate_session_id();
        assert_eq!(session_id.len(), 32);
    }

    #[test]
    fn test_sign_cookie() {
        let value = "sessionId12345";
        let secret = "my_secret_key".to_string();
        let signed_cookie = sign_cookie(value, secret);
        assert!(signed_cookie.contains(value));
    }

    #[test]
    fn test_verify_cookie() {
        let cookie_value = "sessionId=abcd1234|signature";
        save_cookie_to_file(cookie_value).expect("Failed to save cookie");
        assert!(verify_cookie(cookie_value));

        let non_existent_cookie = "sessionId=nonexistent|signature";
        assert!(!verify_cookie(non_existent_cookie));
    }

    #[test]
    fn test_save_file() {
        let result = save_file("/tmp", "test.txt", b"Hello, world!");
        assert!(result.is_ok());
    }
}
