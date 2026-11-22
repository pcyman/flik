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
}

impl PreviewServer {
    pub(crate) fn bind(path: PathBuf) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
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
        };
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let host = listener.local_addr()?.to_string();
        let server = Server::from_listener(listener, None)?;
        Ok(Self { server, host, deck })
    }

    pub(crate) fn url(&self) -> String {
        format!("http://{}/", self.host)
    }

    pub(crate) fn serve(mut self) {
        for request in self.server.incoming_requests() {
            self.deck.serve(request, &self.host);
        }
    }
}

struct DeckState {
    path: PathBuf,
    root: PathBuf,
    title: String,
    cached: Option<Snapshot>,
}

struct Snapshot {
    source: String,
    deck: flik::Deck,
    assets: HashMap<String, PathBuf>,
}

impl DeckState {
    fn deck(&mut self) -> String {
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
            return self.response(cached);
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
                let response = self.response(&snapshot);
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

    fn response(&self, snapshot: &Snapshot) -> String {
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
        json!({"deck": snapshot.deck, "imageRevisions": revisions}).to_string()
    }

    fn serve(&mut self, request: Request, host: &str) {
        let headers = request.headers().iter().filter(|h| h.field.equiv("Host"));
        if !headers.map(|h| h.value.as_str()).eq([host]) {
            send_text(request, 403, "text/plain; charset=utf-8", "Forbidden host");
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
            "/" => send_text(
                request,
                200,
                "text/html; charset=utf-8",
                include_str!("../../web/index.html"),
            ),
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
            "/api/deck" => send_text(request, 200, "application/json; charset=utf-8", self.deck()),
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
