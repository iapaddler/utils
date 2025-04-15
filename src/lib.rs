use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use reqwest::Client;
use serde::Serialize;
use std::env;
use std::fs;
use std::io::Write;
use std::io::{stderr, stdout};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process;
use std::sync::{mpsc, Arc, LazyLock, Mutex};

const NOTIFY_URL: &str = "https://slack.com/api/chat.postMessage";
const NOTIFY_CHANNEL: &str = "#drn";
const NOTIFY_ENV_VAR: &str = "APPVIEW_SLACKBOT_TOKEN";
pub const PERIOD: u64 = 5;
const MAX_ENTRIES: usize = 288; // Assuming 5 mins per measurement, gives us 24 hours of data
const EXPORT_HOST: &str = "default.main.musing-faraday-83adewh.cribl.cloud:20000";
pub const HW1: &str = "/dev/ttyUSB0";
pub const HW2: &str = "/dev/i2c-1";
pub const TEST_DATA: &str = "/tmp/sensor.dat";
pub const DBG: LogLevel = LogLevel::Debug;
pub const ERR: LogLevel = LogLevel::Error;
pub const INF: LogLevel = LogLevel::Info;
pub const WAR: LogLevel = LogLevel::Warn;

// Could use features. Too confusing
// DEBUG:
//pub const NUM_MEASUREMENTS: i32 = 2;
//pub const NUM_RUNS: i32 = 7;

pub const NUM_MEASUREMENTS: i32 = 12; // report every 1 hour
pub const NUM_RUNS: i32 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Clone)]
pub struct Config {
    pub debug: bool,
    pub s1: bool,
    pub s2: bool,
    pub s3: bool,
    pub llevel: LogLevel,
    pub lfile: PathBuf,
}

impl Config {
    fn new() -> Self {
        Self {
            debug: false,
            s1: true,
            s2: true,
            s3: true,
            llevel: LogLevel::Info,
            lfile: PathBuf::from("/tmp/rserve.log"),
        }
    }
}

pub static CONFIG: LazyLock<Mutex<Config>> = LazyLock::new(|| Mutex::new(Config::new()));

#[derive(Debug)]
pub struct HandlerChannels {
    pub s1_cmd_tx: mpsc::Sender<String>,
    pub s1_cmd_rx: Arc<Mutex<mpsc::Receiver<String>>>,
    pub s1_data_tx: mpsc::Sender<String>,
    pub s1_data_rx: Arc<Mutex<mpsc::Receiver<String>>>,

    pub s2_cmd_tx: mpsc::Sender<String>,
    pub s2_cmd_rx: Arc<Mutex<mpsc::Receiver<String>>>,
    pub s2_data_tx: mpsc::Sender<String>,
    pub s2_data_rx: Arc<Mutex<mpsc::Receiver<String>>>,

    pub s3_cmd_tx: mpsc::Sender<String>,
    pub s3_cmd_rx: Arc<Mutex<mpsc::Receiver<String>>>,
    pub s3_data_tx: mpsc::Sender<String>,
    pub s3_data_rx: Arc<Mutex<mpsc::Receiver<String>>>,
}

#[derive(Debug)]
pub struct CommChannels {
    pub cmd_tx: mpsc::Sender<String>,
    pub data_rx: Arc<Mutex<mpsc::Receiver<String>>>,
}

#[derive(Debug)]
pub struct WebHandlerChannels {
    pub s1_cmd_tx: mpsc::Sender<String>,
    pub s1_data_rx: Arc<Mutex<mpsc::Receiver<String>>>,

    pub s2_cmd_tx: mpsc::Sender<String>,
    pub s2_data_rx: Arc<Mutex<mpsc::Receiver<String>>>,

    pub s3_cmd_tx: mpsc::Sender<String>,
    pub s3_data_rx: Arc<Mutex<mpsc::Receiver<String>>>,
}

#[derive(Debug)]
pub struct SensorChannel {
    pub cmd_rx: Arc<Mutex<mpsc::Receiver<String>>>,
    pub data_tx: mpsc::Sender<String>,
}

// Implementation used for static store of measurement data
pub struct StateBuffer {
    buffer: Vec<String>,
    index: usize,
}

impl StateBuffer {
    pub fn new() -> Self {
        Self {
            buffer: Vec::with_capacity(MAX_ENTRIES),
            index: 0,
        }
    }

