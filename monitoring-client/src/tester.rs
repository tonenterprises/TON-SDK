use rand::{thread_rng, Rng};
use ton_block::{Message, MsgAddressExt, MsgAddressInt, InternalMessageHeader, Grams, 
    ExternalInboundMessageHeader, CurrencyCollection, Serializable, 
    MessageProcessingStatus};
use tvm::bitstring::Bitstring;
use tvm::types::{AccountId};
use ed25519_dalek::Keypair;
use futures::Stream;
use sha2::Sha512;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use futures_timer::ext::StreamExt;


use ton_sdk::*;
use super::*;

const REQUEST_TIMEOUT: u64 = 60; // seconds
const WALLET_ABI: &str = r#"{
	"ABI version" : 0,

	"functions" :	[
	    {
	        "inputs": [
	            {
	                "name": "recipient",
	                "type": "bits256"
	            },
	            {
	                "name": "value",
	                "type": "duint"
	            }
	        ],
	        "name": "sendTransaction",
					"signed": true,
	        "outputs": [
	            {
	                "name": "transaction",
	                "type": "uint64"
	            },
							{
	                "name": "error",
	                "type": "int8"
	            }
	        ]
	    },
	    {
	        "inputs": [
						  {
	                "name": "type",
	                "type": "uint8"
	            },
							{
	                "name": "value",
	                "type": "duint"
	            },
							{
	                "name": "meta",
	                "type": "bitstring"
	            }
					],
	        "name": "createLimit",
					"signed": true,
	        "outputs": [
							{
	                "name": "limitId",
	                "type": "uint8"
	            },
							{
	                "name": "error",
	                "type": "int8"
	            }
	        ]
	    },
	    {
	        "inputs": [
							{
	                "name": "limitId",
	                "type": "uint8"
	            },
							{
	                "name": "value",
	                "type": "duint"
	            },
							{
	                "name": "meta",
	                "type": "bitstring"
	            }
	        ],
	        "name": "changeLimitById",
					"signed": true,
	        "outputs": [
							{
	                "name": "error",
	                "type": "int8"
	            }
	        ]
	    },
			{
	        "inputs": [
							{
	                "name": "limitId",
	                "type": "uint8"
	            }
	        ],
	        "name": "removeLimit",
					"signed": true,
	        "outputs": [
							{
	                "name": "error",
	                "type": "int8"
	            }
	        ]
	    },
			{
	        "inputs": [
							{
	                "name": "limitId",
	                "type": "uint8"
	            }
	        ],
	        "name": "getLimitById",
	        "outputs": [
							{
									"name": "limitInfo",
					        "type": "tuple",
					        "components": [
											{
					                "name": "value",
					                "type": "duint"
					            },
											{
					                "name": "type",
					                "type": "uint8"
					            },
											{
					                "name": "meta",
					                "type": "bitstring"
					            }
									]
							},
							{
	                "name": "error",
	                "type": "int8"
	            }
	        ]
	    },
			{
	        "inputs": [],
	        "name": "getLimits",
	        "outputs": [
							{
									"name": "list",
					        "type": "uint8[]"
							},
							{
	                "name": "error",
	                "type": "int8"
	            }
	        ]
	    },
			{
	        "inputs": [],
	        "name": "getVersion",
	        "outputs": [
							{
									"name": "version",
					        "type": "tuple",
					        "components": [
											{
					                "name": "major",
					                "type": "uint16"
					            },
											{
					                "name": "minor",
					                "type": "uint16"
					            }
									]
							},
							{
	                "name": "error",
	                "type": "int8"
	            }
	        ]
	    },
			{
	        "inputs": [],
	        "name": "getBalance",
	        "outputs": [
							{
	                "name": "balance",
	                "type": "uint64"
	            }
	        ]
	    },
			{
	        "inputs": [],
	        "name": "constructor",
	        "outputs": []							
	    },
			{
	        "inputs": [{"name": "address", "type": "bits256" }],
	        "name": "setSubscriptionAccount",
					"signed": true,
	        "outputs": []							
	    },
			{
	        "inputs": [],
	        "name": "getSubscriptionAccount",
	        "outputs": [{"name": "address", "type": "bits256" }]							
	    }
	]
}
"#;

