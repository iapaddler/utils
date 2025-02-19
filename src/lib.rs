use chrono::{Local, Utc};
use libc::{c_double, c_int};
use rand::Rng;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use reqwest::Client;
use std::env;
use std::process;
use std::sync::{mpsc, Arc, LazyLock, Mutex};
use std::thread;
use std::time::Duration;

const NOTIFY_URL: &str = "https://slack.com/api/chat.postMessage";
const NOTIFY_CHANNEL: &str = "#drn";
const NOTIFY_ENV_VAR: &str = "APPVIEW_SLACKBOT_TOKEN";
pub const PERIOD: u64 = 5; //30; //300; // read every 5 mins
const MAX_ENTRIES: usize = 100;

// Could use features. Too confusing
// DEBUG:
//const NUM_MEASUREMENTS: i32 = 2;
//const NUM_RUNS: i32 = 7;
//const HAVE_SENSOR: bool = false;

const NUM_MEASUREMENTS: i32 = 12; // report every 1 hour
const NUM_RUNS: i32 = 60;
const HAVE_SENSOR: bool = true;

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

#[repr(C)]
pub struct sensor_data_t {
    pub temperature: c_double,
    pub pressure: c_double,
}

pub struct CommChannels {
    pub cmd_tx: mpsc::Sender<String>,
    pub cmd_rx: Arc<Mutex<mpsc::Receiver<String>>>,
    pub data_tx: mpsc::Sender<String>,
    pub data_rx: Arc<Mutex<mpsc::Receiver<String>>>,
}

// Implementation used for static store of measurement data
struct StateBuffer {
    buffer: Vec<String>,
    index: usize,
}

impl StateBuffer {
    fn new() -> Self {
        Self {
            buffer: Vec::with_capacity(MAX_ENTRIES),
            index: 0,
        }
    }

    // after max size, replace oldest entry
    fn add(&mut self, entry: String) {
        if self.buffer.len() < MAX_ENTRIES {
            self.buffer.push(entry);
        } else {
            self.buffer[self.index] = entry;
        }
        self.index = (self.index + 1) % MAX_ENTRIES;
    }

    // returns an iterator
    fn get_all(&self) -> &[String] {
        &self.buffer
    }
}

pub static CONFIG: LazyLock<Mutex<Config>> = LazyLock::new(|| Mutex::new(Config::new()));

#[link(name = "rsd", kind = "static")]
extern "C" {
    fn getSensorData(sdata: &sensor_data_t) -> c_int;
}

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
pub fn initialize_channels() -> CommChannels {
    let (cmd_tx, cmd_rx) = mpsc::channel::<String>();
    let (data_tx, data_rx) = mpsc::channel::<String>();

    CommChannels {
        cmd_tx,
        cmd_rx: Arc::new(Mutex::new(cmd_rx)),
        data_tx,
        data_rx: Arc::new(Mutex::new(data_rx)),
    }
}

