//! Network cameras: frames read from a camera on the local network, either
//! an MJPEG stream held open or a snapshot endpoint polled. This is the only
//! outbound network path in the app, and it is fenced in hard — a frame is
//! fetched only from a loopback, private (RFC 1918), link-local, or `.local`
//! host, and redirects are never followed. Anything else is refused, so the
//! app never reaches the public internet.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Largest frame we will read, so a hostile or wrong endpoint can't stream
/// unbounded data into memory.
const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;

fn ip_is_local(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback() || v4.is_private() || v4.is_link_local()
        }
        // Loopback, unique-local (fc00::/7), and link-local (fe80::/10).
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || (v6.segments()[0] & 0xfe00) == 0xfc00
                || (v6.segments()[0] & 0xffc0) == 0xfe80
        }
    }
}

/// Whether a URL's host is on the local network. Public hosts are rejected.
pub fn host_is_local(url: &str) -> bool {
    let Ok(parsed) = reqwest::Url::parse(url) else {
        return false;
    };
    if !matches!(parsed.scheme(), "http" | "https") {
        return false;
    }
    let Some(host) = parsed.host_str() else {
        return false;
    };
    if let Ok(ip) = host.parse::<IpAddr>() {
        return ip_is_local(ip);
    }
    host == "localhost" || host.ends_with(".local")
}

const START_MARKER: [u8; 2] = [0xFF, 0xD8];
const END_MARKER: [u8; 2] = [0xFF, 0xD9];

fn find(haystack: &[u8], marker: [u8; 2]) -> Option<usize> {
    haystack.windows(2).position(|w| w == marker)
}

/// Pulls complete JPEGs (FF D8 … FF D9) out of a byte stream that arrives in
/// arbitrary chunks, such as a multipart MJPEG body. Holds at most one
/// partial frame, capped at `MAX_FRAME_BYTES`.
#[derive(Default)]
pub struct JpegExtractor {
    buffer: Vec<u8>,
    /// Where the end-marker search resumes, so each byte is scanned once.
    scanned: usize,
}

impl JpegExtractor {
    pub fn push(&mut self, chunk: &[u8]) -> Result<Vec<Vec<u8>>, String> {
        self.buffer.extend_from_slice(chunk);
        let mut frames = Vec::new();
        loop {
            // Everything before a start marker is multipart framing.
            match find(&self.buffer, START_MARKER) {
                Some(0) => {}
                Some(start) => {
                    self.buffer.drain(..start);
                    self.scanned = 0;
                }
                None => {
                    // A trailing FF may be the first half of a start marker.
                    let keep = usize::from(self.buffer.last() == Some(&0xFF));
                    self.buffer.drain(..self.buffer.len() - keep);
                    self.scanned = 0;
                    break;
                }
            }
            let from = self.scanned.max(START_MARKER.len());
            match find(&self.buffer[from..], END_MARKER) {
                Some(offset) => {
                    let end = from + offset + END_MARKER.len();
                    frames.push(self.buffer.drain(..end).collect());
                    self.scanned = 0;
                }
                None => {
                    // Step back one byte: a trailing FF may start the end marker.
                    self.scanned = self.buffer.len().saturating_sub(1);
                    break;
                }
            }
        }
        if self.buffer.len() > MAX_FRAME_BYTES {
            self.buffer.clear();
            self.scanned = 0;
            return Err("camera frame exceeds the size limit".to_string());
        }
        Ok(frames)
    }

    #[cfg(test)]
    fn buffered(&self) -> usize {
        self.buffer.len()
    }
}

const REFUSED_NOT_LOCAL: &str = "refused: camera URL must be on the local network";

#[derive(Clone, Copy)]
struct Timing {
    /// TCP connect, and waiting for the response head.
    connect: Duration,
    /// No new frame for this long counts as disconnected.
    idle: Duration,
    /// Pause before reconnecting after a failure.
    retry: Duration,
    /// A reader nobody asked for in this long stops (camera removed).
    unrequested: Duration,
    /// Gap between requests to a snapshot endpoint.
    snapshot_interval: Duration,
}

impl Default for Timing {
    fn default() -> Self {
        Timing {
            connect: Duration::from_secs(4),
            idle: Duration::from_secs(4),
            retry: Duration::from_secs(5),
            unrequested: Duration::from_secs(10),
            snapshot_interval: Duration::from_millis(100),
        }
    }
}

