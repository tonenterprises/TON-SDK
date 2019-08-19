extern crate clap;
extern crate ton_sdk;

use cmds::{deploy_contract, call_contract};
use ton_sdk::init_json;
use tvm::block::*;

fn load_config(config: Option<&str>) -> Result<String, String> {
    std::fs::read_to_string(config_file.unwrap_or("config")).map_err("Couldn't read config file".to_string())
}

fn main() -> Result<(), String> {
    let result = tlkit_main();
    if let Err(ref err_str) = result {
        println!("Error: {}", err_str);
    }
    result
}

fn tlkit_main() -> Result<(), String> {
    let matches = clap_app! (tvm_loader =>
        (version: "0.1")
        (author: "TonLabs")
        (about: "SDK console client")
        (@subcommand deploy => 
            (about: "Deploy smart contract to TON blockchain")
            (version: "0.1")
            (author: "tonlabs")
            (@arg CONTRACT: +required +takes_value "Compiled smart contract file")
            (@arg WORKCHAIN: -w --workchain +takes_value "Contract workchain ID")
            (@arg DEBUG: --debug "Enable debug printing")
        )
        (@subcommand call => 
            (about: "Call smart contract method")
            (version: "0.1")
            (author: "TonLabs")
            (@arg ADDRESS: +required +takes_value "Smart contract address in the form of <wc_id:32bytes>")
            (@arg METHOD: +required +takes_value "The name of the calling method")
            (@arg JSON: +required +takes_value "Path to contract ABI file")
            (@arg PARAMS: +required +takes_value "Method arguments")
            (@arg SIGN: --sign +takes_value "Path to keypair file for signing message")
            (@arg GET: --get +takes_value "Path to keypair file for signing message")
            (@arg DEBUG: --debug "Enable debug printing")
        )
        (@setting SubcommandRequired)
    ).get_matches();

    let config = load_config(matches.value_of("config"))?;

    if let Some(deploy_matches) = matches.subcommand_matches("deploy") {
        deploy(
            &config,
            deploy_matches.value_of("CONTRACT").unwrap(),
            deploy_matches.value_of("WORKCHAIN").unwrap(),
        )?;
    }

    if let Some(call_matches) = matches.subcommand_matches("call") {
        call(
            &config,
            call_matches.value_of("ADDRESS").unwrap(),
            call_matches.value_of("METHOD").unwrap(),
            call_matches.value_of("JSON").unwrap(),
            call_matches.value_of("PARAMS").unwrap(),
        )?;
    }

    ok!()
}
