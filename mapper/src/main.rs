use std::env;

mod errors;
mod mapper;
use errors::Error;
use mapper::mapper_wait;

async fn wait_main(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    if args.len() < 3 {
        let err = Error::MissingWaitArg(args[0].parse::<String>()?);
        println!("{}", err);
        return Err(err)?;
    }
    let obj_path = args[2].parse::<String>()?;
    mapper_wait(obj_path).await?;
    Ok(())
}

async fn subtree_main(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    if args.len() < 3 {
        let err = Error::MissingSubtreeRemoveArg(args[0].parse::<String>()?);
        println!("{}", err);
        return Err(err)?;
    }
    let namespace_interface = args[2].parse::<String>()?;
    let split = namespace_interface.split(':');
    let split_strs = split.collect::<Vec<&str>>();
    if split_strs.len() < 2 {
        let err = Error::InvalidSubtreeRemoveArg(namespace_interface);
        println!("{}", err);
        return Err(err)?;
    }
    let _namespace = split_strs[0];
    let _interface = split_strs[1];

    println!("TODO(wltu): subtree_main not implmented yet.");
    Ok(())
}

async fn get_service_main(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    if args.len() < 3 {
        let err = Error::MissingGetServiceArg(args[0].parse::<String>()?);
        println!("{}", err);
        return Err(err)?;
    }

    let _obj_path = args[2].parse::<String>()?;
    println!("TODO(wltu): get_service_main not implmented yet.");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        let err = Error::MissingCommand;
        println!("Missing Command Arg.\n{}", err);
        return Err(err)?;
    }
    let command = args[1].parse::<String>();
    match command {
        Ok(command_str) => match command_str.as_str() {
            "wait" => wait_main(args).await?,
            "subtree-remove" => subtree_main(args).await?,
            "get-service" => get_service_main(args).await?,
            invalid_command => {
                let err = Error::InvalidCommand;
                println!("invalid command: {}:\n{}", invalid_command, err);
                return Err(err)?;
            }
        },
        Err(e) => {
            println!("failed to parse command: {}", e);
            return Err(e)?;
        }
    }

    Ok(())
}
