use crate::models::{
    FanModeSelection, MinerOptionSettings, MiningModeSelection, PoolInput, TuningTargetSelection,
};
use asic_rs::MinerFactory;
use asic_rs_core::{
    config::{
        fan::FanConfig,
        pools::{PoolConfig, PoolGroupConfig},
        scaling::ScalingConfig,
        tuning::TuningConfig,
    },
    data::pool::PoolURL,
    data::{
        hashrate::{HashRate, HashRateUnit},
        miner::{MiningMode, TuningTarget},
    },
};
use futures::stream::{self, StreamExt};
use measurements::Power;

fn map_mining_mode(mode: MiningModeSelection) -> MiningMode {
    match mode {
        MiningModeSelection::Low => MiningMode::Low,
        MiningModeSelection::Normal => MiningMode::Normal,
        MiningModeSelection::High => MiningMode::High,
    }
}

fn map_mining_mode_selection(mode: MiningMode) -> MiningModeSelection {
    match mode {
        MiningMode::Low => MiningModeSelection::Low,
        MiningMode::Normal => MiningModeSelection::Normal,
        MiningMode::High => MiningModeSelection::High,
    }
}

fn tuning_target_from_settings(settings: &MinerOptionSettings) -> TuningTarget {
    match settings.tuning_target {
        TuningTargetSelection::MiningMode => {
            TuningTarget::MiningMode(map_mining_mode(settings.mining_mode))
        }
        TuningTargetSelection::Power => {
            TuningTarget::Power(Power::from_watts(settings.tuning_power_watts))
        }
        TuningTargetSelection::Hashrate => TuningTarget::HashRate(HashRate {
            value: settings.tuning_hashrate_ths,
            unit: HashRateUnit::TeraHash,
            algo: settings.tuning_hashrate_algo.trim().to_string(),
        }),
    }
}

