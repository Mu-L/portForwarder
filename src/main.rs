#![allow(non_snake_case)]

mod tcp_forwarder;
mod udp_forwarder;
mod tcp_udp_forwarder;
mod utils;
use tcp_udp_forwarder::*;
use regex::Regex;
use std::fs;
use yaml_rust::{YamlLoader, Yaml};


fn usage() {
    let args: Vec<String> = std::env::args().collect();
    println!(
"usage:
    {} [-htu] <bind-address> <forward-address>
    {} -c <yaml-config-file>

    -t    disable tcp
    -u    disable udp
    -c    config file (a yaml file)
    -e    show an example of config file
    -h    show help", args[0], args[0]);
}

fn print_example_of_config_file() {
    println!(
"forwarders:
  - local: <bind-address/0.0.0.0:1234>
    remote: <remote-address/127.0.0.1:2233>
    enable_tcp: true # default is true
    enable_udp: true # default is true");
}

#[derive(Clone)]
struct ForwardSessionConfig {
    local: String,
    remote: String,
    enable_tcp: bool,
    enable_udp: bool,
}

impl ForwardSessionConfig {
    fn run(&self) -> std::thread::JoinHandle<()> {
        let cp = self.clone();
        std::thread::spawn(move || {
            let forwarder = TcpUdpForwarder::from(&cp.local, &cp.remote, cp.enable_udp, cp.enable_tcp).unwrap();
            forwarder.listen();
        })
   }

    fn from(yaml: &Yaml) -> Result<Self,&'static str> {
        let local = match yaml["local"].as_str() {
            Some(s) => String::from(s),
            None => return Err("missing local"),
        };
        let remote = match yaml["remote"].as_str() {
            Some(s) => String::from(s),
            None => return Err("missing remote"),
        };
        let enable_tcp = yaml["enable_tcp"].as_bool().unwrap_or(true);
        let enable_udp = yaml["enable_udp"].as_bool().unwrap_or(true);

        Ok(Self {local, remote, enable_tcp, enable_udp})
    }
}

fn main() {
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"));
    let mut bind_addr = None;
    let mut forward_addr = None;
    let mut enable_tcp = true;
    let mut enable_udp = true;
    let mut args: Vec<String> = std::env::args().collect();
    let mut config_file: Option<String> = None;
    args.remove(0);
    let valid_ipv4_port = Regex::new(r"^([0-9]{1,3}.){3}[0-9]{1,3}:[0-9]{1,5}$").unwrap();
    for i in 0..args.len() {
        let s = args[i].as_str();
        match s {
            "-h" => {
                usage();
                std::process::exit(0);
            }
            "-u" => {
                enable_udp = false;
            }
            "-t" => {
                enable_tcp= false;
            }
            "-e" => {
                print_example_of_config_file();
                std::process::exit(0);
            }
            "-c" => {
                if i + 1 < args.len() {
                    config_file = Some(args[i+1].clone());
                    break;
                } else {
                    usage();
                    std::process::exit(1);
                }
            }
            _ => {
                if valid_ipv4_port.is_match(s) {
                    if bind_addr.is_none() {
                        bind_addr = Some(String::from(s));
                    } else if forward_addr.is_none() {
                        forward_addr = Some(String::from(s));
                    } else {
                        usage();
                        std::process::exit(1);
                    }
                } else {
                    usage();
                    std::process::exit(1);
                }
            }
        }
    }

    let mut forwarder_configs = vec![];
    if config_file.is_some() {
        let file = &config_file.unwrap();
        if let Ok(file_content) = fs::read_to_string(file) {
            if let Ok(config) = YamlLoader::load_from_str(file_content.as_str()) {
                if config.is_empty() {
                    println!("do nothing");
                    std::process::exit(0);
                }
                let config = &config[0];

                let forwarders = &config["forwarders"];
                if !forwarders.is_array() {
                    println!("invalid config file, expect an array but get {:?}", forwarders);
                    std::process::exit(1);
                }

                let fflist = forwarders.as_vec().unwrap();
                for ff in fflist {
                    match ForwardSessionConfig::from(ff) {
                        Ok(c) => forwarder_configs.push(c),
                        Err(e) => {
                            println!("invalid config file: {e}\n{:?}", ff);
                            std::process::exit(1);
                        }
                    }
                }
            } else {
                println!("invalid config file, should be a valid yaml file");
                std::process::exit(1);
            }
        } else {
            println!("open file {} failed", file);
            std::process::exit(1);
        }
    } else {
        if bind_addr.is_none() || forward_addr.is_none() {
            usage();
            std::process::exit(1);
        }

        forwarder_configs.push(
            ForwardSessionConfig {
                local: bind_addr.unwrap(),
                remote: forward_addr.unwrap(),
                enable_tcp,
                enable_udp
            }
        );
    }

    let mut handlers = vec![];
    for cc in forwarder_configs {
        handlers.push(cc.run());
    }

    for h in handlers {
        h.join().unwrap();
    }
}
