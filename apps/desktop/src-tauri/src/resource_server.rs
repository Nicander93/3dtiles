//! Local read-only artifact HTTP server bound to 127.0.0.1.
//! Serves only registered artifact roots; path-traversal safe; CORS for Cesium.

use crate::artifact_store::ArtifactStore;
use axum::body::Body;
use axum::extract::{Path as AxumPath, Request, State};
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::oneshot;
use tower_http::cors::{Any, CorsLayer};

#[derive(Clone)]
struct ServerState {
  roots: Arc<parking_lot::RwLock<HashMap<String, PathBuf>>>,
}

pub struct ResourceServerHandle {
  pub port: u16,
  pub base_url: String,
  shutdown: Option<oneshot::Sender<()>>,
}

impl ResourceServerHandle {
  pub fn preview_url(&self, artifact_id: &str) -> String {
    format!(
      "{}/artifacts/{}/tileset.json",
      self.base_url.trim_end_matches('/'),
      artifact_id
    )
  }
}

impl Drop for ResourceServerHandle {
  fn drop(&mut self) {
    if let Some(tx) = self.shutdown.take() {
      let _ = tx.send(());
    }
  }
}

/// Bind `127.0.0.1:preferred_port` (0 = ephemeral). Spawns the axum server on a tokio runtime.
pub async fn start_resource_server(
  roots: Arc<parking_lot::RwLock<HashMap<String, PathBuf>>>,
  preferred_port: u16,
) -> Result<ResourceServerHandle, String> {
  let state = ServerState { roots };
  let cors = CorsLayer::new()
    .allow_origin(Any)
    .allow_methods([Method::GET, Method::HEAD, Method::OPTIONS])
    .allow_headers(Any);

  let app = Router::new()
    .route("/health", get(|| async { "ok" }))
    .route(
      "/artifacts/:id/*path",
      get(serve_artifact).head(serve_artifact),
    )
    .route(
      "/artifacts/:id/",
      get(|State(s): State<ServerState>, AxumPath(id): AxumPath<String>, req: Request| async move {
        serve_artifact_inner(s, id, "tileset.json".into(), req).await
      }),
    )
    .route(
      "/artifacts/:id",
      get(|State(s): State<ServerState>, AxumPath(id): AxumPath<String>, req: Request| async move {
        serve_artifact_inner(s, id, "tileset.json".into(), req).await
      }),
    )
    .layer(cors)
    .with_state(state);

  let addr = SocketAddr::from(([127, 0, 0, 1], preferred_port));
  let listener = tokio::net::TcpListener::bind(addr)
    .await
    .map_err(|e| format!("bind resource server failed: {e}"))?;
  let port = listener
    .local_addr()
    .map_err(|e| e.to_string())?
    .port();
  let base_url = format!("http://127.0.0.1:{port}");
  let (tx, rx) = oneshot::channel::<()>();

  tokio::spawn(async move {
    let server = axum::serve(listener, app).with_graceful_shutdown(async {
      let _ = rx.await;
    });
    if let Err(e) = server.await {
      log::error!("resource server error: {e}");
    }
  });

  log::info!("artifact resource server listening on {base_url}");
  Ok(ResourceServerHandle {
    port,
    base_url,
    shutdown: Some(tx),
  })
}

async fn serve_artifact(
  State(state): State<ServerState>,
  AxumPath((id, path)): AxumPath<(String, String)>,
  req: Request,
) -> Response {
  serve_artifact_inner(state, id, path, req).await
}

async fn serve_artifact_inner(
  state: ServerState,
  id: String,
  path: String,
  req: Request,
) -> Response {
  let root = {
    let map = state.roots.read();
    match map.get(&id) {
      Some(p) => p.clone(),
      None => {
        return (StatusCode::NOT_FOUND, format!("artifact not found: {id}")).into_response();
      }
    }
  };

  let file = match ArtifactStore::resolve_file(&root, &path) {
    Ok(p) => p,
    Err(e) => {
      let status = if e.contains("traversal") {
        StatusCode::BAD_REQUEST
      } else {
        StatusCode::NOT_FOUND
      };
      return (status, e).into_response();
    }
  };

  let meta = match tokio::fs::metadata(&file).await {
    Ok(m) => m,
    Err(e) => return (StatusCode::NOT_FOUND, e.to_string()).into_response(),
  };
  let len = meta.len();
  let mime = mime_guess::from_path(&file)
    .first_or_octet_stream()
    .essence_str()
    .to_string();

  let range_header = req
    .headers()
    .get(header::RANGE)
    .and_then(|v| v.to_str().ok())
    .map(|s| s.to_string());

  if let Some(range) = range_header {
    if let Some((start, end)) = parse_bytes_range(&range, len) {
      let to_read = end - start + 1;
      match read_range(&file, start, to_read).await {
        Ok(buf) => {
          let mut headers = HeaderMap::new();
          headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_str(&mime).unwrap_or(HeaderValue::from_static("application/octet-stream")),
          );
          headers.insert(
            header::CONTENT_LENGTH,
            HeaderValue::from_str(&to_read.to_string()).unwrap(),
          );
          headers.insert(
            header::CONTENT_RANGE,
            HeaderValue::from_str(&format!("bytes {start}-{end}/{len}")).unwrap(),
          );
          headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
          return Response::builder()
            .status(StatusCode::PARTIAL_CONTENT)
            .body(Body::from(buf))
            .map(|mut r| {
              *r.headers_mut() = headers;
              r
            })
            .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response());
        }
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
      }
    }
  }

  match tokio::fs::read(&file).await {
    Ok(buf) => {
      let mut headers = HeaderMap::new();
      headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&mime).unwrap_or(HeaderValue::from_static("application/octet-stream")),
      );
      headers.insert(
        header::CONTENT_LENGTH,
        HeaderValue::from_str(&len.to_string()).unwrap(),
      );
      headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
      Response::builder()
        .status(StatusCode::OK)
        .body(Body::from(buf))
        .map(|mut r| {
          *r.headers_mut() = headers;
          r
        })
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
    }
    Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
  }
}

fn parse_bytes_range(header: &str, len: u64) -> Option<(u64, u64)> {
  // bytes=START-END or bytes=START-
  let s = header.strip_prefix("bytes=")?;
  let (start_s, end_s) = s.split_once('-')?;
  let start: u64 = start_s.parse().ok()?;
  let end: u64 = if end_s.is_empty() {
    len.saturating_sub(1)
  } else {
    end_s.parse().ok()?
  };
  if start > end || start >= len {
    return None;
  }
  Some((start, end.min(len.saturating_sub(1))))
}

async fn read_range(path: &PathBuf, start: u64, len: u64) -> Result<Vec<u8>, String> {
  use tokio::io::{AsyncReadExt, AsyncSeekExt};
  let mut f = tokio::fs::File::open(path)
    .await
    .map_err(|e| e.to_string())?;
  f.seek(std::io::SeekFrom::Start(start))
    .await
    .map_err(|e| e.to_string())?;
  let mut buf = vec![0u8; len as usize];
  f.read_exact(&mut buf).await.map_err(|e| e.to_string())?;
  Ok(buf)
}
