use josefine_raft::config::RaftConfig;
use josefine_broker::config::BrokerConfig;

#[serde(default)]
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct JosefineConfig {
    pub raft: RaftConfig,
    pub broker: BrokerConfig,
}

pub fn config<P: AsRef<std::path::Path>>(config_path: P) -> JosefineConfig {
    let mut settings = config::Config::default();
    settings
        .merge(config::File::from(config_path.as_ref()))
        .expect("Could not read configuration file")
        .merge(config::Environment::with_prefix("JOSEFINE"))
        .expect("Could not read environment variables");

    settings.try_into().expect("Could not create configuration")
}