    // after max size, replace oldest entry
    pub fn add(&mut self, entry: String) {
        if self.buffer.len() < MAX_ENTRIES {
            self.buffer.push(entry);
        } else {
            self.buffer[self.index] = entry;
        }
        self.index = (self.index + 1) % MAX_ENTRIES;
    }

    // returns an iterator
    pub fn get_all(&self) -> &[String] {
        &self.buffer
    }
}

impl Default for StateBuffer {
    fn default() -> Self {
        Self::new()
    }
}

#[macro_export]
macro_rules! get_guard {
    ($lock:expr) => {
        match $lock.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                $lock.clear_poison();
                poisoned.into_inner()
            }
        }
    };
}

pub fn get_cfg() -> Config {
    let cfg = get_guard!(&CONFIG);

    cfg.clone()
}

pub fn set_cfg(new_cfg: Config) {
    let mut cfg = get_guard!(&CONFIG);

    *cfg = new_cfg;
}

/*
 * Errors are written to the log file defined in cfg.
 */
fn perr(cfg: &Config, level: LogLevel, perr: String) -> Result<(), std::io::Error> {
    let mut lfile = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&cfg.lfile)?;
    let _ = writeln!(lfile, "[{:?}] {}", level, perr);
    Ok(())
}

pub fn ulog<W: std::io::Write>(mut out: W, level: LogLevel, msg: String) {
    let cfg = get_cfg();
    match level {
        LogLevel::Trace => {
            if cfg.llevel <= LogLevel::Trace {
                let _ = writeln!(out, "[{:?}] {}", level, msg);
            };
        }
        LogLevel::Debug => {
            if cfg.llevel <= LogLevel::Debug {
                let _ = writeln!(out, "[{:?}] {}", level, msg);
            };
        }
        // for now, always emit info, warn & error
        LogLevel::Info => {
            let _ = writeln!(out, "[{:?}] {}", level, msg);
        }
        LogLevel::Warn => {
            let _ = writeln!(out, "[{:?}] {}", level, msg);
        }
        LogLevel::Error => {
            // Intent is output to stderr & the log file
            let _ = writeln!(out, "[{:?}] {}", level, msg);
            let _ = perr(&cfg, level, msg);
        }
    };
}

pub fn have_hw() -> bool {
    let mut hw: bool = false;

    if fs::metadata(HW1).is_ok() & fs::metadata(HW2).is_ok() {
        ulog(stdout(), DBG, String::from("Sensor H/W exists"));
        hw = true;
    }

    hw
}

pub fn validate_f64(val: f64) -> f64 {
    let mut rval = 0.0;
    if val.is_normal() {
        rval = val;
    }
    rval
}

pub fn validate_f32(val: f32) -> f32 {
    let mut rval = 0.0;
    if val.is_normal() {
        rval = val;
    }
    rval
}

fn usage() {
    eprintln!("rserve [command]");
    eprintln!("\t-s1 Disable sensor 1");
    eprintln!("\t-s2 Disable sensor 2");
    eprintln!("\t-s3 Disable sensor 3");
    eprintln!("\t-d | --debug Enable debug logs and debug mode");
    eprintln!("\t-h | --help Display usage detail");
    eprintln!("\t-l | --level Define log level");
    eprintln!("\t\tLogLevels:");
    eprintln!("\t\ttrace|debug|dbg|info|inf|warn|warning|error|err");
    process::exit(-1);
}

fn get_level(lvl: &str) -> LogLevel {
    match lvl {
        "trace" => LogLevel::Trace,
        "debug" => LogLevel::Debug,
        "dbg" => LogLevel::Debug,
        "info" => LogLevel::Info,
        "inf" => LogLevel::Info,
        "warn" => LogLevel::Warn,
        "warning" => LogLevel::Warn,
        "error" => LogLevel::Error,
        "err" => LogLevel::Error,
        _ => {
            eprintln!("Error: log level {lvl} isn't supported");
            usage();
            LogLevel::Debug
        }
    }
}