pub async fn fetch_current_options(
    ip: String,
    defaults: MinerOptionSettings,
) -> Result<MinerOptionSettings, String> {
    let factory = MinerFactory::new();
    let parsed_ip = ip
        .parse()
        .map_err(|e| format!("Invalid IP address {ip}: {e}"))?;

    let Some(miner) = factory
        .get_miner(parsed_ip)
        .await
        .map_err(|e| format!("Failed to connect to {ip}: {e}"))?
    else {
        return Err(format!("No supported miner found at {ip}"));
    };

    let mut settings = defaults;

    if let Some(target) = miner.get_tuning_target().await {
        match target {
            TuningTarget::Power(power) if miner.supports_set_power_limit() => {
                settings.apply_power_limit = true;
                settings.power_limit_watts = power.as_watts();
                settings.tuning_target = TuningTargetSelection::Power;
                settings.tuning_power_watts = power.as_watts();
            }
            TuningTarget::MiningMode(mode) if miner.supports_tuning_config() => {
                settings.apply_tuning_config = true;
                settings.tuning_target = TuningTargetSelection::MiningMode;
                settings.mining_mode = map_mining_mode_selection(mode);
            }
            TuningTarget::HashRate(hashrate) if miner.supports_tuning_config() => {
                settings.apply_tuning_config = true;
                settings.tuning_target = TuningTargetSelection::Hashrate;
                settings.tuning_hashrate_ths =
                    hashrate.clone().as_unit(HashRateUnit::TeraHash).value;
                settings.tuning_hashrate_algo = hashrate.algo;
            }
            _ => {}
        }
    }

    if miner.supports_fan_config() {
        if let Ok(fan_config) = miner.get_fan_config().await {
            settings.apply_fan_config = true;
            match fan_config {
                FanConfig::Auto {
                    target_temp,
                    idle_speed,
                } => {
                    settings.fan_mode = FanModeSelection::Auto;
                    settings.fan_target_temp_c = target_temp;
                    if let Some(idle) = idle_speed {
                        settings.fan_idle_speed_percent = idle;
                    }
                }
                FanConfig::Manual { fan_speed } => {
                    settings.fan_mode = FanModeSelection::Manual;
                    settings.fan_speed_percent = fan_speed;
                }
            }
        }
    }

    if miner.supports_tuning_config() {
        if let Ok(tuning_config) = miner.get_tuning_config().await {
            if let Some(algorithm) = tuning_config.algorithm {
                settings.tuning_algorithm = algorithm;
            }

            match tuning_config.target {
                TuningTarget::MiningMode(mode) => {
                    settings.apply_tuning_config = true;
                    settings.tuning_target = TuningTargetSelection::MiningMode;
                    settings.mining_mode = map_mining_mode_selection(mode);
                }
                TuningTarget::Power(power) if miner.supports_set_power_limit() => {
                    settings.apply_power_limit = true;
                    settings.power_limit_watts = power.as_watts();
                    settings.apply_tuning_config = true;
                    settings.tuning_target = TuningTargetSelection::Power;
                    settings.tuning_power_watts = power.as_watts();
                }
                TuningTarget::HashRate(hashrate) => {
                    settings.apply_tuning_config = true;
                    settings.tuning_target = TuningTargetSelection::Hashrate;
                    settings.tuning_hashrate_ths =
                        hashrate.clone().as_unit(HashRateUnit::TeraHash).value;
                    settings.tuning_hashrate_algo = hashrate.algo;
                }
                _ => {}
            }
        }
    }

    if miner.supports_scaling_config() {
        if let Ok(scaling) = miner.get_scaling_config().await {
            settings.apply_scaling_config = true;
            settings.scaling_step = scaling.step;
            settings.scaling_minimum = scaling.minimum;
            settings.scaling_shutdown = scaling.shutdown.unwrap_or(false);
            if let Some(duration) = scaling.shutdown_duration {
                settings.scaling_shutdown_duration = duration;
            }
        }
    }

    if miner.supports_pools_config() {
        if let Ok(pool_groups) = miner.get_pools_config().await {
            if let Some(group) = pool_groups.first() {
                if !group.pools.is_empty() {
                    settings.apply_pool_config = true;
                    settings.pool_group_name = if group.name.trim().is_empty() {
                        "Primary".to_string()
                    } else {
                        group.name.clone()
                    };
                    settings.pool_group_quota = group.quota;
                    settings.pool_inputs = group
                        .pools
                        .iter()
                        .map(|pool| PoolInput {
                            url: pool.url.to_string(),
                            username: pool.username.clone(),
                            password: pool.password.clone(),
                        })
                        .collect();
                }
            }
        }
    }

    Ok(settings)
}

