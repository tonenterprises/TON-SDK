use ton_sdk::*;

pub fn deploy(
    config: &str, 
    contract: &str, 
    wc: &str, 
    key_file: Option<&str>,
) -> Result<(), String> {
    let wc = i32::from_str_radix(wc, 10)
        .map_err(|_| "invalid workchain id".to_string())?;
    
    init_json(Some(wc), config.clone())
        .map_err(|e| format!("couldn't establish connection: {}", e))?;

}

pub fn call(
    config: &str,
    address: &str, 
    method: &str, 
    abi: &str, 
    params: &str, 
    key_file: Option<&str>
) -> Result<(), String> {
    let addr_and_wc: Vec<&str> = address.split(':').collect();
    if addr_and_wc.len() != 2 {
        Err("invalid address format".to_string())?;
    }
    let wc = i32::from_str_radix(addr_and_wc[0], 10)
        .map_err(|_| "invalid workchain id".to_string())?;
    let account_id = AccountId::from(addr_and_wc[1]);
    

    let keys = match key_file {
        Some(file) => {
            let bytes = std::fs::read(file)?;
            Some(
                Keypair::from_bytes(&bytes)
                    .map_err(|e| format!("couldn't load keypair from file: {}", e)?
            );
        },
        None => None,        
    };

    init_json(Some(wc), config.clone()).map_err(|e| format!("couldn't establish connection: {}", e))?;

    let answer = call_contract_and_wait(account_id, method, params, abi, keys.as_ref());
}


fn call_contract_and_wait(
    address: AccountId, 
    func: &str, 
    input: &str, 
    abi: &str, 
    key_pair: Option<&Keypair>
) -> Result<(),String> {

    let contract = Contract::load(address.into())
        .map_err(|_| "error calling load contract".to_string())?
        .wait()
        .next()
        .map_err(|_| "error unwrap stream next while loading Contract")?
        .map_err(|_| "error unwrap result while loading Contract")?
        .map_err(|_| "error unwrap contract while loading Contract")?;

    let changes_stream = Contract::call_json(
        contract.id().into(), 
        func.to_owned(), 
        input.to_owned(), 
        abi.to_owned(), 
        key_pair
    ).map_err(|_| "Error calling contract method".to_string())?;

    // wait transaction id in message-status 
    let tr_id = wait_message_processed(changes_stream);

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
    let responce = out_msg.body().expect("error unwrap out message body");

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


fn wait_message_processed(
    changes_stream: Box<dyn Stream<Item = ContractCallState, Error = ton_sdk::SdkError>>
    ) -> TransactionId
{
    let mut tr_id = None;
    for state in changes_stream.wait() {
        if let Err(e) = state {
            panic!("error next state getting: {}", e);
        }
        if let Ok(s) = state {
            println!("{} : {:?}", s.message_id.to_hex_string(), s.message_state);
            if is_message_done(s.message_state) {
                tr_id = Some(s.message_id.clone());
                break;
            }
        }
    }
    tr_id.expect("Error: no transaction id")
}


// Create message "from wallet" to transfer some funds 
// from one account to another
pub fn create_external_transfer_funds_message(src: AccountId, dst: AccountId, value: u128) -> Message {
    
    let mut rng = thread_rng();    
    let mut builder = BuilderData::new();
    builder.append_u64(rng.gen::<u64>()).unwrap();    
    let mut msg = Message::with_ext_in_header(
        ExternalInboundMessageHeader {
            src: MsgAddressExt::with_extern(builder.into()).unwrap(),
            dst: MsgAddressInt::with_standart(None, 0, src.clone()).unwrap(),
            import_fee: Grams::default(),
        }
    );

    let mut balance = CurrencyCollection::default();
    balance.grams = Grams(value.into());

    let workchain = Contract::get_default_workchain().unwrap();

    let int_msg_hdr = InternalMessageHeader::with_addresses(
            MsgAddressInt::with_standart(None, workchain as i8, src).unwrap(),
            MsgAddressInt::with_standart(None, workchain as i8, dst).unwrap(),
            balance);

    *msg.body_mut() = Some(int_msg_hdr.write_to_new_cell().unwrap().into());

    msg
}

fn deploy_contract_and_wait(code_file_name: &str, abi: &str, constructor_params: &str, key_pair: &Keypair) -> AccountId {
    // read image from file and construct ContractImage
    let mut state_init = std::fs::File::open(code_file_name).expect("Unable to open contract code file");

    let contract_image = ContractImage::from_state_init_and_key(&mut state_init, &key_pair.public).expect("Unable to parse contract code file");

    let account_id = contract_image.account_id();

    // before deploying contract need to transfer some funds to its address
    //println!("Account ID to take some grams {}\n", account_id.to_hex_string());
    let msg = create_external_transfer_funds_message(AccountId::from([0_u8; 32]), account_id.clone(), 100000000000);
    let changes_stream = Contract::send_message(msg).expect("Error calling contract method");

    // wait transaction id in message-status 
    let tr_id = wait_message_processed(changes_stream);
    
    let tr = Transaction::load(tr_id)
        .expect("Error load Transaction")
        .wait()
        .next()
        .expect("Error unwrap stream next while loading Transaction")
        .expect("Error unwrap result while loading Transaction")
        .expect("Error unwrap returned Transaction");

    //println!("transaction:\n\n{}", serde_json::to_string_pretty(tr.tr()).unwrap());

    if tr.tr().is_aborted() {
        panic!("transaction aborted!\n\n{}", serde_json::to_string_pretty(tr.tr()).unwrap())
    }

    tr.out_messages_id().iter().for_each(|msg_id| {
        wait_message_processed_by_id(msg_id.clone());
    });

    // call deploy method
    let changes_stream = Contract::deploy_json("constructor".to_owned(), constructor_params.to_owned(), abi.to_owned(), contract_image, Some(key_pair))
        .expect("Error deploying contract");

    // wait transaction id in message-status 
    // contract constructor doesn't return any values so there are no output messages in transaction
    // so just check deployment transaction created
    let tr_id = wait_message_processed(changes_stream);

    let tr = Transaction::load(tr_id)
        .expect("Error calling load Transaction")
        .wait()
        .next()
        .expect("Error unwrap stream next while loading Transaction")
        .expect("Error unwrap result while loading Transaction")
        .expect("Error unwrap returned Transaction");

    if tr.tr().is_aborted() {
        panic!("transaction aborted!\n\n{}", serde_json::to_string_pretty(tr.tr()).unwrap())
    }

    account_id
}