extern crate micro_sf_client;
#[macro_use]
extern crate serde_derive;
extern crate structopt;
#[macro_use]
extern crate structopt_derive;

mod error;
mod config;

use structopt::StructOpt;

use config::Config;
use micro_sf_client::SFClient;

#[derive(StructOpt, Debug)]
#[structopt(name = "Micro SF CLI", about = "An example micro SalesForce client")]
struct Options {
    /// A path to the config file to load
    #[structopt(short = "c", long = "config", help = "Path to config file")]
    config: String,

    /// A query to run against the SalesForce API
    #[structopt(short = "q", long = "query", help = "Query to run against the API")]
    query: String,
}

fn main() {
    let options = Options::from_args();

    Config::parse_config(options.config.as_str()).and_then(|c| {
        let create_client = SFClient::new(
            c.login_url,
            c.version,
            c.client_id,
            c.client_secret,
            c.username,
            c.password,
        );

        match create_client {
            Ok(mut client) => {
                client.set_attempt_limit(1);
                let res = client.query(options.query.as_str()).map_err(
                    error::CLIError::Network,
                );

                match res {
                    Ok(response) => println!("{:?}", response),
                    Err(err) => println!("{}", err),
                }
            }
            Err(error) => println!("{}", error),
        };

        Ok(())
    });
}
