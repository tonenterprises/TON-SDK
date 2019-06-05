extern crate ton_sdk;
extern crate hex;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
extern crate num_bigint;
extern crate futures;

use rand::{thread_rng, Rng};
use ton_block::{Message, MsgAddressExt, MsgAddressInt, InternalMessageHeader, Grams, 
    ExternalInboundMessageHeader, CurrencyCollection, Serializable, GetSetValueForVarInt,
    MessageProcessingStatus};
use tvm::bitstring::Bitstring;
use tvm::types::{AccountId};
use ed25519_dalek::Keypair;
use futures::{Stream, Async, Poll};
use sha2::Sha512;
use std::str::FromStr;
use num_traits::cast::ToPrimitive;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::fmt::Display;

use ton_sdk::*;

const REQUEST_TIMEOUT: u64 = 60; // seconds
const WALLET_ABI: &str = r#"{
    "ABI version" : 0,
    "functions" :	[{
            "inputs": [
                {"name": "recipient", "type": "bits256"},
                {"name": "value", "type": "duint"}
            ],
            "name": "sendTransaction",
            "signed": true,
            "outputs": [
                {"name": "transaction", "type": "uint64"},
                {"name": "error", "type": "int8"}
            ]
        }, {
            "inputs": [
                {"name": "type", "type": "uint8"},
                {"name": "value", "type": "duint"},
                {"name": "meta", "type": "bitstring"}
            ],
            "name": "createLimit",
            "signed": true,
            "outputs": [
                {"name": "limitId", "type": "uint8"},
                {"name": "error", "type": "int8"}
            ]
        }, {
            "inputs": [
                {"name": "limitId", "type": "uint8"},
                {"name": "value", "type": "duint"},
                {"name": "meta", "type": "bitstring"}
            ],
            "name": "changeLimitById",
            "signed": true,
            "outputs": [{"name": "error", "type": "int8"}]
        }, {
            "inputs": [{"name": "limitId", "type": "uint8"}],
            "name": "removeLimit",
            "signed": true,
            "outputs": [{"name": "error", "type": "int8"}]
        }, {
            "inputs": [{"name": "limitId", "type": "uint8"}],
            "name": "getLimitById",
            "outputs": [
                {
                    "name": "limitInfo",
                    "type": "tuple",
                    "components": [
                        {"name": "value", "type": "duint"},
                        {"name": "type", "type": "uint8"},
                        {"name": "meta", "type": "bitstring"}
                        ]
                },
                {"name": "error", "type": "int8"}
            ]
        }, {
            "inputs": [],
            "name": "getLimits",
            "outputs": [
                {"name": "list", "type": "uint8[]"},
                {"name": "error", "type": "int8"}
            ]
        }, {
            "inputs": [],
            "name": "getVersion",
            "outputs": [
                {
                    "name": "version",
                    "type": "tuple",
                    "components": [
                        {"name": "major", "type": "uint16"},
                        {"name": "minor", "type": "uint16"}
                    ]
                },
                {"name": "error", "type": "int8"}
            ]
        }, {
            "inputs": [],
            "name": "getBalance",
            "outputs": [{"name": "balance", "type": "uint64"}]
        }, {
            "inputs": [],
            "name": "constructor",
            "outputs": []							
        }, {
            "inputs": [{"name": "address", "type": "bits256" }],
            "name": "setSubscriptionAccount",
                    "signed": true,
            "outputs": []							
        }, {
            "inputs": [],
            "name": "getSubscriptionAccount",
            "outputs": [{"name": "address", "type": "bits256" }]							
        }
    ]
}
"#;

struct Statistic {
    pub log: Vec<String>,
    pub attempt: u32,
    pub fails: u32,
}

struct TesterParameters {
    acc_count: u32, // count of deployed accounts
    tr_interval: u64, // in seconds - interval between sending founds (default 3600)
    tr_count: u32, // count of transactions in each attempt (default 3)
}

struct ReporterParameters {
    emails: Vec<String>, // addresses list to send logs to
    email: String, // own email address
    email_sever: String, // smtp server to send emails
    username: String, // credentials to access the server
    password: String, 
    reports_interval: u32 // interval between reports sending
}

