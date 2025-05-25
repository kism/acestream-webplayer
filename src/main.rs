#[macro_use]
extern crate rocket;

use rocket::State;
use rocket::fs::FileServer;
use rocket::fs::NamedFile;
use rocket::http::ContentType;
use rocket::http::Status;
use rocket::request::Request;
use rocket::response::{self, Responder, Response};

use rocket_dyn_templates::{Template, context};
use std::fs;
use std::io::Cursor;
use std::path::Path;

// Configuration struct to hold our settings
struct AppConfig {
    ace_base_url: String,
    external_base_url: String,
    ace_stream_id: String,
    stream_password: String,
}

// A wrapper for our proxied response
struct ProxyResponse {
    content_type: ContentType,
    data: Vec<u8>,
}

impl<'r> Responder<'r, 'static> for ProxyResponse {
    fn respond_to(self, _: &'r Request<'_>) -> response::Result<'static> {
        Response::build()
            .header(self.content_type)
            .sized_body(self.data.len(), Cursor::new(self.data))
            .ok()
    }
}

// Returns the template for the 403 Forbidden page, and sets the status to 403
fn render_forbidden(message: String) -> Template {
    Template::render(
        "403",
        context! {
            message: message,
        },
    )
}

// Validate that TLS certificate files exist and are readable
fn validate_tls_certificates() -> Result<(), String> {
    // Print a folder listing of the current directory
    if let Ok(entries) = fs::read_dir("private") {
        println!("\nprivate directory contents:");
        for entry in entries.flatten() {
            println!("  - {}", entry.path().display());
        }
    } else {
        println!("Could not read private directory contents.");
    }

    // Get the figment to read configuration
    let figment = rocket::Config::figment();

    // Try to extract TLS configuration - this will only exist in release mode
    if let Ok(certs_path) = figment.extract_inner::<String>("tls.certs") {
        if let Ok(key_path) = figment.extract_inner::<String>("tls.key") {
            // Check if certificate file exists and is readable
            if !Path::new(&certs_path).exists() {
                return Err(format!(
                    "TLS certificate file not found: '{}'\n\
                    Please ensure the certificate file exists and is accessible.\n\
                    Check your Rocket.toml configuration for the correct path.",
                    certs_path
                ));
            }

            // Check if certificate file is readable
            if let Err(e) = fs::metadata(&certs_path) {
                return Err(format!(
                    "Cannot access TLS certificate file '{}': {}\n\
                    Please check file permissions and ensure the file is readable.",
                    certs_path, e
                ));
            }

            // Check if private key file exists and is readable
            if !Path::new(&key_path).exists() {
                return Err(format!(
                    "TLS private key file not found: '{}'\n\
                    Please ensure the private key file exists and is accessible.\n\
                    Check your Rocket.toml configuration for the correct path.",
                    key_path
                ));
            }

            // Check if private key file is readable
            if let Err(e) = fs::metadata(&key_path) {
                return Err(format!(
                    "Cannot access TLS private key file '{}': {}\n\
                    Please check file permissions and ensure the file is readable.",
                    key_path, e
                ));
            }

            println!("✓ TLS certificate files validated successfully:");
            println!("  Certificate: {}", certs_path);
            println!("  Private Key: {}", key_path);
        }
    }

    Ok(())
}

// HLS proxy endpoint
#[get("/stream/<path..>")]
async fn stream_page(path: std::path::PathBuf, config: &State<AppConfig>) -> Template {
    if path.to_str() != Some(&config.stream_password) {
        return render_forbidden("Stream Forbidden".to_string());
    }

    let stream_url = format!(
        "{}/hls/{}",
        config.external_base_url, config.stream_password
    );

    Template::render(
        "stream",
        context! {
            message: "HLS Stream",
            stream_url: stream_url,
            stream_id: config.ace_stream_id.clone(),
            stream_password: config.stream_password.clone(),
        },
    )
}

