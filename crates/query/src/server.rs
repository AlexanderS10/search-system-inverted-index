//! Web UI server using tiny_http
//!
//! Serves the single-page application at / and provides a JSON search API
//! at /api/search?q=<query>&mode=<and|or>&k=10

use crate::engine::SearchEngine;
use crate::search::QueryMode;
use std::io::{Cursor, Result as IoResult};
use std::sync::{Arc, Mutex};
use tiny_http::{Header, Response, Server, StatusCode};

const INDEX_HTML: &str = include_str!("../web/index.html");

/// Starts the web server listening on the specified port
///
/// Arguments:
///     engine: SearchEngine wrapped in Arc<Mutex<SearchEngine>>
///     port: port number to listen on (e.g. 8080)
///
/// Returns:
///     IoResult<()>
pub fn run_web_server(engine: Arc<Mutex<SearchEngine>>, port: u16) -> IoResult<()> {
    let addr = format!("127.0.0.1:{}", port);
    let server = Server::http(&addr).map_err(|e| {
        std::io::Error::other(format!("failed to bind web server: {e}"))
    })?;

    println!("MS MARCO Web UI live at http://{}", addr);
    println!("Press Ctrl+C to terminate the server\n");

    for request in server.incoming_requests() {
        let url = request.url().to_string();

        if url == "/" || url == "/index.html" {
            // Serve the single-page application
            let content_type = Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=utf-8"[..]).unwrap();
            let response = Response::new(
                StatusCode(200),
                vec![content_type],
                Cursor::new(INDEX_HTML.as_bytes()),
                Some(INDEX_HTML.len()),
                None,
            );
            let _ = request.respond(response);
        } else if url.starts_with("/api/search") {
            // Parse query parameters
            let (query_str, mode, k) = parse_search_params(&url);

            let json_body = {
                let mut engine_lock = match engine.lock() {
                    Ok(guard) => guard,
                    Err(poisoned) => poisoned.into_inner(),
                };

                match engine_lock.search(&query_str, mode, k) {
                    Ok(resp) => serde_json::to_string(&resp).unwrap_or_else(|_| "{}".to_string()),
                    Err(e) => format!("{{\"error\":\"{}\"}}", e),
                }
            };

            let content_type = Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap();
            let response = Response::new(
                StatusCode(200),
                vec![content_type],
                Cursor::new(json_body.as_bytes()),
                Some(json_body.len()),
                None,
            );
            let _ = request.respond(response);
        } else {
            // Return 404 for unknown endpoints
            let response = Response::from_string("Not Found").with_status_code(404);
            let _ = request.respond(response);
        }
    }

    return Ok(());
}

/// Helper function to parse q, mode, and k from a request URL
///
/// Arguments:
///     url: relative request path such as /api/search?q=machine+learning&mode=and&k=10
///
/// Returns:
///     (query_string, QueryMode, top_k)
fn parse_search_params(url: &str) -> (String, QueryMode, usize) {
    let mut query = String::new();
    let mut mode = QueryMode::And;
    let mut k = 10;

    let Some((_path, query_part)) = url.split_once('?') else {
        return (query, mode, k);
    };

    for pair in query_part.split('&') {
        let Some((key, val)) = pair.split_once('=') else {
            continue;
        };

        match key {
            "q" => {
                query = decode_url_component(val);
            }
            "mode" => {
                if val.eq_ignore_ascii_case("or") {
                    mode = QueryMode::Or;
                } else {
                    mode = QueryMode::And;
                }
            }
            "k" => {
                if let Ok(parsed_k) = val.parse::<usize>() {
                    k = parsed_k.clamp(1, 100);
                }
            }
            _ => {}
        }
    }

    return (query, mode, k);
}

/// Decodes standard URL percent-encoded characters and plus signs
///
/// Arguments:
///     input: encoded URL string component
///
/// Returns:
///     Decoded clean String
fn decode_url_component(input: &str) -> String {
    let mut result = Vec::new();
    let bytes = input.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'+' {
            result.push(b' ');
            i += 1;
        } else if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = &input[i + 1..i + 3];
            if let Ok(byte_val) = u8::from_str_radix(hex, 16) {
                result.push(byte_val);
                i += 3;
            } else {
                result.push(bytes[i]);
                i += 1;
            }
        } else {
            result.push(bytes[i]);
            i += 1;
        }
    }

    return String::from_utf8_lossy(&result).into_owned();
}