/// The newest frame from a camera; `seq` grows by one per frame so a reader
/// of the feed can skip a frame it already has.
#[derive(Clone, Debug)]
pub struct Frame {
    pub jpeg: Arc<Vec<u8>>,
    pub seq: u64,
}

struct Feed {
    latest: Option<Frame>,
    /// Set when the connection fails; cleared by the next frame.
    error: Option<String>,
    last_requested: Instant,
}

/// One background reader per camera URL, started on first request and
/// stopped once nobody has asked for that URL in a while. Readers keep only
/// the newest frame, so callers read memory, never the network.
#[derive(Clone, Default)]
pub struct Registry {
    feeds: Arc<Mutex<HashMap<String, Arc<Mutex<Feed>>>>>,
    timing: Timing,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    #[cfg(test)]
    fn with_timing(timing: Timing) -> Self {
        Registry {
            feeds: Arc::default(),
            timing,
        }
    }

    /// The newest frame for `url`: `Ok(None)` while the first connection is
    /// still being made, `Err` while the camera is unreachable. Must be
    /// called inside a tokio runtime (the first call spawns the reader).
    pub fn latest(&self, url: &str) -> Result<Option<Frame>, String> {
        if !host_is_local(url) {
            return Err(REFUSED_NOT_LOCAL.to_string());
        }
        let mut feeds = self.feeds.lock().unwrap();
        let feed = feeds
            .entry(url.to_string())
            .or_insert_with(|| {
                let feed = Arc::new(Mutex::new(Feed {
                    latest: None,
                    error: None,
                    last_requested: Instant::now(),
                }));
                tokio::spawn(read(self.clone(), url.to_string(), feed.clone()));
                feed
            })
            .clone();
        // Marked under the registry lock, so a reader can't retire this feed
        // between the lookup and the mark.
        let mut feed = feed.lock().unwrap();
        feed.last_requested = Instant::now();
        drop(feeds);
        match &feed.error {
            Some(error) => Err(error.clone()),
            None => Ok(feed.latest.clone()),
        }
    }

    /// Removes `feed` if nobody asked for it recently; true when removed.
    fn retire_if_unrequested(&self, url: &str, feed: &Arc<Mutex<Feed>>) -> bool {
        let mut feeds = self.feeds.lock().unwrap();
        if feed.lock().unwrap().last_requested.elapsed() < self.timing.unrequested {
            return false;
        }
        if feeds
            .get(url)
            .is_some_and(|current| Arc::ptr_eq(current, feed))
        {
            feeds.remove(url);
        }
        true
    }
}

fn publish(feed: &Mutex<Feed>, jpeg: Vec<u8>) {
    let mut feed = feed.lock().unwrap();
    let seq = feed.latest.as_ref().map_or(1, |frame| frame.seq + 1);
    feed.latest = Some(Frame {
        jpeg: Arc::new(jpeg),
        seq,
    });
    feed.error = None;
}

/// The reader task: (re)connects until the feed is retired.
async fn read(registry: Registry, url: String, feed: Arc<Mutex<Feed>>) {
    let timing = registry.timing;
    let client = reqwest::Client::builder()
        .connect_timeout(timing.connect)
        // A redirect would reach a host `host_is_local` never saw.
        .redirect(reqwest::redirect::Policy::none())
        .build();
    let client = match client {
        Ok(client) => client,
        Err(error) => {
            feed.lock().unwrap().error = Some(error.to_string());
            return;
        }
    };
    loop {
        if registry.retire_if_unrequested(&url, &feed) {
            return;
        }
        match connect_and_read(&registry, &client, &url, &feed).await {
            Ok(()) => return,
            Err(error) => feed.lock().unwrap().error = Some(error),
        }
        tokio::time::sleep(timing.retry).await;
    }
}

/// Opens `url` once and reads frames until it fails (`Err`) or the feed is
/// retired (`Ok`).
async fn connect_and_read(
    registry: &Registry,
    client: &reqwest::Client,
    url: &str,
    feed: &Arc<Mutex<Feed>>,
) -> Result<(), String> {
    let timing = registry.timing;
    let mut response = open(client, url, timing).await?;
    let is_multipart = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.contains("multipart"));

    if !is_multipart {
        // A snapshot endpoint: one image per request, polled.
        loop {
            publish(feed, read_body(&mut response, timing.idle).await?);
            tokio::time::sleep(timing.snapshot_interval).await;
            if registry.retire_if_unrequested(url, feed) {
                return Ok(());
            }
            response = open(client, url, timing).await?;
        }
    }

    let mut extractor = JpegExtractor::default();
    let mut last_frame = Instant::now();
    loop {
        if registry.retire_if_unrequested(url, feed) {
            return Ok(());
        }
        let wait = timing.idle.saturating_sub(last_frame.elapsed());
        let chunk = match tokio::time::timeout(wait, response.chunk()).await {
            Err(_) => {
                return Err(format!(
                    "no frame from the camera for {} s",
                    timing.idle.as_secs_f32()
                ))
            }
            Ok(Err(error)) => return Err(error.to_string()),
            Ok(Ok(None)) => return Err("camera closed the stream".to_string()),
            Ok(Ok(Some(chunk))) => chunk,
        };
        for jpeg in extractor.push(&chunk)? {
            publish(feed, jpeg);
            last_frame = Instant::now();
        }
    }
}