// HLS proxy endpoint
#[get("/hls/<path..>")]
async fn hls_proxy(
    path: std::path::PathBuf,
    config: &State<AppConfig>,
) -> Result<ProxyResponse, Status> {
    // If the path isn't the stream password, return a 403
    if path.to_str() != Some(&config.stream_password) {
        return Err(Status::Forbidden);
    }

    // println!("Proxying HLS stream for path: {}", path.display());

    // Use the base URL from config
    // let url = format!("{}/ace/manifest.m3u8?content_id={}&transcode_ac3=1", config.ace_base_url, config.ace_stream_id);
    let url = format!(
        "{}/ace/manifest.m3u8?content_id={}",
        config.ace_base_url, config.ace_stream_id
    );

    // Fetch the content from the source
    let client = reqwest::Client::new();
    let response = match client.get(&url).send().await {
        Ok(resp) => resp,
        Err(_) => return Err(Status::BadGateway),
    };

    // Get content type
    let content_type = match response.headers().get("content-type") {
        Some(ct) => match ct.to_str() {
            Ok(ct_str) => ContentType::parse_flexible(ct_str).unwrap_or(ContentType::Binary),
            Err(_) => ContentType::Binary,
        },
        None => ContentType::Binary,
    };

    // Get body as bytes
    let bytes = match response.bytes().await {
        Ok(b) => b,
        Err(_) => return Err(Status::InternalServerError),
    };

    // If this is an M3U playlist file, rewrite the URLs
    if content_type == ContentType::new("application", "vnd.apple.mpegurl")
        || content_type == ContentType::new("audio", "mpegurl")
        || content_type == ContentType::new("application", "x-mpegurl")
    {
        let content = match std::str::from_utf8(&bytes) {
            Ok(s) => s,
            Err(_) => return Err(Status::InternalServerError),
        };

        // Rewrite URLs to be handled by our ts_segment_proxy endpoint
        let modified_content = content
            .lines()
            .map(|line| {
                // Check if the line is a URL for a .ts segment
                if line.trim().ends_with(".ts") && !line.trim().starts_with("#") {
                    // Extract just the filename part from the full URL
                    if let Some(filename) = line.split('/').last() {
                        // Replace with our proxied URL
                        let segment_path = line.split("ace/c/").nth(1).unwrap_or(filename);
                        format!("{}/ace/c/{}", config.external_base_url, segment_path)
                    } else {
                        line.to_string()
                    }
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<String>>()
            .join("\n");

        return Ok(ProxyResponse {
            content_type,
            data: modified_content.into_bytes(),
        });
    }

    Ok(ProxyResponse {
        content_type,
        data: bytes.to_vec(),
    })
}

// TS segment proxy endpoint
#[get("/ace/c/<path..>")]
async fn ts_segment_proxy(
    path: std::path::PathBuf,
    config: &State<AppConfig>,
) -> Result<ProxyResponse, Status> {
    // println!("Proxying TS segment: {}", path.display());

    // Construct the source URL using the base from the config
    let url = format!("{}/ace/c/{}", config.ace_base_url, path.display());

    // Fetch the content from the source
    let client = reqwest::Client::new();
    let response = match client.get(&url).send().await {
        Ok(resp) => resp,
        Err(_) => return Err(Status::BadGateway),
    };

    // Get content type
    let content_type = match response.headers().get("content-type") {
        Some(ct) => match ct.to_str() {
            Ok(ct_str) => ContentType::parse_flexible(ct_str).unwrap_or(ContentType::Binary),
            Err(_) => ContentType::Binary,
        },
        None => ContentType::Binary,
    };

    // Get body as bytes
    let bytes = match response.bytes().await {
        Ok(b) => b,
        Err(_) => return Err(Status::InternalServerError),
    };

    Ok(ProxyResponse {
        content_type,
        data: bytes.to_vec(),
    })
}

#[get("/")]
async fn index() -> NamedFile {
    NamedFile::open("static/index.html").await.unwrap()
}

#[launch]
fn rocket() -> _ {
    let config = AppConfig {
        ace_base_url: rocket::Config::figment()
            .extract_inner("ace_base_url")
            .unwrap(),
        external_base_url: rocket::Config::figment()
            .extract_inner("external_base_url")
            .unwrap(),
        ace_stream_id: rocket::Config::figment()
            .extract_inner("ace_stream_id")
            .unwrap(),
        stream_password: rocket::Config::figment()
            .extract_inner("stream_password")
            .unwrap(),
    };

    // Validate TLS certificate files (if configured)
    if let Err(error_msg) = validate_tls_certificates() {
        eprintln!("❌ TLS Certificate Validation Error:");
        eprintln!("{}", error_msg);
        eprintln!("\nServer startup aborted due to missing or inaccessible certificate files.");
        std::process::exit(1);
    }

    println!("-----------------------------------------------------------------");
    println!("STREAM URL: {}/stream/{}", config.external_base_url, config.stream_password);
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
