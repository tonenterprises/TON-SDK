use super::*;
use chrono::{Local, DateTime};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TesterConfig {
    pub acc_count: usize, // count of deployed accounts
    pub tr_interval: u64, // in seconds - interval between sending founds (default 3600)
    pub tr_count: u32, // count of transactions in each attempt (default 3)
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ReporterConfig {
    pub web_hook: String, // discord webhook to send reports
    pub reports_interval: u64, // interval between success reports sending
    pub fails_reports_interval: u64, // interval between success reports sending
}

pub struct Statistic {
    pub log: Vec<String>,
    pub total_attempts: u32,
    pub total_fails: u32,
    pub total_started: DateTime<Local>,

    pub attempts: u32,
    pub last_error: String,
    pub fails: u32,
    pub started: DateTime<Local>,
}

impl Statistic {

    pub fn new() -> Self {
        Statistic {
            log: Vec::new(),
            total_attempts: 0,
            total_fails: 0,
            total_started: Local::now(),
            attempts: 0,
            last_error: String::default(),
            fails: 0,
            started: Local::now(),
        }
    }

    pub fn log_info(&mut self, message: &str) {
        let m = format!("{}       {}", Local::now().to_rfc2822(), message);
        println!("{}", &m);
        self.log.push(m);
    }

    pub fn log_success_attempt(&mut self, message: &str) {
        let m = format!("{} OK    {}", Local::now().to_rfc2822(), message);
        println!("{}", &m);
        self.log.push(m);
        self.total_attempts += 1;
        self.attempts += 1;
    }

    pub fn log_error_attempt(&mut self, message: &str) {
        let m = format!("{} ERROR {}", Local::now().to_rfc2822(), message);
        println!("{}", &m);
        self.log.push(m.clone());
        self.last_error = m;
        self.total_attempts += 1;
        self.attempts += 1;
        self.total_fails += 1;
        self.fails += 1;
    }
}