/// Sends one GET after re-checking the host; refuses redirects and errors.
async fn open(
    client: &reqwest::Client,
    url: &str,
    timing: Timing,
) -> Result<reqwest::Response, String> {
    if !host_is_local(url) {
        return Err(REFUSED_NOT_LOCAL.to_string());
    }
    let response = tokio::time::timeout(timing.connect, client.get(url).send())
        .await
        .map_err(|_| "camera did not answer".to_string())?
        .map_err(|e| e.to_string())?;
    if response.status().is_redirection() {
        return Err("refused: camera URL redirects elsewhere".to_string());
    }
    if !response.status().is_success() {
        return Err(format!("camera returned HTTP {}", response.status()));
    }
    Ok(response)
}

/// Reads a whole snapshot body, refusing more than `MAX_FRAME_BYTES`.
async fn read_body(response: &mut reqwest::Response, within: Duration) -> Result<Vec<u8>, String> {
    let mut body = Vec::new();
    let read = async {
        while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
            if body.len() + chunk.len() > MAX_FRAME_BYTES {
                return Err("camera frame exceeds the size limit".to_string());
            }
            body.extend_from_slice(&chunk);
        }
        Ok(())
    };
    tokio::time::timeout(within, read)
        .await
        .map_err(|_| "camera did not send the image".to_string())??;
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_local_and_private_hosts() {
        for url in [
            "http://127.0.0.1:8080/stream",
            "http://localhost/snapshot.jpg",
            "http://192.168.1.50:81/video",
            "http://10.0.0.9/mjpeg",
            "http://172.16.5.4/cam",
            "http://phone.local/stream",
        ] {
            assert!(host_is_local(url), "{url} should be local");
        }
    }

    #[test]
    fn rejects_public_hosts_and_bad_schemes() {
        for url in [
            "http://example.com/stream",
            "https://8.8.8.8/cam",
            "http://172.32.0.1/x", // outside 172.16/12
            "ftp://192.168.1.1/x",
            "not a url",
        ] {
            assert!(!host_is_local(url), "{url} should be refused");
        }
    }

    /// Serves `router` on an ephemeral loopback port; returns its base URL.
    async fn serve(router: axum::Router) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        format!("http://{addr}")
    }

    /// A two-frame multipart body; the second frame starts with FF D8 FF so
    /// a split inside it lands between marker bytes.
    fn two_frame_stream() -> (Vec<u8>, Vec<Vec<u8>>) {
        let frames = vec![
            vec![0xFF, 0xD8, 0x01, 0x02, 0xFF, 0xD9],
            vec![0xFF, 0xD8, 0xFF, 0xE0, 0x03, 0xFF, 0xD9],
        ];
        let mut body = Vec::new();
        for frame in &frames {
            body.extend_from_slice(b"--frame\r\nContent-Type: image/jpeg\r\n\r\n");
            body.extend_from_slice(frame);
            body.extend_from_slice(b"\r\n");
        }
        (body, frames)
    }

    #[test]
    fn extractor_yields_whole_frames_for_every_split_point() {
        let (body, frames) = two_frame_stream();
        for split in 0..=body.len() {
            let mut extractor = JpegExtractor::default();
            let mut got = extractor.push(&body[..split]).unwrap();
            got.extend(extractor.push(&body[split..]).unwrap());
            assert_eq!(got, frames, "split at byte {split}");
        }
    }

    #[test]
    fn extractor_yields_whole_frames_byte_by_byte() {
        let (body, frames) = two_frame_stream();
        let mut extractor = JpegExtractor::default();
        let mut got = Vec::new();
        for byte in &body {
            got.extend(extractor.push(std::slice::from_ref(byte)).unwrap());
        }
        assert_eq!(got, frames);
    }

    #[test]
    fn extractor_refuses_an_unbounded_frame() {
        let mut extractor = JpegExtractor::default();
        extractor.push(&[0xFF, 0xD8]).unwrap();
        let chunk = vec![0u8; 1024 * 1024];
        let mut result = Ok(Vec::new());
        for _ in 0..=MAX_FRAME_BYTES / chunk.len() {
            result = extractor.push(&chunk);
            if result.is_err() {
                break;
            }
        }
        assert!(
            result.is_err(),
            "a frame past the size limit must be refused"
        );
        assert!(extractor.buffered() <= MAX_FRAME_BYTES);
    }

    // --- Streaming reader -------------------------------------------------

    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    /// Short timings so the reader's timeouts run in milliseconds.
    fn fast() -> Registry {
        Registry::with_timing(Timing {
            connect: Duration::from_millis(500),
            idle: Duration::from_millis(300),
            retry: Duration::from_millis(200),
            unrequested: Duration::from_millis(500),
            snapshot_interval: Duration::from_millis(20),
        })
    }

    /// A raw MJPEG camera on loopback. Counts connections; each connection
    /// waits `latency`, then sends a frame every `interval`. The first
    /// connection stops after `first_connection_frames` frames (closing, or
    /// hanging when `hang` is set); later connections stream until dropped.
    async fn mjpeg_camera(
        latency: Duration,
        interval: Duration,
        first_connection_frames: Option<usize>,
        hang: bool,
    ) -> (String, Arc<AtomicUsize>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/video", listener.local_addr().unwrap());
        let connections = Arc::new(AtomicUsize::new(0));
        let counter = connections.clone();
        tokio::spawn(async move {
            loop {
                let (mut socket, _) = listener.accept().await.unwrap();
                let nth = counter.fetch_add(1, Ordering::SeqCst);
                tokio::spawn(async move {
                    let mut request = [0u8; 1024];
                    let _ = socket.read(&mut request).await;
                    let head = "HTTP/1.1 200 OK\r\nConnection: close\r\n\
                        Content-Type: multipart/x-mixed-replace; boundary=frame\r\n\r\n";
                    if socket.write_all(head.as_bytes()).await.is_err() {
                        return;
                    }
                    tokio::time::sleep(latency).await;
                    let limit = if nth == 0 {
                        first_connection_frames
                    } else {
                        None
                    };
                    let mut sent = 0;
                    while limit.is_none_or(|limit| sent < limit) {
                        let part = [
                            b"--frame\r\nContent-Type: image/jpeg\r\n\r\n".as_slice(),
                            &[0xFF, 0xD8, sent as u8, 0xFF, 0xD9],
                            b"\r\n",
                        ]
                        .concat();
                        if socket.write_all(&part).await.is_err() {
                            return;
                        }
                        sent += 1;
                        tokio::time::sleep(interval).await;
                    }
                    if hang {
                        tokio::time::sleep(Duration::from_secs(60)).await;
                    }
                });
            }
        });
        (url, connections)
    }

    /// Polls `registry` for `url` every 10 ms until `done` holds or `within`
    /// passes; returns the last result.
    async fn poll_until(
        registry: &Registry,
        url: &str,
        within: Duration,
        done: impl Fn(&Result<Option<Frame>, String>) -> bool,
    ) -> Result<Option<Frame>, String> {
        let deadline = tokio::time::Instant::now() + within;
        loop {
            let result = registry.latest(url);
            if done(&result) || tokio::time::Instant::now() >= deadline {
                return result;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    fn has_frame(result: &Result<Option<Frame>, String>) -> bool {
        matches!(result, Ok(Some(_)))
    }

    #[tokio::test]
    async fn streams_at_source_rate_over_one_connection() {
        let (url, connections) = mjpeg_camera(
            Duration::from_millis(150),
            Duration::from_millis(100),
            None,
            false,
        )
        .await;
        let registry = Registry::new();
        let first = poll_until(&registry, &url, Duration::from_secs(3), has_frame).await;
        assert!(has_frame(&first), "no frame within 3 s: {first:?}");

        let mut seen = std::collections::HashSet::new();
        let window = tokio::time::Instant::now() + Duration::from_secs(1);
        while tokio::time::Instant::now() < window {
            if let Ok(Some(frame)) = registry.latest(&url) {
                seen.insert(frame.seq);
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(seen.len() >= 9, "only {} frames in one second", seen.len());
        assert_eq!(connections.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn reports_a_closed_stream_and_recovers() {
        let (url, connections) =
            mjpeg_camera(Duration::ZERO, Duration::from_millis(20), Some(3), false).await;
        let registry = fast();
        let first = poll_until(&registry, &url, Duration::from_secs(2), has_frame).await;
        let first_seq = first.unwrap().unwrap().seq;

        let down = poll_until(&registry, &url, Duration::from_secs(1), |r| r.is_err()).await;
        assert!(down.is_err(), "a closed stream must be reported");

        let back = poll_until(
            &registry,
            &url,
            Duration::from_secs(2),
            |r| matches!(r, Ok(Some(frame)) if frame.seq > first_seq),
        )
        .await;
        assert!(has_frame(&back), "stream did not recover: {back:?}");
        assert_eq!(connections.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn reports_a_stream_that_stops_sending() {
        let (url, _) = mjpeg_camera(Duration::ZERO, Duration::from_millis(20), Some(1), true).await;
        let registry = fast();
        let first = poll_until(&registry, &url, Duration::from_secs(2), has_frame).await;
        assert!(has_frame(&first), "no first frame: {first:?}");
        let started = tokio::time::Instant::now();
        let down = poll_until(&registry, &url, Duration::from_secs(2), |r| r.is_err()).await;
        assert!(down.is_err(), "an idle stream must be reported");
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[tokio::test]
    async fn polls_a_snapshot_endpoint() {
        let hits = Arc::new(AtomicUsize::new(0));
        let counter = hits.clone();
        let base = serve(axum::Router::new().route(
            "/shot.jpg",
            axum::routing::get(move || {
                counter.fetch_add(1, Ordering::SeqCst);
                async {
                    (
                        [("content-type", "image/jpeg")],
                        vec![0xFF, 0xD8, 0xFF, 0xD9],
                    )
                }
            }),
        ))
        .await;
        let url = format!("{base}/shot.jpg");
        let registry = fast();
        let result = poll_until(
            &registry,
            &url,
            Duration::from_secs(2),
            |r| matches!(r, Ok(Some(frame)) if frame.seq >= 3),
        )
        .await;
        let frame = result.unwrap().unwrap();
        assert_eq!(*frame.jpeg, vec![0xFF, 0xD8, 0xFF, 0xD9]);
        assert!(hits.load(Ordering::SeqCst) >= 3);
    }

    #[tokio::test]
    async fn reader_refuses_a_redirect() {
        let hits = Arc::new(AtomicUsize::new(0));
        let counter = hits.clone();
        let target = serve(axum::Router::new().route(
            "/video",
            axum::routing::get(move || {
                counter.fetch_add(1, Ordering::SeqCst);
                async {
                    (
                        [("content-type", "image/jpeg")],
                        vec![0xFF, 0xD8, 0xFF, 0xD9],
                    )
                }
            }),
        ))
        .await;
        let camera = serve(axum::Router::new().route(
            "/video",
            axum::routing::get(move || async move {
                axum::response::Redirect::temporary(&format!("{target}/video"))
            }),
        ))
        .await;
        let url = format!("{camera}/video");
        let registry = fast();
        let result = poll_until(&registry, &url, Duration::from_secs(1), |r| r.is_err()).await;
        assert!(result.is_err(), "a redirect must be refused");
        assert_eq!(hits.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn refuses_a_public_host_without_connecting() {
        let registry = fast();
        assert!(registry.latest("http://8.8.8.8/video").is_err());
        assert!(registry.feeds.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn reader_stops_once_nobody_asks() {
        let (url, connections) =
            mjpeg_camera(Duration::ZERO, Duration::from_millis(20), None, false).await;
        let registry = fast();
        let first = poll_until(&registry, &url, Duration::from_secs(2), has_frame).await;
        assert!(has_frame(&first), "no first frame: {first:?}");
        tokio::time::sleep(Duration::from_millis(1000)).await;
        assert!(
            registry.feeds.lock().unwrap().is_empty(),
            "reader outlived its tile"
        );
        assert_eq!(connections.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn errors_never_carry_url_credentials() {
        // Nothing listens on port 1, so the connection is refused.
        let url = "http://user:secret@127.0.0.1:1/video";
        let registry = fast();
        let result = poll_until(&registry, url, Duration::from_secs(2), |r| r.is_err()).await;
        let error = result.expect_err("an unreachable camera must be reported");
        assert!(!error.contains("secret"), "credentials leaked: {error}");
    }
}
