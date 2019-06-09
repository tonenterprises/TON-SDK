use super::*;

use chrono::{Local};

pub fn report_thread(stat: Arc<Mutex<Statistic>>, config: ReporterConfig, only_errors: bool) {

    if !only_errors {
        let mut map = std::collections::HashMap::new();
        map.insert("content", "Hello. Monitoring node client is sucessfully run.");

        let client = reqwest::Client::new();
        let res = client.post(&config.web_hook)
            .json(&map)
            .send();

        match res {
            Ok(_) => (),
            Err(err) => {
                print!("\nERROR SENDING HELLO: {}\n", err);
            }
        }
    }

    loop {
        std::thread::sleep(std::time::Duration::from_secs(
            if only_errors { config.fails_reports_interval } else {config.reports_interval} ));

        let mut message = String::new();        
        let mut stat = stat.lock().unwrap();

        if only_errors && stat.fails == 0 {
            continue;
        }

        let m = format!(
            "Since {} there were {} attempts to transfer founds.\n",
            stat.started.to_rfc2822(), if only_errors { stat.attempts } else { stat.total_attempts });
        message.push_str(&m);

        let m = format!("{} of them FAILED\n", stat.fails);
        message.push_str(if stat.fails == 0 {
            "All of them were SUCCEDED\n"
        } else {
            &m
        });
        if only_errors {
            let m = format!("Last error: **{}**", stat.last_error);
            message.push_str(&m);
        }

        /*
        url='your hook'
        curl -H "Content-Type: application/json" -X POST -d "{\"content\": \"hello!\"}" $url
        */
        let mut map = std::collections::HashMap::new();
        map.insert("content", message);

        let client = reqwest::Client::new();
        let res = client.post(&config.web_hook)
            .json(&map)
            .send();
        match res {
            Ok(resp) => {
                if resp.status().is_server_error() || resp.status().is_client_error() {
                    println!("ERROR SENDING REPORT ({:?})", resp.status().canonical_reason())
                } else {
                    print!("\nREPORT SENT\n");
                    if only_errors {
                        stat.fails = 0;
                        stat.started = Local::now();
                        stat.attempts = 0;
                    } else {
                        stat.log = vec!();
                        stat.total_attempts = 0;
                        stat.total_fails = 0;
                        stat.total_started = Local::now();
                    }
                }
            }
            Err(err) => {
                print!("\nERROR SENDING REPORT: {}\n", err);
            }
        }
    }
}