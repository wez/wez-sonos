use sonos::SonosDevice;

#[tokio::main]
async fn main() -> sonos::Result<()> {
    env_logger::init();

    let device = SonosDevice::for_room("Study").await?;

    //let mut events = device.subscribe(sonos::av_transport::SERVICE_TYPE).await?;
    let mut events = device
        .subscribe(sonos::zone_group_topology::SERVICE_TYPE)
        .await?;

    while let Some((k, v)) = events.recv().await {
        println!("{k}: {v}");
    }

    Ok(())
}
