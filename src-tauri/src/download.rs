use reqwest::header::{CONTENT_LENGTH, CONTENT_RANGE, RANGE, USER_AGENT};
use reqwest::{Client, StatusCode};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tempfile::NamedTempFile;
use tokio::fs::{self, OpenOptions};
use tokio::io::{AsyncSeekExt, AsyncWriteExt, SeekFrom};
use tokio::task::JoinSet;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const SOURCE_TIMEOUT: Duration = Duration::from_secs(120);
const RETRY_LIMIT: usize = 3;
const MIN_RANGE_BYTES: u64 = 1024 * 1024;
const PROGRESS_STEP_BYTES: u64 = 256 * 1024;

pub(crate) struct DownloadRequest {
    pub urls: Vec<String>,
    pub destination: PathBuf,
    pub expected_size: Option<u64>,
    pub max_size: u64,
    pub concurrency: usize,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct DownloadProgress {
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
}

struct RemoteFile {
    total_size: Option<u64>,
    supports_ranges: bool,
}

struct ProgressReporter {
    downloaded: AtomicU64,
    last_emitted: AtomicU64,
    total: Option<u64>,
    max_size: u64,
    callback: Arc<dyn Fn(DownloadProgress) + Send + Sync>,
}

impl ProgressReporter {
    fn new(
        total: Option<u64>,
        max_size: u64,
        callback: Arc<dyn Fn(DownloadProgress) + Send + Sync>,
    ) -> Self {
        callback(DownloadProgress {
            downloaded_bytes: 0,
            total_bytes: total,
        });
        Self {
            downloaded: AtomicU64::new(0),
            last_emitted: AtomicU64::new(0),
            total,
            max_size,
            callback,
        }
    }

    fn add(&self, bytes: u64) -> Result<(), String> {
        let downloaded = self.downloaded.fetch_add(bytes, Ordering::Relaxed) + bytes;
        if downloaded > self.max_size {
            return Err("download exceeds the configured size limit".to_owned());
        }
        let last = self.last_emitted.load(Ordering::Relaxed);
        let crossed_percent = self.total.is_some_and(|total| {
            total > 0 && downloaded.saturating_mul(100) / total > last.saturating_mul(100) / total
        });
        if crossed_percent || downloaded.saturating_sub(last) >= PROGRESS_STEP_BYTES {
            self.emit(downloaded);
        }
        Ok(())
    }

    fn reset(&self) {
        self.downloaded.store(0, Ordering::Relaxed);
        self.last_emitted.store(0, Ordering::Relaxed);
        (self.callback)(DownloadProgress {
            downloaded_bytes: 0,
            total_bytes: self.total,
        });
    }

    fn finish(&self) {
        self.emit(self.downloaded.load(Ordering::Relaxed));
    }

    fn emit(&self, downloaded: u64) {
        self.last_emitted.store(downloaded, Ordering::Relaxed);
        (self.callback)(DownloadProgress {
            downloaded_bytes: downloaded,
            total_bytes: self.total,
        });
    }
}

pub(crate) async fn download<F>(request: DownloadRequest, on_progress: F) -> Result<(), String>
where
    F: Fn(DownloadProgress) + Send + Sync + 'static,
{
    if request.urls.is_empty() {
        return Err("no download source is available".to_owned());
    }
    if request.concurrency == 0 {
        return Err("download concurrency must be positive".to_owned());
    }
    if request
        .expected_size
        .is_some_and(|size| size > request.max_size)
    {
        return Err("download exceeds the configured size limit".to_owned());
    }

    let parent = request
        .destination
        .parent()
        .ok_or("download destination has no parent directory")?;
    fs::create_dir_all(parent)
        .await
        .map_err(|error| error.to_string())?;
    let temporary = NamedTempFile::new_in(parent)
        .map_err(|error| error.to_string())?
        .into_temp_path();
    let temporary_path = temporary.to_path_buf();
    let callback: Arc<dyn Fn(DownloadProgress) + Send + Sync> = Arc::new(on_progress);
    let client = Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|error| error.to_string())?;
    let mut failures = Vec::new();

