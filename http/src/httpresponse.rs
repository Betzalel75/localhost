// http/httpresponse.rs
use std::collections::HashMap;
use std::io::{Result, Write};

#[derive(Debug, PartialEq, Clone)]
pub struct HttpResponse<'a> {
    pub version: &'a str,
    pub status_code: String,
    pub status_text: &'a str,
    pub headers: Option<HashMap<&'a str, &'a str>>,
    pub body: Option<String>,
}

impl<'a> Default for HttpResponse<'a> {
    fn default() -> Self {
        Self {
            version: "HTTP/1.1".into(),
            status_code: "200".into(),
            status_text: "OK".into(),
            headers: None,
            body: None,
        }
    }
}

impl<'a> HttpResponse<'a> {
    pub fn new(
        status_code: &'a str,
        host_name: String,
        headers: Option<HashMap<&'a str, &'a str>>,
        body: Option<String>,
    ) -> HttpResponse<'a> {
        let mut response: HttpResponse<'a> = HttpResponse::default();

        response.status_code = status_code.into();

        let mut headers = headers.unwrap_or_else(|| {
            let mut h = HashMap::new();
            h.insert("Content-Type", "text/html");
            h
        });

        headers.insert("Server", Box::leak(host_name.into_boxed_str()));

        response.headers = Some(headers);

        response.status_text = get_status_code_text_n_message(status_code).1;
        //[400,403,404,405,413,500].
        response.body = body;
        response
    }

    pub fn send_response(&self, write_stream: &mut impl Write) -> Result<()> {
        let res = self.clone();
        let response_string: String = String::from(res);
        let _ = write!(write_stream, "{}", response_string);
        Ok(())
    }

    pub fn get_version(&self) -> &str {
        self.version
    }
    pub fn get_status_code(&self) -> &str {
        self.status_code.as_str()
    }
    pub fn get_status_text(&self) -> &str {
        self.status_text
    }
    fn get_headers(&self) -> String {
        let map: HashMap<&str, &str> = self.headers.clone().expect("faild to take headers");
        let mut header_string: String = "".into();
        for (k, v) in map.iter() {
            header_string = format!("{}{}:{}\r\n", header_string, k, v);
        }
        header_string
    }
    pub fn get_body(&self) -> &str {
        match &self.body {
            Some(b) => b.as_str(),
            None => "",
        }
    }
}

impl<'a> From<HttpResponse<'a>> for String {
    fn from(res: HttpResponse) -> String {
        let res1 = res.clone();
        format!(
            "{} {} {}\r\n{}Content-Length: {}\r\n\r\n{}",
            &res1.get_version(),
            &res1.get_status_code(),
            &res1.get_status_text(),
            &res1.get_headers(),
            &res.body.unwrap_or("".to_string()).len(),
            &res1.get_body()
        )
    }
}

pub fn get_status_code_text_n_message(code: &str) -> (&str, &str, &str) {
    match code {
        "200" => ("200", "OK", "The request was successful."),
        "400" =>
            (
                "400",
                "Bad Request",
                "The request could not be understood by the server due to malformed syntax",
            ),
        "401" => ("401", "Unauthorized", "Invalid Session"),
        "403" => ("403", "Forbidden", "You don't have permission to access this resource."),
        "404" => ("404", "Not Found", "The requested resource could not be found"),
        "405" =>
            (
                "405",
                "Method Not Allowed",
                "The method specified in the request is not allowed for the resource identified by the request URI.",
            ),
        "408" => ("408", "Request Timeout", "The request timed out due to a timeout"),
        "413" =>
            (
                "413",
                "Payload Too Large",
                "The server is unwilling to process the request because its payload is too large.",
            ),
        _ => ("500", "Internal Server Error", "An unexpected error occurred"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    // test pour code 200: pour une réponse correct suite a une bonne requête
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
                Some(h)
            },
            body: Some("Item was shipped on 21st Dec 2020".into()),
        };
        assert_eq!(response_actual, response_expected);
    }

    #[test]
    // Test de code 404 pour une page qui n'a pas été trouvé, suite à une requête donnée
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
                Some(h)
            },
            body: Some("Item was shipped on 21st Dec 2020".into()),
        };
        let http_string: String = response_expected.into();
        let response_actual =
            "HTTP/1.1 404 Not Found\r\nContent-Type:text/html\r\nContent-Length: 33\r\n\r\nItem was shipped on 21st Dec 2020";
        assert_eq!(http_string, response_actual);
    }
}
