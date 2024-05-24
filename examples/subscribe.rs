use sonos::SonosDevice;

#[tokio::main]
async fn main() -> sonos::Result<()> {
    env_logger::init();

    let device = SonosDevice::for_room("Study").await?;

    let mut events = device.subscribe_av_transport().await?;

    while let Some(event) = events.recv().await {
        println!("{event:?}");
    }

    Ok(())
}
