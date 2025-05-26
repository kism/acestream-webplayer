#[macro_use]
extern crate rocket;

use rocket::State;
use rocket::fs::{FileServer, NamedFile};
use rocket::http::{ContentType, Status};
use rocket::request::Request;
use rocket::response::{self, Responder, Response};
use rocket_dyn_templates::{Template, context};
use std::fs;
use std::io::Cursor;
use std::path::Path;

// Constants for M3U8 content types
const M3U8_CONTENT_TYPES: &[(&str, &str)] = &[
    ("application", "vnd.apple.mpegurl"),
    ("audio", "mpegurl"),
    ("application", "x-mpegurl"),
];

// Application configuration struct to define our settings
#[derive(Clone)]
struct AppConfig {
    ace_base_url: String,
    external_base_url: String,
    ace_stream_id: String,
    stream_password: String,
}

// Implement a method to load configuration from Rocket's Figment
impl AppConfig {
    fn from_figment() -> Result<Self, rocket::figment::Error> {
        let figment = rocket::Config::figment();
        Ok(AppConfig {
            ace_base_url: figment.extract_inner("ace_base_url")?,
            external_base_url: figment.extract_inner("external_base_url")?,
            ace_stream_id: figment.extract_inner("ace_stream_id")?,
            stream_password: figment.extract_inner("stream_password")?,
        })
    }
}

// A wrapper for our proxied response
struct ProxyResponse {
    content_type: ContentType,
    data: Vec<u8>,
}

// Implement the Responder trait for ProxyResponse to allow it to be returned from routes
impl<'r> Responder<'r, 'static> for ProxyResponse {
    fn respond_to(self, _: &'r Request<'_>) -> response::Result<'static> {
        Response::build()
            .header(self.content_type)
            .sized_body(self.data.len(), Cursor::new(self.data))
            .ok()
    }
}

// Helper functions
fn validate_file_access(path: &str, file_type: &str) -> Result<(), String> {
    if !Path::new(path).exists() {
        return Err(format!(
            "TLS {} file not found: '{}'\n\
            Please ensure the {} file exists and is accessible.\n\
            Check your Rocket.toml configuration for the correct path.",
            file_type, path, file_type
        ));
    }

    fs::metadata(path).map_err(|e| {
        format!(
            "Cannot access TLS {} file '{}': {}\n\
            Please check file permissions and ensure the file is readable.",
            file_type, path, e
        )
    })?;

    Ok(())
}

// Parse the Content-Type header from the response
fn parse_content_type(response: &reqwest::Response) -> ContentType {
    response
        .headers()
        .get("content-type")
        .and_then(|ct| ct.to_str().ok())
        .and_then(|ct_str| ContentType::parse_flexible(ct_str))
        .unwrap_or(ContentType::Binary)
}

// Check if the content type is one of the known M3U8 types
fn is_m3u8_content(content_type: &ContentType) -> bool {
    M3U8_CONTENT_TYPES
        .iter()
        .any(|(main, sub)| content_type == &ContentType::new(*main, *sub))
}

// Fetch content from a URL and return the content type and bytes
async fn fetch_and_proxy(url: &str) -> Result<(ContentType, Vec<u8>), Status> {
    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|_| Status::BadGateway)?;

    let content_type = parse_content_type(&response);
    let bytes = response
        .bytes()
        .await
        .map_err(|_| Status::InternalServerError)?;

    Ok((content_type, bytes.to_vec()))
}

// Returns the template for the 403 Forbidden page
fn render_forbidden(message: &str) -> Template {
    Template::render("403", context! { message })
}

// Validate that TLS certificate files exist and are readable
fn validate_tls_certificates() -> Result<(), String> {
    // Debug: Print private directory contents
    if let Ok(entries) = fs::read_dir("private") {
        println!("\nprivate directory contents:");
        entries.flatten().for_each(|entry| {
            println!("  - {}", entry.path().display());
        });
    } else {
        println!("Could not read private directory contents.");
    }

    let figment = rocket::Config::figment();

    // Only validate TLS if both cert and key paths are configured
    if let (Ok(certs_path), Ok(key_path)) = (
        figment.extract_inner::<String>("tls.certs"),
        figment.extract_inner::<String>("tls.key"),
    ) {
        validate_file_access(&certs_path, "certificate")?;
        validate_file_access(&key_path, "private key")?;

        println!("✓ TLS certificate files validated successfully:");
        println!("  Certificate: {}", certs_path);
        println!("  Private Key: {}", key_path);
    }

    Ok(())
}

