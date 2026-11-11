use morphrpc::{
    protocol::{self, Frame, MessageKind},
    runtime::Client,
};
use morphrpc_bench::{
    get_user, morph_request, morph_response,
    proto::{self, users_client::UsersClient},
    Query, UserData, MAX_DATA,
};
use prost::Message;
use serde::{Deserialize, Serialize};
use std::{
    error::Error,
    fs,
    net::{SocketAddr, TcpListener},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{Arc, OnceLock},
    time::{Duration, Instant},
};
use tokio::{
    sync::Barrier,
    task::JoinSet,
    time::{sleep, timeout},
};

const DEADLINE: Duration = Duration::from_secs(30);
type AnyError = Box<dyn Error + Send + Sync>;

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Profile {
    name: String,
    payload_size: u32,
}
#[derive(Debug, Deserialize, Serialize)]
struct Config {
    requests: usize,
    warmup_requests: usize,
    repetitions: usize,
    concurrency: Vec<usize>,
    profiles: Vec<Profile>,
    target_rps: Option<u32>,
}
impl Config {
    fn validate(&self) -> Result<(), AnyError> {
        if self.requests == 0
            || self.warmup_requests == 0
            || self.repetitions == 0
            || self.concurrency.is_empty()
            || self.profiles.is_empty()
        {
            return Err("counts, concurrency, and profiles must be nonzero".into());
        }
        if self
            .concurrency
            .iter()
            .any(|&n| n == 0 || n > 4096 || n > self.requests || n > self.warmup_requests)
        {
            return Err(
                "concurrency must be positive, at most 4096, and at most both request counts"
                    .into(),
            );
        }
        if self.target_rps == Some(0) {
            return Err("target_rps must be positive".into());
        }
        if self
            .profiles
            .iter()
            .any(|p| p.name.is_empty() || p.payload_size as usize > MAX_DATA)
        {
            return Err("profile names must be nonempty and payload size at most 900000".into());
        }
        Ok(())
    }
}