    for url in &request.urls {
        log::info!("downloading from {url}");
        let result = tokio::time::timeout(
            SOURCE_TIMEOUT,
            fetch(
                &client,
                url,
                &temporary_path,
                request.expected_size,
                request.max_size,
                request.concurrency,
                Arc::clone(&callback),
            ),
        )
        .await
        .map_err(|_| "download source timed out".to_owned())
        .and_then(|result| result);

        match result {
            Ok(()) => {
                temporary
                    .persist(&request.destination)
                    .map_err(|error| error.error.to_string())?;
                return Ok(());
            }
            Err(error) => {
                log::warn!("download source {url} failed: {error}");
                failures.push(error);
            }
        }
    }

    Err(format!(
        "all download sources failed: {}",
        failures.join("; ")
    ))
}

async fn fetch(
    client: &Client,
    url: &str,
    path: &Path,
    expected_size: Option<u64>,
    max_size: u64,
    concurrency: usize,
    callback: Arc<dyn Fn(DownloadProgress) + Send + Sync>,
) -> Result<(), String> {
    let remote = probe(client, url).await?;
    if let (Some(remote_size), Some(expected_size)) = (remote.total_size, expected_size)
        && remote_size != expected_size
    {
        return Err(format!(
            "remote size {remote_size} does not match expected size {expected_size}"
        ));
    }
    let total_size = remote.total_size.or(expected_size);
    if total_size.is_some_and(|size| size > max_size) {
        return Err("download exceeds the configured size limit".to_owned());
    }
    let reporter = Arc::new(ProgressReporter::new(total_size, max_size, callback));

    if remote.supports_ranges
        && let Some(total) = total_size
        && total > 0
    {
        fetch_parts(client, url, path, total, concurrency, &reporter).await?;
    } else {
        fetch_single(client, url, path, &reporter).await?;
    }

    let actual_size = fs::metadata(path)
        .await
        .map_err(|error| error.to_string())?
        .len();
    if let Some(expected) = total_size
        && actual_size != expected
    {
        return Err(format!(
            "downloaded size {actual_size} does not match expected size {expected}"
        ));
    }
    reporter.finish();
    Ok(())
}

async fn probe(client: &Client, url: &str) -> Result<RemoteFile, String> {
    let response = client
        .get(url)
        .header(USER_AGENT, "SeaLantern-Connect/0.6")
        .header(RANGE, "bytes=0-0")
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("download probe returned {}", response.status()));
    }

    if response.status() == StatusCode::PARTIAL_CONTENT {
        let total_size = range_size(
            response
                .headers()
                .get(CONTENT_RANGE)
                .and_then(|value| value.to_str().ok()),
        )?;
        Ok(RemoteFile {
            total_size: Some(total_size),
            supports_ranges: true,
        })
    } else {
        Ok(RemoteFile {
            total_size: response
                .headers()
                .get(CONTENT_LENGTH)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse().ok()),
            supports_ranges: false,
        })
    }
}

fn range_size(value: Option<&str>) -> Result<u64, String> {
    value
        .and_then(|value| value.rsplit('/').next())
        .filter(|value| *value != "*")
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| "range response has an invalid Content-Range header".to_owned())
}