pub fn get_sensor_data() -> sensor_data_t {
    let mut sdata = sensor_data_t {
        temperature: 0.0,
        pressure: 0.0,
    };

    if HAVE_SENSOR == true {
        unsafe {
            getSensorData(&sdata);
        }
    } else {
        // helps with test/debug to have values
        let mut rng = rand::thread_rng();
        sdata.pressure = rng.gen();
        sdata.temperature = 70.0;
    }

    debug(format!(
        "get_sensor_data: Temp {} Pressure {}",
        sdata.temperature, sdata.pressure
    ));

    sdata
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

pub async fn particulate_sensor(channels: CommChannels) {
    debug(format!("particulate_sensor: start"));

    loop {
        thread::sleep(Duration::from_secs(PERIOD));
        debug(format!("particulate_sensor: run"));
    }
}

pub async fn dummy_sensor(channels: CommChannels) {
    let mut num_read: i32 = 0;
    let mut num_run: i32 = 0;
    let data_tx = channels.data_tx.clone();
    let rx = channels.cmd_rx.lock().unwrap();

    debug(format!("dummy_sensor: start"));

    // Vector of strings for data
    let mut buf = StateBuffer::new();

    loop {
        if num_run == NUM_RUNS {
            num_run = 0;

            // Current local date and time
            let dtime = Local::now().format("%Y-%m-%d %H:%M").to_string();
            let dval: f64 = rand::thread_rng().gen();

            // String format for the web server: dummy sensor: value: 0.00 time
            // Using random values for data
            let ddata = format!("Dummy sensor: {dtime} {:.2}", dval);

            buf.add(ddata);
            num_read += 1;
        }

        debug(format!("dummy_sensor: run"));

        // Sending notification every N measurements
        if num_read == NUM_MEASUREMENTS {
            let current_time = Local::now().time();
            let display_time = current_time.format("%H:%M");
            let dval: f64 = rand::thread_rng().gen();
            let message = format!("Today at {display_time} the dummy value is {:.2}", dval);
            notify(message).await;

            // reset after every notify period
            num_read = 0;
        }

        // Checking for a command every period
        if let Ok(_msg) = rx.try_recv() {
            // TODO: process a specific command
            // For now, default to send data
            for entry in buf.get_all() {
                let dres = data_tx.send(entry.to_string());
                match dres {
                    Ok(_) => debug(format!("notify thread sent data")),
                    Err(e) => eprintln!("Error on data send: {e}"),
                };
            }
        }

        num_run += 1;
    }
}

pub async fn pressure_sensor(channels: CommChannels) {
    let mut high_press: f64 = 0.00;
    let mut low_press: f64 = 0.00;
    let mut first_pass_press: f64 = 0.00;
    let mut prev_press: f64 = 0.00;
    let mut num_read: i32 = 0;
    let mut num_run: i32 = 0;
    let data_tx = channels.data_tx.clone();
    let rx = channels.cmd_rx.lock().unwrap();
    let mut sdata = sensor_data_t {
        temperature: 0.0,
        pressure: 0.0,
    };

    debug(format!("Update thread starting"));

    // Vector of strings for data
    let mut buf = StateBuffer::new();

    loop {
        // Getting measurements every N minutes
        if num_run == NUM_RUNS {
            num_run = 0;

            sdata = get_sensor_data();

            debug(format!(
                "update thread({num_read}): Temp {} Pressure {}",
                sdata.temperature, sdata.pressure
            ));

            // Current local date and time
            let dtime = Local::now().format("%Y-%m-%d %H:%M").to_string();
            let epoch_secs = Utc::now().timestamp();

            // String format for the web server: 0.16 70.00 -2.00 2025-01-18 16:06 (17000)
            let wsdata = format!(
                "{:.2} {:.2} {:.2} {dtime} ({epoch_secs})",
                sdata.pressure,
                (sdata.temperature * 1.8) + 32.0,
                sdata.pressure - prev_press
            );

            buf.add(wsdata);

            if first_pass_press == 0.0 {
                first_pass_press = sdata.pressure;
            }

            if sdata.pressure > high_press {
                high_press = sdata.pressure;
            }

            if (low_press == 0.0) | (sdata.pressure < low_press) {
                low_press = sdata.pressure;
            }

            prev_press = sdata.pressure;
            num_read += 1;
        }

        // Sending notification every N measurements
        if num_read == NUM_MEASUREMENTS {
            // last measurement - first measurement
            let pstat: f64 = sdata.pressure - first_pass_press;

            debug(format!(
                "pstat(num_read): {pstat} high: {high_press} low: {low_press}"
            ));

            let rising = if pstat > 0.00 {
                "rising".to_string()
            } else {
                "falling".to_string()
            };

            let current_time = Local::now().time();
            let display_time = current_time.format("%H:%M");
            let message = format!(
                "Today at {display_time} the pressure is {rising} (delta: {:.2} current: {:.2} max {:.2} min {:.2})",
                pstat, sdata.pressure, high_press, low_press
            );

            notify(message).await;

            // reset after every notify period
            high_press = 0.00;
            low_press = 0.00;
            first_pass_press = 0.00;
            num_read = 0;
        }

        // Checking for a command every period
        if let Ok(_msg) = rx.try_recv() {
            // TODO: process a specific command
            // For now, default to send data
            for entry in buf.get_all() {
                let dres = data_tx.send(entry.to_string());
                match dres {
                    Ok(_) => debug(format!("notify thread sent data")),
                    Err(e) => eprintln!("Error on data send: {e}"),
                };
            }
        }

        num_run += 1;
        thread::sleep(Duration::from_secs(PERIOD));
    }
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
