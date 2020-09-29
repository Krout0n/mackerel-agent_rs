use mackerel_client::{client::Client, metric};
use std::{collections::HashMap, time::Duration};
use tokio::time;

#[derive(Debug)]
pub struct Config {
    pub api_key: String,
    pub apibase: String,
}

impl Config {
    pub fn new() -> Self {
        Self {
            api_key: String::new(),
            apibase: String::new(),
        }
    }

    pub fn from_ini(ini: ini::Ini) -> Self {
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

#[derive(Debug)]
pub struct Values(HashMap<String, f64>);
// &'a str expects host id.
pub struct HostMetricWrapper<'a>(&'a str, Values);

impl<'a> Into<Vec<metric::HostMetricValue>> for HostMetricWrapper<'a> {
    fn into(self) -> Vec<metric::HostMetricValue> {
        use std::time::SystemTime;
        let host_id = self.0;
        let value = self.1;
        let host_metric_value = value.0;
        let now = match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
            Ok(n) => n.as_secs(),
            Err(_) => panic!("SystemTime before UNIX EPOCH!"),
        };
        host_metric_value
            .into_iter()
            .map(|hmv| {
                let (name, value) = hmv;
                metric::HostMetricValue {
                    host_id: host_id.to_owned(),
                    name,
                    value,
                    time: now,
                }
            })
            .collect()
    }
}

#[derive(Debug)]
pub struct Executor {
    pub config: Config,
    pub client: Client,
    pub host_id: String,
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
        let mut interval = time::interval(Duration::from_secs(5));
        loop {
            interval.tick().await;
            let cpu_metric = self.get_cpu_metrics().await.unwrap();
            self.send_metric(cpu_metric).await;
        }
    }

    async fn send_metric(&self, val: Values) {
        let metric = HostMetricWrapper(&self.host_id, val).into();
        self.client.post_metrics(metric).await;
    }
}

pub mod cpu;
