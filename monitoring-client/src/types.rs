use super::*;
use chrono::{Local, DateTime};
use lettre::EmailAddress;

pub struct TesterParameters {
    pub acc_count: usize, // count of deployed accounts
    pub tr_interval: u64, // in seconds - interval between sending founds (default 3600)
    pub tr_count: u32, // count of transactions in each attempt (default 3)
}

pub struct ReporterParameters {
    pub emails: Vec<EmailAddress>, // addresses list to send logs to
    pub email: EmailAddress, // own email address
    pub email_sever: String, // smtp server to send emails
    pub username: String, // credentials to access the server
    pub password: String, 
    pub reports_interval: u64 // interval between reports sending
}

pub struct Statistic {
    pub log: Vec<String>,
    pub attempts: u32,
    pub fails: u32,
    pub started: DateTime<Local>,
}

impl Statistic {

    pub fn new() -> Self {
        Statistic {
            log: Vec::new(),
            attempts: 0,
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
        self.attempts += 1;
    }

    pub fn log_error_attempt(&mut self, message: &str) {
        let m = format!("{} ERROR {}", Local::now().to_rfc2822(), message);
        println!("{}", &m);
        self.log.push(m);
        self.attempts += 1;
        self.fails += 1;
    }
}