trait WithLimit<S> {
    fn limit(self, timeout: Duration) -> StreamLimiter<S>;
}

impl<S: Stream> WithLimit<S> for S {
    fn limit(self, timeout: Duration) -> StreamLimiter<S> {
        StreamLimiter::new(self, timeout)
    }
}

struct StreamLimiter<S> {
    inner: S,
    timeout: Duration,
    started: Option<Instant>,
}

impl<S> StreamLimiter<S> {
    pub fn new(inner: S, timeout: Duration) -> Self {
        StreamLimiter {
            inner,
            timeout,
            started: None,
        }
    }
}

impl<S> Stream for StreamLimiter<S> 
    where 
        S: Stream,
        S::Error: Display
{
    type Item = S::Item;
    type Error = String; //T::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        let now = Instant::now();
        if self.started.is_none() {
            self.started = Some(now);
        }
        match self.inner.poll() {
            Ok(Async::Ready(value)) => {
                self.started = Some(now);
                Ok(Async::Ready(value))
            },
            Err(err) => {
                self.started = Some(now);
                Err(format!("{}", err))
            },
            Ok(Async::NotReady) => {
                if self.started.unwrap() + self.timeout < now {
                    Err("Error timeout".to_string())
                } else {
                    Ok(Async::NotReady)
                }
            }            
        }
    }
}

fn wait_message_processing<S>(changes_stream: S) -> Result<TransactionId, String>
    where
        S: Stream,
        <S as futures::Stream>::Error: std::fmt::Display
    {

    let mut tr_id = None;
    for state in changes_stream.wait() {
        if let Err(e) = state {
            return Err(format!("error next state getting: {}", e));
        }
        if let Ok(s) = state {
            //println!("message: {}  next state: {}", s.message_id.to_hex_string(), s.state);
            if s.message_state == MessageProcessingStatus::Finalized {
                tr_id = Some(s.message_id.clone());
                break;
            }
        }
    }
    tr_id.ok_or("Error: no transaction id")?;

    let tr = Transaction::load(tr_id)
        .map_err(|err| format!("Error calling load Transaction: {}", err))?
        .limit(Duration::from_secs(REQUEST_TIMEOUT))
        .wait()
        .next()
        .ok_or("Error unwrap stream next while loading Transaction")?
        .ok_or("Error unwrap result while loading Transaction")?
        .ok_or("Error unwrap returned Transaction")?;

    if tr.tr().is_aborted() {
        Err(format!("transaction aborted!\n\n{}", serde_json::to_string_pretty(tr.tr()).unwrap()))
    }
}

// Create message "from wallet" to transfer some funds 
// from one account to another
pub fn create_external_transfer_funds_message(src: AccountId, dst: AccountId, value: u128) -> Message {
    
    let mut rng = thread_rng();    
    let mut msg = Message::with_ext_in_header(
        ExternalInboundMessageHeader {
            src: MsgAddressExt::with_extern(&Bitstring::from(rng.gen::<u64>())).unwrap(),
            dst: MsgAddressInt::with_standart(None, 0, src.clone()).unwrap(),
            import_fee: Grams::default(),
        }
    );

    let mut balance = CurrencyCollection::default();
    balance.grams = Grams(value.into());

    let int_msg_hdr = InternalMessageHeader::with_addresses(
            MsgAddressInt::with_standart(None, 0, src).unwrap(),
            MsgAddressInt::with_standart(None, 0, dst).unwrap(),
            balance);

    msg.body = Some(int_msg_hdr.write_to_new_cell().unwrap().into());

    msg
}