// Tried using clap. It's big and complex. This is simple, just a few options.
pub fn cli() -> Config {
    let mut cfg = get_guard!(&CONFIG);

    let args: Vec<String> = env::args().collect();

    if args.len() > 1 {
        let mut iter = args.iter();
        while let Some(arg) = iter.next() {
            if arg.contains("rserve") {
                continue;
            }

            match arg.as_str() {
                "-s1" => {
                    cfg.s1 = false;
                }

                "-s2" => {
                    cfg.s2 = false;
                }

                "-s3" => {
                    cfg.s3 = false;
                }

                "--debug" => {
                    cfg.debug = true;
                    cfg.llevel = DBG;
                }
                "-d" => {
                    cfg.debug = true;
                    cfg.llevel = DBG;
                }
                "--help" => {
                    usage();
                }
                "-h" => {
                    usage();
                }
                "--level" => {
                    if let Some(lvl) = iter.next() {
                        cfg.llevel = get_level(lvl);
                    } else {
                        eprintln!("level requires a log level value");
                        usage();
                    }
                }
                "-l" => {
                    if let Some(lvl) = iter.next() {
                        cfg.llevel = get_level(lvl);
                    } else {
                        eprintln!("level requires a log level value");
                        usage();
                    }
                }
                _ => {
                    eprintln!("arg {} is not valid", arg.as_str());
                    usage();
                }
            }
        }
    }
    cfg.clone()
}

// Init command channels
pub fn initialize_channels() -> HandlerChannels {
    ulog(stdout(), INF, String::from("initialize_channels"));

    let (s1_cmd_tx, s1_cmd_rx) = mpsc::channel::<String>();
    let (s1_data_tx, s1_data_rx) = mpsc::channel::<String>();
    let (s2_cmd_tx, s2_cmd_rx) = mpsc::channel::<String>();
    let (s2_data_tx, s2_data_rx) = mpsc::channel::<String>();
    let (s3_cmd_tx, s3_cmd_rx) = mpsc::channel::<String>();
    let (s3_data_tx, s3_data_rx) = mpsc::channel::<String>();

    HandlerChannels {
        s1_cmd_tx,
        s1_cmd_rx: Arc::new(Mutex::new(s1_cmd_rx)),
        s1_data_tx,
        s1_data_rx: Arc::new(Mutex::new(s1_data_rx)),
        s2_cmd_tx,
        s2_cmd_rx: Arc::new(Mutex::new(s2_cmd_rx)),
        s2_data_tx,
        s2_data_rx: Arc::new(Mutex::new(s2_data_rx)),
        s3_cmd_tx,
        s3_cmd_rx: Arc::new(Mutex::new(s3_cmd_rx)),
        s3_data_tx,
        s3_data_rx: Arc::new(Mutex::new(s3_data_rx)),
    }
}

/*
 * Convert a serializable struct to JSON
 * For my reference: (credit Google search AI)
 * The compiler handles generic parameters to functions through a process called monomorphization, where
 * it generates separate, specialized code for each concrete type the function is called with, effectively
 * replacing generic types with their specific implementations at compile time.
 * So, a small function getting repeated for each concrete type seems more efficient.
*/
pub fn to_json<T: Serialize>(data: &T) -> serde_json::Result<String> {
    serde_json::to_string(data)
}

// TODO: make the export operation configurable
pub fn export_data(jdata: &str) -> std::io::Result<()> {
    // TODO: move the const to cmd line param or env var
    let server_addr = EXPORT_HOST;
    let mut stream = TcpStream::connect(server_addr)?;

    ulog(
        stdout(),
        DBG,
        format!("Connected to export server at {}", server_addr),
    );

    // Send JSON data over the TCP connection
    stream.write_all(jdata.as_bytes())?;
    stream.write_all(b"\n")?; // Ensure the server knows the message boundary

    ulog(stdout(), DBG, format!("export_data: Sent: {}", jdata));
    Ok(())
}

pub async fn notify(message: String) -> bool {
    let api_key = env::var(NOTIFY_ENV_VAR);
    let key: String = match api_key {
        Ok(ekey) => {
            ulog(stdout(), DBG, String::from("We have an API key"));
            ekey
        }
        Err(e) => {
            ulog(
                stderr(),
                ERR,
                format!("Failed to send notification: no API key: {e}"),
            );
            return false;
        }
    };

    let client = Client::new();
    let channel = NOTIFY_CHANNEL;

    // The payload needed for the API: "token={}&channel={}&text={}",
    let mut payload = String::new();
    payload.push_str("token=");
    payload.push_str(&key);
    payload.push_str("&channel=");
    payload.push_str(channel);
    payload.push_str("&text=");
    payload.push_str(&message);

    ulog(stdout(), DBG, format!("Notify: {payload}"));

    // Create headers; sending raw text, not json
    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/x-www-form-urlencoded"),
    );

    let url = String::from(NOTIFY_URL);

    let response = client
        .post(url)
        .headers(headers)
        .body(payload) // raw plain text body.
        .send()
        .await;

    //dbg!(&response);
    let mut result: bool = false;
    match response {
        Ok(hres) => {
            // The response is an involved json object.
            // All we want is the value of ok, which is true or false.
            // The only substring of ':true' is from ok on success.
            // It's a short cut, just don't need any values in the json object.
            let success = match hres.text().await {
                Ok(hrt) => hrt,
                Err(e) => format!("notify: Error: json conversion: {e}"),
            };

            if success.contains(":true") {
                ulog(stdout(), DBG, String::from("Notification Successful"));
                result = true;
            }
        }
        Err(e) => eprintln!("response error: {e}"),
    }

    result
}