pub async fn apply_options_to_miner(
    ip: String,
    settings: MinerOptionSettings,
) -> Result<Vec<String>, String> {
    let factory = MinerFactory::new();
    let parsed_ip = ip
        .parse()
        .map_err(|e| format!("Invalid IP address {ip}: {e}"))?;

    let Some(miner) = factory
        .get_miner(parsed_ip)
        .await
        .map_err(|e| format!("Failed to connect to {ip}: {e}"))?
    else {
        return Err(format!("No supported miner found at {ip}"));
    };

    let mut applied = Vec::new();

    if settings.apply_power_limit && miner.supports_set_power_limit() {
        miner
            .set_power_limit(Power::from_watts(settings.power_limit_watts))
            .await
            .map_err(|e| format!("Failed power limit on {ip}: {e}"))?;
        applied.push(format!("power={}W", settings.power_limit_watts.round()));
    }

    if settings.apply_fan_config && miner.supports_fan_config() {
        let fan_config = match settings.fan_mode {
            FanModeSelection::Auto => FanConfig::auto(
                settings.fan_target_temp_c,
                Some(settings.fan_idle_speed_percent),
            ),
            FanModeSelection::Manual => FanConfig::manual(settings.fan_speed_percent),
        };

        miner
            .set_fan_config(fan_config)
            .await
            .map_err(|e| format!("Failed fan config on {ip}: {e}"))?;

        match settings.fan_mode {
            FanModeSelection::Auto => applied.push(format!(
                "fan=auto(target={}C,idle={}%)",
                settings.fan_target_temp_c, settings.fan_idle_speed_percent
            )),
            FanModeSelection::Manual => {
                applied.push(format!("fan=manual({}%)", settings.fan_speed_percent))
            }
        }
    }

    if settings.apply_tuning_config && miner.supports_tuning_config() {
        if let Some(message) = settings.tuning_validation_message() {
            return Err(format!("{} ({})", message, ip));
        }

        let mut tuning = TuningConfig::new(tuning_target_from_settings(&settings));

        if !settings.tuning_algorithm.trim().is_empty() {
            tuning = tuning.with_algorithm(settings.tuning_algorithm.trim());
        }

        miner
            .set_tuning_config(tuning)
            .await
            .map_err(|e| format!("Failed tuning config on {ip}: {e}"))?;

        match settings.tuning_target {
            TuningTargetSelection::MiningMode => {
                applied.push(format!("tuning_mode={:?}", settings.mining_mode));
            }
            TuningTargetSelection::Power => {
                applied.push(format!(
                    "tuning_power={}W",
                    settings.tuning_power_watts.round()
                ));
            }
            TuningTargetSelection::Hashrate => {
                applied.push(format!(
                    "tuning_hashrate={:.2}TH/s ({})",
                    settings.tuning_hashrate_ths, settings.tuning_hashrate_algo
                ));
            }
        }

        if !settings.tuning_algorithm.trim().is_empty() {
            applied.push(format!(
                "tuning_algorithm={}",
                settings.tuning_algorithm.trim()
            ));
        }
    }

    if settings.apply_scaling_config && miner.supports_scaling_config() {
        let mut scaling = ScalingConfig::new(settings.scaling_step, settings.scaling_minimum)
            .with_shutdown(settings.scaling_shutdown);

        if settings.scaling_shutdown {
            scaling = scaling.with_shutdown_duration(settings.scaling_shutdown_duration);
        }

        miner
            .set_scaling_config(scaling)
            .await
            .map_err(|e| format!("Failed scaling config on {ip}: {e}"))?;

        applied.push(format!(
            "scaling(step={},min={},shutdown={})",
            settings.scaling_step, settings.scaling_minimum, settings.scaling_shutdown
        ));
    }

    if settings.apply_pool_config && miner.supports_pools_config() {
        if let Some(message) = settings.pool_validation_message() {
            return Err(format!("{} ({})", message, ip));
        }

        let group = PoolGroupConfig {
            name: if settings.pool_group_name.trim().is_empty() {
                "Primary".to_string()
            } else {
                settings.pool_group_name.trim().to_string()
            },
            quota: settings.pool_group_quota,
            pools: settings
                .pool_inputs
                .iter()
                .map(|pool| PoolConfig {
                    url: PoolURL::from(pool.url.trim().to_string()),
                    username: pool.username.trim().to_string(),
                    password: pool.password.clone(),
                })
                .collect(),
        };

        miner
            .set_pools_config(vec![group])
            .await
            .map_err(|e| format!("Failed pool config on {ip}: {e}"))?;

        applied.push(format!(
            "pools={} group={}",
            settings.pool_inputs.len(),
            settings.pool_group_name
        ));
    }

    if applied.is_empty() {
        return Err(format!(
            "No compatible options enabled/supported for {}",
            ip
        ));
    }

    Ok(applied)
}

pub async fn apply_options_to_many(ips: Vec<String>, settings: MinerOptionSettings) {
    if ips.is_empty() {
        return;
    }

    let concurrency = ips.len().clamp(1, 24);

    stream::iter(ips)
        .map(|ip| {
            let settings = settings.clone();
            async move {
                match apply_options_to_miner(ip.clone(), settings).await {
                    Ok(applied) => println!("✓ Applied options to {} ({})", ip, applied.join(", ")),
                    Err(err) => eprintln!("✗ {}", err),
                }
            }
        })
        .buffer_unordered(concurrency)
        .collect::<Vec<()>>()
        .await;
}