async fn fetch_parts(
    client: &Client,
    url: &str,
    path: &Path,
    total_size: u64,
    concurrency: usize,
    reporter: &Arc<ProgressReporter>,
) -> Result<(), String> {
    let range_count = concurrency
        .min(total_size.div_ceil(MIN_RANGE_BYTES) as usize)
        .max(1);
    let ranges = ranges(total_size, range_count);
    let file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(path)
        .await
        .map_err(|error| error.to_string())?;
    file.set_len(total_size)
        .await
        .map_err(|error| error.to_string())?;
    drop(file);

    let mut tasks = JoinSet::new();
    for (start, end) in ranges {
        let client = client.clone();
        let url = url.to_owned();
        let path = path.to_owned();
        let reporter = Arc::clone(reporter);
        tasks.spawn(async move { fetch_part(&client, &url, &path, start, end, &reporter).await });
    }

    while let Some(result) = tasks.join_next().await {
        match result {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                tasks.abort_all();
                while tasks.join_next().await.is_some() {}
                return Err(error);
            }
            Err(error) => {
                tasks.abort_all();
                while tasks.join_next().await.is_some() {}
                return Err(format!("download worker failed: {error}"));
            }
        }
    }
    Ok(())
}

async fn fetch_part(
    client: &Client,
    url: &str,
    path: &Path,
    start: u64,
    end: u64,
    reporter: &ProgressReporter,
) -> Result<(), String> {
    let mut next = start;
    let mut failures = 0;
    let mut file = OpenOptions::new()
        .write(true)
        .open(path)
        .await
        .map_err(|error| error.to_string())?;

    while next <= end {
        let response = client
            .get(url)
            .header(USER_AGENT, "SeaLantern-Connect/0.6")
            .header(RANGE, format!("bytes={next}-{end}"))
            .send()
            .await;
        let mut response = match response {
            Ok(response) if response.status() == StatusCode::PARTIAL_CONTENT => response,
            Ok(response) => {
                return Err(format!(
                    "range request returned {} instead of 206",
                    response.status()
                ));
            }
            Err(error) => {
                failures += 1;
                if failures >= RETRY_LIMIT {
                    return Err(error.to_string());
                }
                continue;
            }
        };

        file.seek(SeekFrom::Start(next))
            .await
            .map_err(|error| error.to_string())?;
        let mut received = 0;
        let mut stream_error = None;
        loop {
            match response.chunk().await {
                Ok(Some(chunk)) => {
                    if next + received + chunk.len() as u64 > end + 1 {
                        return Err("range response exceeded the requested boundary".to_owned());
                    }
                    file.write_all(&chunk)
                        .await
                        .map_err(|error| error.to_string())?;
                    received += chunk.len() as u64;
                    reporter.add(chunk.len() as u64)?;
                }
                Ok(None) => break,
                Err(error) => {
                    stream_error = Some(error.to_string());
                    break;
                }
            }
        }
        next += received;
        if let Some(error) = stream_error {
            failures += 1;
            if failures >= RETRY_LIMIT {
                return Err(error);
            }
            continue;
        }
        if received == 0 && next <= end {
            failures += 1;
            if failures >= RETRY_LIMIT {
                return Err("range response ended before the requested bytes arrived".to_owned());
            }
        } else {
            failures = 0;
        }
    }
    file.flush().await.map_err(|error| error.to_string())
}

async fn fetch_single(
    client: &Client,
    url: &str,
    path: &Path,
    reporter: &ProgressReporter,
) -> Result<(), String> {
    let mut last_error = String::new();
    for attempt in 0..RETRY_LIMIT {
        if attempt > 0 {
            reporter.reset();
        }
        let result = fetch_once(client, url, path, reporter).await;
        match result {
            Ok(()) => return Ok(()),
            Err(error) => last_error = error,
        }
    }
    Err(last_error)
}

async fn fetch_once(
    client: &Client,
    url: &str,
    path: &Path,
    reporter: &ProgressReporter,
) -> Result<(), String> {
    let mut response = client
        .get(url)
        .header(USER_AGENT, "SeaLantern-Connect/0.6")
        .send()
        .await
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| error.to_string())?;
    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(path)
        .await
        .map_err(|error| error.to_string())?;
    while let Some(chunk) = response.chunk().await.map_err(|error| error.to_string())? {
        reporter.add(chunk.len() as u64)?;
        file.write_all(&chunk)
            .await
            .map_err(|error| error.to_string())?;
    }
    file.flush().await.map_err(|error| error.to_string())
}