struct Server(Child);
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
async fn start_server(protocol: &str) -> Result<(Server, SocketAddr), AnyError> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let address = listener.local_addr()?;
    drop(listener);
    let child = Command::new(std::env::current_exe()?)
        .args(["server", protocol, &address.to_string()])
        .stdout(Stdio::null())
        .spawn()?;
    let mut server = Server(child);
    timeout(Duration::from_secs(10), async {
        loop {
            if let Some(status) = server.0.try_wait()? {
                return Err::<(), AnyError>(format!("{protocol} server exited: {status}").into());
            }
            if tokio::net::TcpStream::connect(address).await.is_ok() {
                return Ok(());
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await??;
    Ok((server, address))
}

enum Connection {
    Morph(Client),
    Http(reqwest::Client, String),
    Grpc(UsersClient<tonic::transport::Channel>),
}
impl Connection {
    async fn connect(protocol: &str, address: SocketAddr) -> Result<Self, AnyError> {
        match protocol {
            "morphrpc" => Ok(Self::Morph(Client::connect(address, DEADLINE).await?)),
            "http" => Ok(Self::Http(
                reqwest::Client::builder()
                    .no_proxy()
                    .http1_only()
                    .tcp_nodelay(false)
                    .timeout(DEADLINE)
                    .build()?,
                format!("http://{address}/get_user"),
            )),
            "grpc" => {
                let channel = tonic::transport::Endpoint::from_shared(format!("http://{address}"))?
                    .tcp_nodelay(false)
                    .connect_timeout(DEADLINE)
                    .timeout(DEADLINE)
                    .connect()
                    .await?;
                Ok(Self::Grpc(UsersClient::new(channel)))
            }
            _ => Err("unknown protocol".into()),
        }
    }
    async fn call(&mut self, query: &Query) -> Result<UserData, AnyError> {
        match self {
            Self::Morph(client) => {
                let response = client.call(1, morph_request(query)).await?;
                if response.kind != MessageKind::Response {
                    return Err("MorphRPC returned an error".into());
                }
                Ok(morph_response(&response.payload)?)
            }
            Self::Http(client, url) => Ok(client
                .post(url.as_str())
                .json(query)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?),
            Self::Grpc(client) => {
                let response = client
                    .get_user(proto::GetUserRequest {
                        id: query.id,
                        payload_size: query.payload_size,
                    })
                    .await?
                    .into_inner();
                Ok(UserData {
                    id: response.id,
                    name: response.name,
                    data: response.data,
                })
            }
        }
    }
}

#[derive(Serialize)]
struct Row {
    protocol: String,
    profile: String,
    payload_size: u32,
    concurrency: usize,
    repetition: usize,
    requests: usize,
    warmup_requests: usize,
    target_rps: Option<u32>,
    scheduled_p50_us: Option<f64>,
    scheduled_p95_us: Option<f64>,
    scheduled_p99_us: Option<f64>,
    elapsed_seconds: f64,
    throughput_rps: f64,
    p50_us: f64,
    p95_us: f64,
    p99_us: f64,
    request_body_bytes: usize,
    response_body_bytes: usize,
    request_framing_bytes: Option<usize>,
    response_framing_bytes: Option<usize>,
    transport_bytes: Option<usize>,
    cpu_usage: Option<f64>,
    server_rss_bytes: Option<u64>,
}
fn sizes(
    protocol: &str,
    query: &Query,
    expected: &UserData,
) -> Result<(usize, usize, Option<usize>), AnyError> {
    match protocol {
        "morphrpc" => {
            let (_, response) = morphrpc_bench::morph_handler(1, &morph_request(query));
            let frame = Frame {
                kind: MessageKind::Response,
                operation: 1,
                request_id: 0,
                payload: response,
            };
            Ok((
                morph_request(query).len(),
                protocol::encode(&frame)?.len() - 16,
                Some(16),
            ))
        }
        "http" => Ok((
            serde_json::to_vec(query)?.len(),
            serde_json::to_vec(expected)?.len(),
            None,
        )),
        "grpc" => Ok((
            proto::GetUserRequest {
                id: query.id,
                payload_size: query.payload_size,
            }
            .encoded_len(),
            proto::User {
                id: expected.id,
                name: expected.name.clone(),
                data: expected.data.clone(),
            }
            .encoded_len(),
            Some(5),
        )),
        _ => Err("unknown protocol".into()),
    }
}
fn share(total: usize, workers: usize, index: usize) -> usize {
    total / workers + usize::from(index < total % workers)
}
fn percentile(sorted: &[f64], percent: usize) -> f64 {
    sorted[(sorted.len() * percent).div_ceil(100) - 1]
}

async fn measure(
    protocol: &str,
    address: SocketAddr,
    cfg: &Config,
    profile: &Profile,
    concurrency: usize,
    repetition: usize,
    server_pid: u32,
) -> Result<Row, AnyError> {
    let query = Query {
        id: 1,
        payload_size: profile.payload_size,
    };
    let expected = get_user(&query)?;
    let barrier = Arc::new(Barrier::new(concurrency + 1));
    let mut workers = JoinSet::new();
    let scheduled_start = Arc::new(OnceLock::<Instant>::new());
    let (ready_tx, mut ready_rx) = tokio::sync::mpsc::channel(concurrency);
    for index in 0..concurrency {
        let mut connection = Connection::connect(protocol, address).await?;
        let query = query.clone();
        let expected = expected.clone();
        let barrier = barrier.clone();
        let ready_tx = ready_tx.clone();
        let scheduled_start = scheduled_start.clone();
        let target_rps = cfg.target_rps;
        let warmup = share(cfg.warmup_requests, concurrency, index);
        let requests = share(cfg.requests, concurrency, index);
        workers.spawn(async move {
            for _ in 0..warmup {
                if connection.call(&query).await? != expected {
                    return Err::<(Vec<f64>, Vec<f64>), AnyError>(
                        "warmup response mismatch".into(),
                    );
                }
            }
            ready_tx.send(()).await.map_err(|_| "benchmark stopped")?;
            barrier.wait().await;
            let mut latencies = Vec::with_capacity(requests);
            let mut scheduled_latencies = Vec::with_capacity(requests);
            for request_index in 0..requests {
                let scheduled = target_rps.map(|rate| {
                    let ordinal = index + request_index * concurrency;
                    *scheduled_start.get().expect("start set before barrier")
                        + Duration::from_secs_f64(ordinal as f64 / rate as f64)
                });
                if let Some(at) = scheduled {
                    tokio::time::sleep_until(at.into()).await;
                }
                let start = Instant::now();
                let result = connection.call(&query).await?;
                let end = Instant::now();
                latencies.push(end.duration_since(start).as_secs_f64() * 1_000_000.0);
                if let Some(at) = scheduled {
                    scheduled_latencies.push(end.duration_since(at).as_secs_f64() * 1_000_000.0);
                }
                if result != expected {
                    return Err("response mismatch".into());
                }
            }
            Ok((latencies, scheduled_latencies))
        });
    }
    drop(ready_tx);
    for _ in 0..concurrency {
        timeout(DEADLINE, ready_rx.recv())
            .await
            .map_err(|_| "warmup timed out")?
            .ok_or("warmup failed")?;
    }
    let start = Instant::now();
    scheduled_start.set(start).expect("start set once");
    barrier.wait().await;
    let mut scheduled_latencies = Vec::new();
    let mut latencies = Vec::with_capacity(cfg.requests);
    while let Some(result) = workers.join_next().await {
        let (service, scheduled) = result??;
        latencies.extend(service);
        scheduled_latencies.extend(scheduled);
    }
    let elapsed = start.elapsed().as_secs_f64();
    latencies.sort_unstable_by(f64::total_cmp);
    scheduled_latencies.sort_unstable_by(f64::total_cmp);
    let (request, response, framing) = sizes(protocol, &query, &expected)?;
    Ok(Row {
        protocol: protocol.into(),
        profile: profile.name.clone(),
        payload_size: profile.payload_size,
        concurrency,
        repetition,
        requests: cfg.requests,
        warmup_requests: cfg.warmup_requests,
        target_rps: cfg.target_rps,
        scheduled_p50_us: cfg.target_rps.map(|_| percentile(&scheduled_latencies, 50)),
        scheduled_p95_us: cfg.target_rps.map(|_| percentile(&scheduled_latencies, 95)),
        scheduled_p99_us: cfg.target_rps.map(|_| percentile(&scheduled_latencies, 99)),
        elapsed_seconds: elapsed,
        throughput_rps: cfg.requests as f64 / elapsed,
        p50_us: percentile(&latencies, 50),
        p95_us: percentile(&latencies, 95),
        p99_us: percentile(&latencies, 99),
        request_body_bytes: request,
        response_body_bytes: response,
        request_framing_bytes: framing,
        response_framing_bytes: framing,
        transport_bytes: None,
        cpu_usage: None,
        server_rss_bytes: server_rss(server_pid),
    })
}
fn server_rss(pid: u32) -> Option<u64> {
    if !cfg!(any(target_os = "linux", target_os = "macos")) {
        return None;
    }
    command_output("ps", &["-o", "rss=", "-p", &pid.to_string()])?
        .parse::<u64>()
        .ok()?
        .checked_mul(1024)
}

fn command_output(command: &str, args: &[&str]) -> Option<String> {
    let result = Command::new(command).args(args).output().ok()?;
    result
        .status
        .success()
        .then(|| String::from_utf8_lossy(&result.stdout).trim().to_owned())
}
#[derive(Serialize)]
struct Report {
    config: Config,
    timestamp_utc: Option<String>,
    os: Option<String>,
    cpu: Option<String>,
    rustc: Option<String>,
    revision: Option<String>,
    working_tree_dirty: Option<bool>,
    cargo_lock: &'static str,
    rows: Vec<Row>,
}

#[tokio::main]
async fn main() -> Result<(), AnyError> {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("server") {
        return morphrpc_bench::serve(
            args.get(2).ok_or("missing protocol")?,
            args.get(3).ok_or("missing address")?.parse()?,
        )
        .await;
    }
    if args.len() != 4 || args[1] != "run" {
        return Err("usage: morphrpc-bench run CONFIG.toml OUTPUT_DIRECTORY".into());
    }
    if cfg!(debug_assertions) {
        return Err("use a release build for benchmarks".into());
    }
    let cfg: Config = toml::from_str(&fs::read_to_string(&args[2])?)?;
    cfg.validate()?;
    let out = PathBuf::from(&args[3]);
    fs::create_dir_all(&out)?;
    let mut servers = Vec::new();
    for name in ["morphrpc", "grpc", "http"] {
        let (server, addr) = start_server(name).await?;
        servers.push((name, server, addr));
    }
    let mut rows = Vec::new();
    for repetition in 0..cfg.repetitions {
        for profile in &cfg.profiles {
            for &concurrency in &cfg.concurrency {
                for offset in 0..servers.len() {
                    let (name, server, addr) = &servers[(offset + repetition) % servers.len()];
                    eprintln!(
                        "{name} {} concurrency={concurrency} repetition={}",
                        profile.name,
                        repetition + 1
                    );
                    rows.push(
                        measure(
                            name,
                            *addr,
                            &cfg,
                            profile,
                            concurrency,
                            repetition + 1,
                            server.0.id(),
                        )
                        .await?,
                    );
                }
            }
        }
    }
    let cpu = command_output("sysctl", &["-n", "machdep.cpu.brand_string"]).or_else(|| {
        fs::read_to_string("/proc/cpuinfo").ok().and_then(|s| {
            s.lines()
                .find(|line| line.starts_with("model name") || line.starts_with("Hardware"))
                .map(str::to_owned)
        })
    });
    let report = Report {
        config: cfg,
        timestamp_utc: command_output("date", &["-u", "+%Y-%m-%dT%H:%M:%SZ"]),
        os: command_output("uname", &["-a"]),
        cpu,
        rustc: command_output("rustc", &["--version"]),
        revision: command_output("git", &["rev-parse", "HEAD"]),
        working_tree_dirty: command_output("git", &["status", "--porcelain"])
            .map(|s| !s.is_empty()),
        cargo_lock: include_str!("../../Cargo.lock"),
        rows,
    };
    fs::write(
        out.join("results.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    let mut csv = csv::Writer::from_path(out.join("results.csv"))?;
    for row in &report.rows {
        csv.serialize(row)?;
    }
    csv.flush()?;
    println!(
        "Wrote {} measurements to {}",
        report.rows.len(),
        out.display()
    );
    Ok(())
}
