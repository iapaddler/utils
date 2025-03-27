use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use reqwest::Client;
use serde::Serialize;
use serde_json::{to_string, Result};
use std::env;
use std::io::Write;
use std::net::TcpStream;
use std::process;
use std::sync::{mpsc, Arc, LazyLock, Mutex};

const NOTIFY_URL: &str = "https://slack.com/api/chat.postMessage";
const NOTIFY_CHANNEL: &str = "#drn";
const NOTIFY_ENV_VAR: &str = "APPVIEW_SLACKBOT_TOKEN";
pub const PERIOD: u64 = 5;
const MAX_ENTRIES: usize = 288; // Assuming 5 mins per measurement, gives us 24 hours of data
const EXPORT_HOST: &str = "default.main.musing-faraday-83adewh.cribl.cloud:20000";

// Could use features. Too confusing
// DEBUG:
//pub const NUM_MEASUREMENTS: i32 = 2;
//pub const NUM_RUNS: i32 = 7;
//pub const HAVE_SENSOR: bool = false;

pub const NUM_MEASUREMENTS: i32 = 12; // report every 1 hour
pub const NUM_RUNS: i32 = 60;
pub const HAVE_SENSOR: bool = true;

#[derive(Clone, Debug)]
pub struct Config {
    pub debug: bool,
    pub s1: bool,
    pub s2: bool,
    pub s3: bool,
}

impl Config {
    fn new() -> Self {
        Self {
            debug: false,
            s1: true,
            s2: true,
            s3: true,
        }
    }
}

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

pub static CONFIG: LazyLock<Mutex<Config>> = LazyLock::new(|| Mutex::new(Config::new()));

// Tried using clap. It's big and complex. This is simple, just a few bools.
pub fn cli() -> Config {
    let mut cfg = CONFIG.lock().unwrap();
    let args: Vec<String> = env::args().collect();

    if args.len() > 1 {
        let options = [
            "-d".to_string(),
            "-s1".to_string(),
            "-s2".to_string(),
            "-s3".to_string(),
        ];

        for i in 0..args.len() {
            let arg = args.get(i).unwrap();

            if arg.contains(&options[0]) {
                cfg.debug = true;
            } else if arg.contains(&options[1]) {
                cfg.s1 = false;
            } else if arg.contains(&options[2]) {
                cfg.s2 = false;
            } else if arg.contains(&options[3]) {
                cfg.s3 = false;
            }
        }
    }

    cfg.clone()
}

pub fn debug(msg: String) {
    let cfg = CONFIG.lock().unwrap();
    if cfg.debug == true {
        println!("{}", msg);
    }
}

// Init command channels
pub fn initialize_channels() -> HandlerChannels {
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
pub fn to_json<T: Serialize>(data: &T) -> Result<String> {
    to_string(data)
}

// TODO: make the export operation configurable
pub fn export_data(jdata: &str) -> std::io::Result<()> {
    // TODO: move the const to cmd line param or env var
    let server_addr = EXPORT_HOST;
    let mut stream = TcpStream::connect(server_addr)?;

    debug(format!("Connected to export server at {}", server_addr));

    // Send JSON data over the TCP connection
    stream.write_all(jdata.as_bytes())?;
    stream.write_all(b"\n")?; // Ensure the server knows the message boundary

    debug(format!("export_data: Sent: {}", jdata));
    Ok(())
}

pub async fn notify(message: String) -> bool {
    let key: String;
    let api_key = env::var(NOTIFY_ENV_VAR);
    match api_key {
        Ok(ekey) => {
            debug(format!("We have an API key"));
            key = ekey;
        }
        Err(e) => {
            eprintln!("Failed to send notification: no API key: {e}");
            return false;
        }
    }

    let client = Client::new();
    let channel = NOTIFY_CHANNEL;

    // The payload needed for the API: "token={}&channel={}&text={}",
    let mut payload = String::new();
    payload.push_str("token=");
    payload.push_str(&key);
    payload.push_str("&channel=");
    payload.push_str(&channel);
    payload.push_str("&text=");
    payload.push_str(&message);

    //debug(format!("Notify: {payload}"));

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
            let success = hres.text().await.unwrap();
            if success.contains(":true") {
                debug(format!("Notification successful"));
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

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
