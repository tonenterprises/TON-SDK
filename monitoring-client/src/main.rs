extern crate ton_sdk;
extern crate hex;
extern crate serde;
extern crate serde_json;
extern crate serde_derive;
extern crate num_bigint;
extern crate futures;
extern crate futures_timer;
extern crate chrono;
extern crate reqwest;
extern crate http;

mod reporter;
use reporter::*;

mod tester;
use tester::*;

mod types;
use types::*;

use ton_sdk::*;
use std::sync::{Arc, Mutex};
use std::env::var_os;

fn env_string_unwrap(name: &str, default: Option<&str>) -> String {
    match var_os(&name) {
        Some(str) => str.into_string().expect("{} contains invalid Unicode data"),
        None => default.map(|s| s.to_string()).expect(&format!("error getting {}", name))
    }
}

fn main() {

    let stat = Arc::new(Mutex::new(Statistic::new()));
    let stat1 = Arc::clone(&stat);
    let stat2 = Arc::clone(&stat);

    let tester_config = TesterConfig {
        acc_count: usize::from_str_radix(&env_string_unwrap("ACCOUNTS_COUNT", Some("2")), 10)
            .expect("error parsing ACCOUNTS_COUNT"),

        tr_interval: u64::from_str_radix(&env_string_unwrap("TRANSACTIONS_INTERVAL_SEC", Some("60")), 10)
            .expect("error parsing TRANSACTIONS_INTERVAL_SEC"),

        tr_count: u32::from_str_radix(&env_string_unwrap("TRANSACTIONS_COUNT", Some("2")), 10)
            .expect("error parsing TRANSACTIONS_COUNT"),
    };

    let client_config = NodeClientConfig {
        db_config: RethinkConfig {
            servers: vec!( env_string_unwrap("RETHINK_ENDPOINT", Some("142.93.137.28:28015")) ),
            db_name: env_string_unwrap("RETHINK_DATABASE", Some("blockchain")),
        },
        kafka_config: KafkaConfig {
            servers: vec!( env_string_unwrap("KAFKA_ENDPOINT", Some("142.93.137.28:9092")) ),
            topic: env_string_unwrap("KAFKA_TOPIC", Some("requests")),
            ack_timeout:  u64::from_str_radix(&env_string_unwrap("KAFKA_ACT_TIMEOUT", Some("1000")), 10)
                .expect("error parsing KAFKA_ACT_TIMEOUT"),
        },
    };

    let reporter_config = ReporterConfig {
        web_hook: env_string_unwrap("DISCORD_WEBHOOK", Some("https://discordapp.com/api/webhooks/586169338063749150/w_9eeI-EnL8vCCsupU7hzjdBK-s1-DDJduO-eWzld_Md5MB6yiNGhHZ3XAZYlN0IhxDg")),
        reports_interval: u64::from_str_radix(&env_string_unwrap("REPORTS_INTERVAL_SEC", Some("21600")), 10)
            .expect("error parsing REPORTS_INTERVAL_SEC"),
        fails_reports_interval: u64::from_str_radix(&env_string_unwrap("FAIL_REPORTS_INTERVAL_SEC", Some("300")), 10)
            .expect("error parsing FAIL_REPORTS_INTERVAL_SEC"),
    };
    let reporter_config2 = reporter_config.clone();

    std::thread::spawn(move || {
        report_thread(stat1, reporter_config2, false);
    });
    std::thread::spawn(move || {
        report_thread(stat2, reporter_config, true);
    });

    attempts_thread(stat, tester_config, client_config);
}