fn deploy_contract_and_wait(code_file_name: &str, abi: &str, constructor_params: &str, key_pair: &Keypair) 
    -> Result<AccountId, String>  {

    // read image from file and construct ContractImage
    let mut state_init = std::fs::File::open(code_file_name)
        .map_err(|err| format!("Unable to open contract code file: {}", err))?;

    let contract_image = ContractImage::from_state_init_and_key(&mut state_init, &key_pair.public)
        .map_err(|err| format!("Unable to parse contract code file: {}", err))?;

    let account_id = contract_image.account_id();

    // before deploying contract need to transfer some funds to its address
    //println!("Account ID to take some grams {}\n", account_id);
    let msg = create_external_transfer_funds_message(AccountId::from([0_u8; 32]), account_id.clone(), 100000000000);
    let changes_stream = Contract::send_message(msg)
        .map_err(|err| format!("Error calling contract method: {}", err))?
        .limit(Duration::from_secs(REQUEST_TIMEOUT));

    // wait transaction id in message-status 
    let mut tr_id = None;
    for state in changes_stream.wait() {
        if let Err(e) = state {
            return Err(format!("error next state getting: {}", e));
        }
        if let Ok(s) = state {
            //println!("message: {}  next state: {}", s.message_id.to_hex_string(), s.state);
            if s.message_state == MessageProcessingStatus::Finalized {
                tr_id = Some(s.message_id.clone());
                break;
            }
        }
    }
    tr_id.ok_or("Error: no transaction id")?;


    // call deploy method
    let changes_stream = Contract::deploy_json("constructor".to_owned(), constructor_params.to_owned(), abi.to_owned(), contract_image, Some(key_pair))
        .map_err(|err| format!("Error deploying contract: {}", err))?;

    // wait transaction id in message-status 
    let mut tr_id = None;
    for state in changes_stream.wait() {
        if let Err(e) = state {
            panic!("error next state getting: {}", e);
        }
        if let Ok(s) = state {
            //println!("next state: {:?}", s);
            if s.message_state == MessageProcessingStatus::Finalized {
                tr_id = Some(s.message_id.clone());
                break;
            }
        }
    }
    // contract constructor doesn't return any values so there are no output messages in transaction
    // so just check deployment transaction created
    let _tr_id = tr_id.expect("Error: no transaction id");

    Ok(account_id)
}


fn call_contract_and_wait(address: AccountId, func: &str, input: &str, abi: &str, key_pair: Option<&Keypair>) -> String {

    let contract = Contract::load(address)
        .expect("Error calling load Contract")
        .wait()
        .next()
        .expect("Error unwrap stream next while loading Contract")
        .expect("Error unwrap result while loading Contract")
        .expect("Error unwrap contract while loading Contract");

    // call needed method
    let changes_stream = 
        Contract::call_json(contract.id(), func.to_owned(), input.to_owned(), abi.to_owned(), key_pair)
            .expect("Error calling contract method");

    // wait transaction id in message-status 
    let mut tr_id = None;
    for state in changes_stream.wait() {
        if let Err(e) = state {
            panic!("error next state getting: {}", e);
        }
        if let Ok(s) = state {
            //println!("next state: {:?}", s);
            if s.message_state == MessageProcessingStatus::Finalized {
                tr_id = Some(s.message_id.clone());
                break;
            }
        }
    }
    let tr_id = tr_id.expect("Error: no transaction id");

    // OR 
    // wait message will done and find transaction with the message

    // load transaction object
    let tr = Transaction::load(tr_id)
        .expect("Error calling load Transaction")
        .wait()
        .next()
        .expect("Error unwrap stream next while loading Transaction")
        .expect("Error unwrap result while loading Transaction")
        .expect("Error unwrap got Transaction");

    // take external outbound message from the transaction
    let out_msg = tr.load_out_messages()
        .expect("Error calling load out messages")
        .wait()
        .find(|msg| {
            msg.as_ref()
                .expect("error unwrap out message 1")
                .as_ref()
                    .expect("error unwrap out message 2")
                    .msg_type() == MessageType::ExternalOutbound
        })
            .expect("erro unwrap out message 2")
            .expect("erro unwrap out message 3")
            .expect("erro unwrap out message 4");

    // take body from the message
    let responce = out_msg.body().expect("error unwrap out message body").into();

    // decode the body by ABI
    let result = Contract::decode_function_response_json(abi.to_owned(), func.to_owned(), responce)
        .expect("Error decoding result");

    //println!("Contract call result: {}\n", result);

    result

    // this way it is need:
    // 1. message status with transaction id or transaction object with in-message id
    // 2. transaction object with out messages ids
    // 3. message object with body
}

