extern crate ton_sdk;
extern crate hex;
extern crate serde;
extern crate serde_json;
extern crate serde_derive;
extern crate num_bigint;
extern crate futures;
extern crate chrono;
extern crate lettre;

mod reporter;
use reporter::*;

mod tester;
use tester::*;

mod types;
use types::*;

use ton_sdk::*;
use std::sync::{Arc, Mutex};
use lettre::EmailAddress;

fn main() {

    let stat = Arc::new(Mutex::new(Statistic::new()));
    
    let tester_params = TesterParameters{
        acc_count: 2,
        tr_interval: 60,
        tr_count: 2,
    };

    let client_config = NodeClientConfig {
        db_config: RethinkConfig {
            servers: vec!["142.93.137.28:28015".to_string()],
            db_name: "blockchain".to_string(),
        },
        kafka_config: KafkaConfig {
            servers: vec!["142.93.137.28:9092".to_string()],
            topic: "requests".to_string(),
            ack_timeout: 1000,
        },
    };

    let reporter_params = ReporterParameters {
        emails: vec![EmailAddress::new("".to_string()).expect("error parsing email")],
        email: EmailAddress::new("".to_string()).expect("error parsing email"),
        email_sever: "".to_string(),
        username: "".to_string(),
        password: "".to_string(), 
        reports_interval: 3600
    };

    attempts_thread(stat.clone(), tester_params, client_config);

    std::thread::spawn(move || {
        report_thread(stat, reporter_params);
    });


}
