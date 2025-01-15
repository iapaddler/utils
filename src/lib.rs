use chrono::Local;
use libc::{c_double, c_int};
use rand::Rng;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use reqwest::Client;
use std::env;
use std::process;
use std::thread;
use std::time::Duration;

const NOTIFY_URL: &str = "https://slack.com/api/chat.postMessage";
const NOTIFY_CHANNEL: &str = "#drn";
const NOTIFY_ENV_VAR: &str = "APPVIEW_SLACKBOT_TOKEN";
const PERIOD: u64 = 300; // read every 5 mins
const NUM_READS: i32 = 12; // report every 1 hour
const HAVE_SENSOR: bool = true;

#[repr(C)]
pub struct sensor_data_t {
    pub temperature: c_double,
    pub pressure: c_double,
}

#[link(name = "rsd", kind = "static")]
extern "C" {
    fn getSensorData(sdata: &sensor_data_t) -> c_int;
}

pub fn debug(msg: String) {
    let args: Vec<String> = env::args().collect();

    if args.len() >= 3 {
        let arg = match args.get(1) {
            Some(cmd) => cmd,
            None => {
                eprintln!("Unknown arguments provided");
                return;
            }
        };

        let dcmd = String::from("-d");
        if arg == dcmd.as_str() {
            let dval = match args.get(2) {
                Some(val) => val,
                None => {
                    eprintln!("Unknown arguments provided!");
                    return;
                }
            };

            let gdbg = match dval.parse::<u64>() {
                Ok(val) => val,
                Err(e) => {
                    eprintln!("Unable to parse number from argument: {}", e);
                    return;
                }
            };

            if gdbg > 0 {
                println!("{}", msg);
            }
        }
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

pub async fn update_and_notify() {
    let mut sdata;

    let mut high_press: f64 = 0.00;
    let mut low_press: f64 = 0.00;
    let mut first_pass_press: f64 = 0.00;
    let mut num_read: i32 = 0;

    debug(format!("Update thread starting"));

    loop {
        sdata = get_sensor_data();

        debug(format!(
            "update thread({num_read}): Temp {} Pressure {}",
            sdata.temperature, sdata.pressure
        ));

        if first_pass_press == 0.0 {
            first_pass_press = sdata.pressure;
        }

        if sdata.pressure > high_press {
            high_press = sdata.pressure;
        }

        if (low_press == 0.0) | (sdata.pressure < low_press) {
            low_press = sdata.pressure;
        }

        num_read += 1;

        if num_read == NUM_READS {
            // last measurement - first measurement
            let pstat: f64 = sdata.pressure - first_pass_press;

            debug(format!(
                "pstat: {pstat} high: {high_press} low: {low_press}"
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