/*fn call_create(current_address: &mut Option<AccountId>) {
    println!("Creating new wallet account");

    // generate key pair
    let mut csprng = rand::rngs::OsRng::new().unwrap();
    let keypair = Keypair::generate::<Sha512, _>(&mut csprng);
   
    // deploy wallet
    let wallet_address = deploy_contract_and_wait("Wallet.tvc", WALLET_ABI, "{}", &keypair);
    let str_address = hex::encode(wallet_address.as_slice());

    println!("Acoount created. Address {}", str_address);

    std::fs::write("last_address", wallet_address.as_slice()).expect("Couldn't save wallet address");
    std::fs::write(str_address, &keypair.to_bytes().to_vec()).expect("Couldn't save wallet key pair");

    *current_address = Some(wallet_address);
}*/

fn call_get_balance(current_address: &Option<AccountId>, params: &[&str]) {
    let address = if params.len() > 0 {
        AccountId::from(hex::decode(params[0]).unwrap())
    } else {
        if let Some(addr) = current_address.clone() {
            addr
        } else {
            println!("Current address not set");
            return;
        }
    };

    let contract = Contract::load(address)
        .expect("Error calling load Contract")
        .wait()
        .next()
        .expect("Error unwrap stream next while loading Contract")
        .expect("Error unwrap result while loading Contract")
        .expect("Error unwrap contract while loading Contract");

    let nanogram_balance = contract.balance_grams();
    let nanogram_balance = nanogram_balance.get_value().to_u128().expect("error cust grams to u128");
    let gram_balance = nanogram_balance as f64 / 1000000000f64;

    println!("Account balance {}", gram_balance);
}

#[derive(Deserialize)]
struct SendTransactionAnswer {
    transaction: String,
    error: String
}

fn call_send_transaction(current_address: &Option<AccountId>, params: &[&str]) {
    if params.len() < 2 {
        println!("Not enough parameters");
        return;
    }

    let address = if let Some(addr) = current_address {
        addr.clone()
    } else {
        println!("Current address not set");
        return;
    };

    println!("Sending {} grams to {}", params[1], params[0]);

    let nanogram_value = params[1].to_owned() + "000000000";

    let str_params = format!("{{ \"recipient\" : \"x{}\", \"value\": \"{}\" }}", params[0], nanogram_value);

    let pair = std::fs::read(hex::encode(address.as_slice())).expect("Couldn't read key pair");
    let pair = Keypair::from_bytes(&pair).expect("Couldn't restore key pair");

    let answer = call_contract_and_wait(address, "sendTransaction", &str_params, WALLET_ABI, Some(&pair));


    let answer: SendTransactionAnswer = serde_json::from_str(&answer).unwrap();

    let transaction = u64::from_str_radix(&answer.transaction[2..], 16).expect("Couldn't parse transaction number");

    println!("Transaction ID {}", transaction);
}

fn attempts_thread(stat: Arc<Mutex<Statistic>>, params: TesterParameters) {
    let mut inited = false;
    let mut acc_addresses: Vec<Option<AccountId>> = vec![None; params.acc_count as usize];
    let mut acc_keys: Vec<Keypair> = Vec::with_capacity(params.acc_count as usize);
    let mut next_acc = 0;

    loop {
        if !inited {

            inited = true;
        }

        /*for acc_stuff in accounts_stuff.iter_mut() {
            if acc_stuff.is_none() {
                // Deploy

            }
        }*/

        for i in 0..params.tr_count {


            next_acc = (next_acc + 1) % params.acc_count;
        }


        std::thread::sleep(std::time::Duration::from_secs(params.tr_interval));
    }
}

fn report_thread() {

}

fn main() {

    let stat: Arc<Mutex<Statistic>>;


    loop {
        // init if need

        // deploy if need

        // 
    }
}