// Create a ctl-c handler that exits the process immediately
pub fn ctl_c_handler() {
    ctrlc::set_handler(move || {
        println!("Ctrl+C received! Cleaning up...");
        process::exit(0);
    })
    .expect("Error setting Ctrl+C handler");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(serde::Serialize)]
    struct TestStruct {
        t1: u64,
        t2: u64,
    }

    #[test]
    // command line: $ cargo test --  cfg_test --nocapture
    // Tests the macro get_guard in order to obtain a CONFIG
    fn cfg_test() {
        println!("cfg test");
        let mut cfg = Config::new();
        cfg.llevel = DBG;
        cfg.lfile = PathBuf::from("test_file");
        set_cfg(cfg);

        println!("Testing set cfg");
        let cfg2 = get_cfg();

        let mut buf = Vec::new();
        ulog(&mut buf, DBG, String::from("Testing Debug set"));

        let output = String::from_utf8_lossy(&buf);

        println!("Captured output: {}", output);
        assert_eq!(output.trim(), "[Debug] Testing Debug set");

        println!("{:?}", cfg2.lfile);
        let path_str = cfg2.lfile.to_string_lossy();
        assert!(path_str.contains("test_file"));
    }

    #[test]
    // command line:
    // $ cargo test -- log_test --nocapture -- -- -d
    // $ cargo test -- log_test --nocapture -- -- --level debug
    // Tests the macro get_guard in order to obtain a CONFIG
    fn log_test() {
        ulog(stdout(), WAR, String::from("TEST: Warn"));
        ulog(stderr(), ERR, String::from("TEST: Error"));
        ulog(stdout(), DBG, String::from("TEST: Debug"));
        ulog(stdout(), INF, String::from("TEST: Info"));

        let mut buf = Vec::new();
        ulog(&mut buf, INF, String::from("Testing log output"));

        let output = String::from_utf8_lossy(&buf);

        println!("Captured output: {}", output);
        assert_eq!(output.trim(), "[Info] Testing log output");
    }

    #[test]
    //$ cargo test --  state_buffer_test
    fn state_buffer_test() {
        println!("state buffer test");
        let mut buf = StateBuffer::new();

        let mut i: usize;
        for i in 0..MAX_ENTRIES {
            buf.add(format!("sb.{i}"));
        }

        i = 0;
        for entry in buf.get_all() {
            let sbs = format!("sb.{i}");
            assert_eq!(entry, &sbs);
            i += 1;
        }
    }

    #[test]
    //$ cargo test --  to_json_test --nocapture
    fn to_json_test() {
        println!("json serialize test");
        let jdata = TestStruct { t1: 99, t2: 100 };

        match to_json(&jdata) {
            Ok(jser) => assert_eq!(jser, String::from("{\"t1\":99,\"t2\":100}")),
            Err(e) => {
                eprintln!("Error: json serialization: {e}");
                panic!();
            }
        }
    }

    #[test]
    //$ cargo test -- fpval_test --nocapture -- -- -d
    fn fpval_test() {
        println!("f64 & f32 validation test");
        let mut v1: f64 = 0.0;
        let mut v2: f32 = 0.0;

        assert_eq!(validate_f64(v1), 0.0);
        assert_eq!(validate_f32(v2), 0.0);

        v1 = 12.99;
        v2 = 42.42;
        assert_eq!(validate_f64(v1), 12.99);
        assert_eq!(validate_f32(v2), 42.42);

        v1 = -12.12;
        v2 = -42.99;
        assert_eq!(validate_f64(v1), -12.12);
        assert_eq!(validate_f32(v2), -42.99);

        let nan1 = f64::NAN;
        let nan2 = f32::NAN;
        assert_eq!(validate_f64(nan1), 0.0);
        assert_eq!(validate_f32(nan2), 0.0);
    }
}
