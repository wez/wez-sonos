use futures_util::TryStreamExt;
use sonos::prelude::*;

#[tokio::main]
async fn main() -> sonos::Result<()> {
    env_logger::init();

    let devices = sonos::discover(std::time::Duration::from_secs(15)).await?;

    futures_util::pin_mut!(devices);

    while let Some(device) = devices.try_next().await? {
        match device.name().await {
            Ok(name) => {
                println!("{name}");
                if let Ok(state) = device.get_zone_group_state().await {
                    println!("{state:?}");
                }
            }
            Err(err) => {
                // log::error!("device: {device:#?}");
                log::error!("{:?} {err:#}", device.device_spec().model_description);
            }
        }
    }

    Ok(())
}