// Stream page endpoint
#[get("/stream/<path..>")]
async fn stream_page(path: std::path::PathBuf, config: &State<AppConfig>) -> Template {
    if path.to_str() != Some(&config.stream_password) {
        return render_forbidden("Stream Forbidden");
    }

    Template::render(
        "stream",
        context! {
            message: "HLS Stream",
            stream_url: format!("{}/hls/{}", config.external_base_url, config.stream_password),
            stream_id: &config.ace_stream_id,
            stream_password: &config.stream_password,
        },
    )
}

// HLS proxy endpoint
#[get("/hls/<path..>")]
async fn hls_proxy(
    path: std::path::PathBuf,
    config: &State<AppConfig>,
) -> Result<ProxyResponse, Status> {
    if path.to_str() != Some(&config.stream_password) {
        return Err(Status::Forbidden);
    }

    let url = format!(
        "{}/ace/manifest.m3u8?content_id={}",
        config.ace_base_url, config.ace_stream_id
    );

    let (content_type, bytes) = fetch_and_proxy(&url).await?;

    // If this is an M3U playlist file, rewrite the URLs
    if is_m3u8_content(&content_type) {
        let content = std::str::from_utf8(&bytes).map_err(|_| Status::InternalServerError)?;

        let modified_content = content
            .lines()
            .map(|line| rewrite_m3u8_line(line, &config.external_base_url))
            .collect::<Vec<String>>()
            .join("\n");

        return Ok(ProxyResponse {
            content_type,
            data: modified_content.into_bytes(),
        });
    }

    Ok(ProxyResponse {
        content_type,
        data: bytes,
    })
}

// Rewrite M3U8 lines to use the external base URL for TS segments
fn rewrite_m3u8_line(line: &str, external_base_url: &str) -> String {
    let trimmed = line.trim();
    if trimmed.ends_with(".ts") && !trimmed.starts_with("#") {
        if let Some(segment_path) = line.split("ace/c/").nth(1) {
            format!("{}/ace/c/{}", external_base_url, segment_path)
        } else if let Some(filename) = line.split('/').last() {
            format!("{}/ace/c/{}", external_base_url, filename)
        } else {
            line.to_string()
        }
    } else {
        line.to_string()
    }
}

// TS segment proxy endpoint
#[get("/ace/c/<path..>")]
async fn ts_segment_proxy(
    path: std::path::PathBuf,
    config: &State<AppConfig>,
) -> Result<ProxyResponse, Status> {
    let url = format!("{}/ace/c/{}", config.ace_base_url, path.display());
    let (content_type, data) = fetch_and_proxy(&url).await?;

    Ok(ProxyResponse { content_type, data })
}

#[get("/")]
async fn index() -> Result<NamedFile, Status> {
    NamedFile::open("static/index.html")
        .await
        .map_err(|_| Status::NotFound)
}

#[launch]
fn rocket() -> _ {
    let config = AppConfig::from_figment().expect("Failed to load configuration from Rocket.toml");

    // Validate TLS certificate files (if configured)
    if let Err(error_msg) = validate_tls_certificates() {
        eprintln!("❌ TLS Certificate Validation Error:\n{}", error_msg);
        eprintln!("\nServer startup aborted due to missing or inaccessible certificate files.");
        std::process::exit(1);
    }

    println!("-----------------------------------------------------------------");
    println!(
        "STREAM URL: {}/stream/{}",
        config.external_base_url, config.stream_password
    );

    println!(
        "Proxying stream: {}/ace/manifest.m3u8?content_id={}",
        config.ace_base_url, config.ace_stream_id
    );
    println!("-----------------------------------------------------------------");

    rocket::build()
        .mount(
            "/",
            routes![index, hls_proxy, stream_page, ts_segment_proxy],
        )
        .mount("/static", FileServer::from("static"))
        .manage(config)
        .attach(Template::fairing())
}