fn wait_message_processing<S>(changes_stream: S) -> Result<Transaction, String>
    where S: Stream<Item = ContractCallState, Error = SdkError> {

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
    let tr_id = tr_id.ok_or("Error: no transaction id")?;

    let tr = Transaction::load(tr_id.clone())
        .map_err(|err| format!("Error calling load Transaction: {}", err))?
        .timeout(Duration::from_secs(REQUEST_TIMEOUT))
        .wait()
        .next()
        .ok_or("Error unwrap stream next while loading Transaction")?
        .map_err(|err| format!("Error while loading Transaction: {}", err))?
        .ok_or("Error unwrap returned Transaction")?;

    if tr.tr().is_aborted() {
        Err(format!("transaction aborted!\n\n{}", serde_json::to_string_pretty(tr.tr()).unwrap()))
    } else {
        Ok(tr)
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
    -> Result<AccountId, String> {

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
        .timeout(Duration::from_secs(REQUEST_TIMEOUT));

    wait_message_processing(changes_stream)?;


    // call deploy method
    let changes_stream = Contract::deploy_json("constructor".to_owned(), 
        constructor_params.to_owned(), abi.to_owned(), contract_image, Some(key_pair))
        .map_err(|err| format!("Error deploying contract: {}", err))?
        .timeout(Duration::from_secs(REQUEST_TIMEOUT));

    wait_message_processing(changes_stream)?;

    Ok(account_id)
}


fn call_contract_and_wait(address: AccountId, func: &str, input: &str, abi: &str, key_pair: Option<&Keypair>)
    -> Result<String, String> {

    let contract = Contract::load(address)
        .map_err(|err| format!("Error calling load Contract: {}", err))?
        .timeout(Duration::from_secs(REQUEST_TIMEOUT))
        .wait()
        .next()
        .ok_or("Error unwrap stream next while loading Contract")?
        .map_err(|err| format!("Error while loading Contract: {}", err))?
        .ok_or("Error unwrap returned Contract")?;

    // call needed method
    let changes_stream = Contract::call_json(contract.id(), func.to_owned(), input.to_owned(),
         abi.to_owned(), key_pair)
        .map_err(|err| format!("Error deploying contract: {}", err))?
        .timeout(Duration::from_secs(REQUEST_TIMEOUT));

    let tr = wait_message_processing(changes_stream)?;

    // take external outbound message from the transaction
    let out_msg = tr.load_out_messages()
        .map_err(|err| format!("Error calling load out messages: {}", err))?
        .timeout(Duration::from_secs(REQUEST_TIMEOUT))
        .wait()
        .find(|msg| {
            if let Ok(msg) = msg.as_ref() {
                if let Some(msg) = msg.as_ref() {
                    return msg.msg_type() == MessageType::ExternalOutbound
                }
            }
            false
        })
            .ok_or("erro unwrap out message 2")?
            .map_err(|err| format!("error unwrap out message 3: {}", err))?
            .ok_or("erro unwrap out message 4")?;

    // take body from the message
    let responce = out_msg.body().ok_or("error unwrap out message body")?.into();

    // decode the body by ABI
    let result = Contract::decode_function_response_json(abi.to_owned(), func.to_owned(), responce)
        .map_err(|err| format!("Error decoding result: {}", err))?;

    Ok(result)
}

pub fn attempts_thread(stat: Arc<Mutex<Statistic>>, config: TesterConfig, client_config: NodeClientConfig) {

    let mut inited = false;
    let mut next_acc = 0;
    let mut first_time = true;
    let mut accounts = Vec::with_capacity(config.acc_count);
    let mut last_init = Instant::now();

    'main_loop: loop {
        if !first_time {
            std::thread::sleep(std::time::Duration::from_secs(config.tr_interval));
        }
        first_time = false;

        if last_init + Duration::from_secs(1800) < Instant::now() {
            last_init = Instant::now();
            inited = false;
        }

        if !inited {
            let init_res = init(client_config.clone());
            let mut stat = stat.lock().unwrap();
            match init_res {
                Ok(_) => {
                    stat.log_info("Successfully initialized");
                    inited = true;
                },
                Err(e) => {
                    stat.log_error_attempt(&format!("Error while initialization: {}", e));
                    continue;
                }
            }
        }

        for _ in accounts.len()..config.acc_count {

            let mut csprng = rand::rngs::OsRng::new().unwrap();
            let keypair = Keypair::generate::<Sha512, _>(&mut csprng);

            let deploy_res = deploy_contract_and_wait("Wallet.tvc", WALLET_ABI, "{}", &keypair);

            let mut stat = stat.lock().unwrap();
            match deploy_res {
                Ok(id) => {
                    stat.log_info(&format!("{} successfully deployed", id.to_hex_string()));
                    accounts.push((id, keypair));
                },
                Err(e) => {
                    stat.log_error_attempt(&format!("Error while deploying: {}", e));
                    continue 'main_loop;
                }
            }
        }

        for _ in 0..config.tr_count {

            let (acc1, keypair) = accounts.get(next_acc).unwrap();
            let (acc2, _) = accounts.get((next_acc + 1) % config.acc_count).unwrap();

            let str_params = format!("{{ \"recipient\" : \"x{}\", \"value\": \"{}\" }}", acc2.to_hex_string(), 100);

            let res = call_contract_and_wait(acc1.clone(), "sendTransaction", &str_params, WALLET_ABI, Some(keypair));

            let mut stat = stat.lock().unwrap();
            match res {
                Ok(_) => {
                    stat.log_success_attempt(&format!("Founds from {} to {} successfully transferred", 
                        acc1.to_hex_string(), acc2.to_hex_string()));
                },
                Err(e) => {
                    stat.log_error_attempt(&format!("Error while transferring founds: {}", e));
                    continue 'main_loop;
                }
            }

            next_acc = (next_acc + 1) % config.acc_count;
        }
    }
}