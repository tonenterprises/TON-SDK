use super::*;

use lettre::smtp::authentication::{Credentials, Mechanism};
use lettre::{SendableEmail, Envelope, Transport, SmtpClient};
use chrono::{Local};

pub fn report_thread(stat: Arc<Mutex<Statistic>>, params: ReporterParameters) {

    loop {

        
        // Connect to a remote server on a custom port
        let mut mailer = SmtpClient::new_simple("server.tld").unwrap()
            // Set the name sent during EHLO/HELO, default is `localhost`
            //.hello_name(ClientId::Domain("my.hostname.tld".to_string()))
            // Add credentials for authentication
            .credentials(Credentials::new(params.username.clone(), params.password.clone()))
            // Enable SMTPUTF8 if the server supports it
            .smtp_utf8(true)
            // Configure expected authentication mechanism
            .authentication_mechanism(Mechanism::Plain)
            .transport();

        
        std::thread::sleep(std::time::Duration::from_secs(params.reports_interval));

        let mut message = String::new();        
        let mut stat = stat.lock().unwrap();
        let subject = if stat.fails == 0 {
            "node client report - SUCCESS"
        } else {
            "node client report - ERRORS"
        }.to_string();

        let m = format!(
            "Since {} there were {} attempts to transfer founds.\n",
            stat.started.to_rfc2822(), stat.attempts);
        message.push_str(&m);

        let m = format!("{} of them FAILED\n", stat.fails);
        message.push_str(if stat.fails == 0 {
            "All of them were SUCCEDED\n"
        } else {
            &m
        });

        message.push_str("\n");
        for str in stat.log.iter() {
            message.push_str(str);
            message.push_str("\n");
        }

        /* 
        println!("\n====== LETTER =======\n");
        println!("{}", &message);
        println!("\n====== ====== =======\n");
        print!("\nREPORT SENT\n");
        stat.log = vec!();
        stat.attempts = 0;
        stat.fails = 0;
        stat.started = Local::now();
         */ 

        
        
        let email = SendableEmail::new(
            Envelope::new(
                Some(params.email.clone()),
                params.emails.clone(),
            ).expect("error creating Envelope"),
            subject,
            message.into_bytes(),
        );

        match mailer.send(email) {
            Ok(_) => {
                print!("\nREPORT SENT\n");
                stat.log = vec!();
                stat.attempts = 0;
                stat.fails = 0;
                stat.started = Local::now();
            }
            Err(err) => {
                print!("\nERROR SENDING REPORT: {}\n", err);
            }
        };
        
    }
}




#[test]
fn test_email() {
        
        let params = ReporterParameters {
            emails: vec![EmailAddress::new("zavarovskii@ya.ru".to_string()).expect("error parsing email")],
            email: EmailAddress::new("zavarovskii@ya.ru".to_string()).expect("error parsing email"),
            email_sever: "smtp.yandex.ru".to_string(),
            username: "zavarovskii".to_string(),
            password: "Anita439Mvs".to_string(), 
            reports_interval: 60
        };
        

        let mut tls_builder = TlsConnector::builder();
            tls_builder.min_protocol_version(Some(Protocol::Tlsv10));
        let tls_parameters =
            ClientTlsParameters::new(
                params.email_sever.clone(),
                tls_builder.build().unwrap()
            );


        // Connect to a remote server on a custom port
        let mut mailer = SmtpClient::new((&params.email_sever, 465), ClientSecurity::Wrapper(tls_parameters))
            .unwrap()
            // Set the name sent during EHLO/HELO, default is `localhost`
            //.hello_name(ClientId::Domain("my.hostname.tld".to_string()))
            // Add credentials for authentication
            .credentials(Credentials::new(params.username.clone(), params.password.clone()))
            // Enable SMTPUTF8 if the server supports it
            .smtp_utf8(true)
            // Configure expected authentication mechanism
            .authentication_mechanism(Mechanism::Plain)
            .transport();
        
        let email = SendableEmail::new(
            Envelope::new(
                Some(params.email.clone()),
                params.emails.clone(),
            ).expect("error creating Envelope"),
            "subject".to_string(),
            "test".to_string().into_bytes(),
        );

        match mailer.send(email) {
            Ok(_) => {
                print!("\nREPORT SENT\n");
            }
            Err(err) => {
                print!("\nERROR SENDING REPORT: {}\n", err);
            }
        };
}