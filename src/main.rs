use clap::{load_yaml, App};
use ini::Ini;
use mackerel_client::{client::Client, metric};
use os_stat_rs::cpu;
use std::{collections::HashMap, fs::File, io::prelude::*, path::Path, time::Duration};
use tokio::time;

const HOST_PATH: &'static str = "/var/lib/mackerel-agent";
// TODO: change path as /var/lib/mackerel-agent/id
const HOST_ID_PATH: &'static str = "./id";

#[derive(Debug)]
struct Values(HashMap<String, f64>);

#[derive(Debug)]
struct Executor {
    pub config: Config,
    pub client: Client,
    pub host_id: String,
}

impl From<(cpu::CPU, cpu::CPU)> for Values {
    fn from((previous, current): (cpu::CPU, cpu::CPU)) -> Self {
        let mut value = HashMap::new();
        let total_diff = (current.total - previous.total) as f64;
        let cpu_count = current.cpu_count as f64;

        macro_rules! val_insert_inner {
            ($key:expr, $val:expr) => {
                value.insert(
                    $key.into(),
                    $val as f64 * cpu_count as f64 * 100.0 / total_diff,
                );
            };
        }
        val_insert_inner!(
            "cpu.user.percentage",
            (current.user - current.guest) - (previous.user - previous.guest)
        );

        macro_rules! val_insert {
            ($field:ident) => {
                let field = stringify!($field);
                let key = format!("cpu.{}.percentage", field);
                val_insert_inner!(key, current.$field - previous.$field)
            };
        }

        val_insert!(nice);
        val_insert!(system);
        val_insert!(idle);

        macro_rules! val_insert_if_bigger {
            ($stat_count:expr, $field:ident) => {
                if current.stat_count >= $stat_count {
                    val_insert!($field);
                }
            };
        }

        val_insert_if_bigger!(5, iowait);
        val_insert_if_bigger!(6, irq);
        val_insert_if_bigger!(7, softirq);
        val_insert_if_bigger!(8, steal);
        val_insert_if_bigger!(9, guest);
        Self(value)
    }
}

impl Executor {
    pub fn new(config: Config, host_id: String) -> Self {
        Self {
            client: Client::new(&config.api_key.clone()),
            config,
            host_id,
        }
    }

    pub async fn run(&self) {
        let mut interval = time::interval(Duration::from_secs(1));
        loop {
            interval.tick().await;
            let _ = dbg!(cpu::get());
        }
    }
}

#[derive(Debug)]
struct Config {
    api_key: String,
    apibase: String,
}

impl Config {
    fn new() -> Self {
        Self {
            api_key: String::new(),
            apibase: String::new(),
        }
    }

    fn from_ini(ini: ini::Ini) -> Self {
        let mut conf = Self::new();
        let map = &ini
            .iter()
            .map(|(_, val)| val.iter().collect::<HashMap<_, _>>())
            .collect::<Vec<_>>()[0];
        conf.api_key = map.get("apikey").unwrap().to_string();
        conf.apibase = map
            .get("apibase")
            .unwrap_or(&"https://api.mackerelio.com/")
            .to_string();
        conf
    }
}

// Register the running host or get host own id.
async fn initialize(client: &Client) -> std::io::Result<String> {
    // if !Path::exists(Path::new(HOST_PATH)) {
    //     std::fs::create_dir(HOST_PATH)?;
    // }
    Ok(if let Ok(file) = File::open(HOST_ID_PATH) {
        let mut file = file;
        let mut buf = String::new();
        if file.read_to_string(&mut buf).is_err() {
            unimplemented!()
        }
        buf
    } else {
        let hostname = hostname::get();
        if hostname.is_err() {
            todo!();
        }
        let hostname = hostname.unwrap().to_str().unwrap().to_owned();
        let param = mackerel_client::create_host_param!({name -> hostname});
        let result = client.create_host(param).await;
        if result.is_err() {
            unimplemented!();
        }
        let registerd_host_id = result.unwrap();
        let mut file = File::create(HOST_ID_PATH)?;
        file.write(registerd_host_id.as_bytes())?;
        registerd_host_id
    })
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let yaml = load_yaml!("../cli.yml");
    let matches = App::from_yaml(yaml).get_matches();
    let path = Path::new(
        matches
            .value_of("config")
            .unwrap_or("/src/mackerel-agent.conf"),
    );
    let ini = Ini::load_from_file(path).unwrap();
    let conf = dbg!(Config::from_ini(ini));
    let client = Client::new(&conf.api_key);
    let host_id = initialize(&client).await;
    if host_id.is_err() {
        todo!()
    }
    let executor = Executor::new(conf, host_id.unwrap());
    executor.run().await;
    Ok(())
}
