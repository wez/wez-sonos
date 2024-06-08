#[tokio::main]
async fn main() -> sonos::Result<()> {
    env_logger::init();

    let mut disco = sonos::discover(std::time::Duration::from_secs(15)).await?;
    while let Some(device) = disco.recv().await {
        match device.name().await {
            Ok(name) => {
                println!("{name}");
                if let Ok(state) = device.get_zone_group_state().await {
                    println!("{state:?}");
                }
            }
            Err(err) => {
                log::error!("{:?} {err:#}", device.device_spec().model_description);
            }
        }
    }

    Ok(())
}