fn ranges(total_size: u64, count: usize) -> Vec<(u64, u64)> {
    let count = count.min(total_size as usize).max(1);
    let base_size = total_size / count as u64;
    (0..count)
        .map(|index| {
            let start = index as u64 * base_size;
            let end = if index + 1 == count {
                total_size - 1
            } else {
                start + base_size - 1
            };
            (start, end)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::atomic::{AtomicBool, Ordering};

    struct TestServer {
        address: std::net::SocketAddr,
        stopped: Arc<AtomicBool>,
        thread: Option<std::thread::JoinHandle<()>>,
    }

    impl TestServer {
        fn start(payload: Vec<u8>) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let stopped = Arc::new(AtomicBool::new(false));
            let server_stopped = Arc::clone(&stopped);
            let payload = Arc::new(payload);
            let thread = std::thread::spawn(move || {
                for stream in listener.incoming() {
                    if server_stopped.load(Ordering::Relaxed) {
                        break;
                    }
                    let payload = Arc::clone(&payload);
                    std::thread::spawn(move || serve_range(stream.unwrap(), &payload));
                }
            });
            Self {
                address,
                stopped,
                thread: Some(thread),
            }
        }

        fn url(&self) -> String {
            format!("http://{}/client.bin", self.address)
        }
    }

    impl Drop for TestServer {
        fn drop(&mut self) {
            self.stopped.store(true, Ordering::Relaxed);
            let _ = TcpStream::connect(self.address);
            if let Some(thread) = self.thread.take() {
                thread.join().unwrap();
            }
        }
    }

    fn serve_range(mut stream: TcpStream, payload: &[u8]) {
        let mut request = Vec::new();
        let mut buffer = [0u8; 1024];
        while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
            let read = stream.read(&mut buffer).unwrap();
            if read == 0 {
                return;
            }
            request.extend_from_slice(&buffer[..read]);
        }
        let request = String::from_utf8_lossy(&request);
        let range = request
            .lines()
            .find_map(|line| line.strip_prefix("range: bytes="))
            .or_else(|| {
                request
                    .lines()
                    .find_map(|line| line.strip_prefix("Range: bytes="))
            })
            .unwrap();
        let (start, end) = range.split_once('-').unwrap();
        let start: usize = start.parse().unwrap();
        let end: usize = if end.is_empty() {
            payload.len() - 1
        } else {
            end.parse().unwrap()
        };
        let body = &payload[start..=end];
        write!(
            stream,
            "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {start}-{end}/{}\r\nConnection: close\r\n\r\n",
            body.len(),
            payload.len()
        )
        .unwrap();
        stream.write_all(body).unwrap();
    }

    #[test]
    fn ranges_cover_file() {
        assert_eq!(ranges(10, 3), vec![(0, 2), (3, 5), (6, 9)]);
    }

    #[test]
    fn parses_range_size() {
        assert_eq!(range_size(Some("bytes 0-0/1234")), Ok(1234));
        assert!(range_size(Some("bytes 0-0/*")).is_err());
    }

    #[tokio::test]
    async fn mirror_fallback() {
        let payload: Vec<u8> = (0..3 * 1024 * 1024)
            .map(|index| (index % 251) as u8)
            .collect();
        let server = TestServer::start(payload.clone());
        let unavailable = TcpListener::bind("127.0.0.1:0").unwrap();
        let unavailable_url = format!("http://{}/client.bin", unavailable.local_addr().unwrap());
        drop(unavailable);
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("client.bin");

        download(
            DownloadRequest {
                urls: vec![unavailable_url, server.url()],
                destination: destination.clone(),
                expected_size: Some(payload.len() as u64),
                max_size: 4 * 1024 * 1024,
                concurrency: 4,
            },
            |_| {},
        )
        .await
        .unwrap();

        assert_eq!(std::fs::read(destination).unwrap(), payload);
    }
}
