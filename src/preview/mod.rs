//! Loopback server for the browser preview and its deck state.
mod assets;

use self::assets::{asset_url, image_mime, image_path};
use serde_json::json;
use std::{
    collections::{BTreeMap, HashMap},
    fs::File,
    io::Read,
    net::{Ipv4Addr, TcpListener},
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

const MAX_DECK_BYTES: u64 = 4 * 1024 * 1024;
const CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'";

pub(crate) struct PreviewServer {
    server: Server,
    host: String,
    deck: DeckState,
    presenter: bool,
}

impl PreviewServer {
    pub(crate) fn bind(
        path: PathBuf,
        presenter: bool,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        if !path.is_file() {
            return Err(format!("Not a file: {}", path.display()).into());
        }
        let root = path
            .parent()
            .ok_or("Missing deck directory")?
            .canonicalize()?;
        let deck = DeckState {
            title: path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            path,
            root,
            cached: None,
            position: 0,
        };
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let host = listener.local_addr()?.to_string();
        let server = Server::from_listener(listener, None)?;
        Ok(Self {
            server,
            host,
            deck,
            presenter,
        })
    }

    pub(crate) fn url(&self) -> String {
        format!("http://{}/", self.host)
    }

    pub(crate) fn serve(mut self) {
        for request in self.server.incoming_requests() {
            self.deck.serve(request, &self.host, self.presenter);
        }
    }
}

struct DeckState {
    path: PathBuf,
    root: PathBuf,
    title: String,
    cached: Option<Snapshot>,
    position: usize,
}

struct Snapshot {
    source: String,
    deck: flik::Deck,
    assets: HashMap<String, PathBuf>,
}

impl DeckState {
    fn deck(&mut self, include_notes: bool) -> String {
        let mut bytes = Vec::new();
        if let Err(err) = File::open(&self.path)
            .and_then(|file| file.take(MAX_DECK_BYTES + 1).read_to_end(&mut bytes))
        {
            return json!({"error": format!("Could not read deck: {err}")}).to_string();
        }
        if bytes.len() as u64 > MAX_DECK_BYTES {
            return json!({"error": "Deck exceeds the 4 MiB size limit"}).to_string();
        }
        let Ok(source) = String::from_utf8(bytes) else {
            return json!({"error": "Deck must be UTF-8"}).to_string();
        };
        if let Some(cached) = self
            .cached
            .as_ref()
            .filter(|cached| cached.source == source)
        {
            return self.response(cached, include_notes);
        }
        let mut assets = HashMap::new();
        match flik::render(&source, &self.title, |input| {
            let path = image_path(input)?;
            let url = asset_url(&path)?;
            assets.insert(url.clone(), path);
            Some(url)
        }) {
            Ok(deck) => {
                let snapshot = Snapshot {
                    source,
                    deck,
                    assets,
                };
                self.position = self.position.min(snapshot.deck.slides.len() - 1);
                let response = self.response(&snapshot, include_notes);
                self.cached = Some(snapshot);
                response
            }
            Err(err) => json!({"error": err}).to_string(),
        }
    }

    fn resolve_asset(&self, path: &Path) -> Option<PathBuf> {
        let resolved = self.root.join(path).canonicalize().ok()?;
        if !resolved.starts_with(&self.root) || !resolved.is_file() {
            return None;
        }
        image_mime(&resolved)?;
        Some(resolved)
    }

    fn response(&self, snapshot: &Snapshot, include_notes: bool) -> String {
        let revisions: BTreeMap<_, _> = snapshot
            .assets
            .iter()
            .map(|(url, path)| {
                let revision = self.resolve_asset(path).and_then(|path| {
                    let meta = path.metadata().ok()?;
                    let modified = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
                    Some(format!("{}-{}", modified.as_nanos(), meta.len()))
                });
                (url, revision)
            })
            .collect();
        let mut response = json!({"deck": snapshot.deck, "imageRevisions": revisions});
        if include_notes {
            let notes: Vec<_> = snapshot
                .deck
                .slides
                .iter()
                .map(|slide| &slide.notes)
                .collect();
            response["notes"] = json!(notes);
        }
        response.to_string()
    }

    fn serve_position(&mut self, request: Request) {
        match request.method() {
            Method::Get => {}
            Method::Post => {
                // Cross-origin pages cannot send this header without an allowed CORS preflight.
                let mut positions = request
                    .headers()
                    .iter()
                    .filter(|h| h.field.equiv("X-Flik-Slide"));
                let header = positions.next();
                if header.is_none() || positions.next().is_some() {
                    send_text(
                        request,
                        403,
                        "text/plain; charset=utf-8",
                        "Missing or ambiguous slide header",
                    );
                    return;
                }
                let position = header.and_then(|h| h.value.as_str().parse::<usize>().ok());
                let (Some(position), Some(snapshot)) = (position, &self.cached) else {
                    send_text(
                        request,
                        400,
                        "text/plain; charset=utf-8",
                        "Invalid slide position",
                    );
                    return;
                };
                self.position = position.min(snapshot.deck.slides.len() - 1);
            }
            _ => {
                send_text(
                    request,
                    405,
                    "text/plain; charset=utf-8",
                    "Method not allowed",
                );
                return;
            }
        }
        send_text(
            request,
            200,
            "application/json; charset=utf-8",
            json!(self.position).to_string(),
        );
    }

    fn serve(&mut self, request: Request, host: &str, presenter: bool) {
        let headers = request.headers().iter().filter(|h| h.field.equiv("Host"));
        if !headers.map(|h| h.value.as_str()).eq([host]) {
            send_text(request, 403, "text/plain; charset=utf-8", "Forbidden host");
            return;
        }
        if presenter && request.url() == "/api/position" {
            self.serve_position(request);
            return;
        }
        if request.method() != &Method::Get {
            send_text(
                request,
                405,
                "text/plain; charset=utf-8",
                "Method not allowed",
            );
            return;
        }
        match request.url() {
            url if url == "/" || (presenter && matches!(url, "/audience" | "/presenter")) => {
                send_text(
                    request,
                    200,
                    "text/html; charset=utf-8",
                    include_str!("../../web/index.html"),
                )
            }
            "/style.css" => send_text(
                request,
                200,
                "text/css; charset=utf-8",
                include_str!("../../web/style.css"),
            ),
            "/app.js" => send_text(
                request,
                200,
                "text/javascript; charset=utf-8",
                include_str!("../../web/app.js"),
            ),
            "/renderer.js" => send_text(
                request,
                200,
                "text/javascript; charset=utf-8",
                include_str!("../../web/renderer.js"),
            ),
            url if url == "/api/deck" || (presenter && url == "/api/presenter") => {
                let body = self.deck(url == "/api/presenter");
                send_text(request, 200, "application/json; charset=utf-8", body);
            }
            url => {
                let image = self
                    .cached
                    .as_ref()
                    .and_then(|cached| cached.assets.get(url.split('?').next().unwrap_or(url)))
                    .and_then(|path| self.resolve_asset(path))
                    .and_then(|path| Some((File::open(&path).ok()?, image_mime(&path)?)));
                match image {
                    Some((file, mime)) => send(request, Response::from_file(file), mime),
                    None => send_text(request, 404, "text/plain; charset=utf-8", "Not found"),
                }
            }
        }
    }
}

fn send_text(request: Request, status: u16, content_type: &str, body: impl Into<String>) {
    let response = Response::from_string(body.into()).with_status_code(StatusCode(status));
    send(request, response, content_type);
}

fn send(request: Request, mut response: Response<impl Read>, content_type: &str) {
    for (name, value) in [
        ("Content-Type", content_type),
        ("Content-Security-Policy", CSP),
        ("X-Content-Type-Options", "nosniff"),
        ("Cross-Origin-Resource-Policy", "same-origin"),
        ("Referrer-Policy", "no-referrer"),
        ("Cache-Control", "no-store"),
    ] {
        response.add_header(Header::from_bytes(name, value).expect("valid HTTP header"));
    }
    let _ = request.respond(response);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Write, net::TcpStream, time::Duration};

    #[test]
    fn presenter_position_is_shared_and_writes_require_a_custom_header() {
        let preview = PreviewServer::bind(PathBuf::from("examples/demo.md"), true).unwrap();
        let host = preview.host.clone();
        std::thread::spawn(move || preview.serve());
        let request = |method, path, position: Option<&str>| {
            let mut stream = TcpStream::connect(&host).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let header =
                position.map_or(String::new(), |value| format!("X-Flik-Slide: {value}\r\n"));
            write!(stream, "{method} {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nContent-Length: 0\r\n{header}\r\n").unwrap();
            let mut response = String::new();
            stream.read_to_string(&mut response).unwrap();
            let (headers, body) = response.split_once("\r\n\r\n").unwrap();
            (
                headers.split_whitespace().nth(1).unwrap().to_owned(),
                body.to_owned(),
            )
        };
        assert_eq!(request("GET", "/api/deck", None).0, "200");
        assert_eq!(request("GET", "/api/position", None).1, "0");
        assert_eq!(request("POST", "/api/position", Some("1")).0, "200");
        assert_eq!(request("GET", "/api/position", None).1, "1");
        assert_eq!(request("POST", "/api/position", None).0, "403");
        assert_eq!(request("OPTIONS", "/api/position", None).0, "405");
        assert_eq!(request("POST", "/api/position", Some("invalid")).0, "400");
        assert_eq!(request("GET", "/api/position", None).1, "1");
    }
}
