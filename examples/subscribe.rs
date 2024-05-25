use sonos::SonosDevice;

#[tokio::main]
async fn main() -> sonos::Result<()> {
    env_logger::init();

    let device = SonosDevice::for_room("Study").await?;

    let mut events = device.subscribe_av_transport().await?;
    //let mut events = device.subscribe_queue().await?;
    //let mut events = device.subscribe_rendering_control().await?;
    //let mut events = device.subscribe_virtual_line_in().await?;

    while let Some(event) = events.recv().await {
        println!("{event:#?}");
    }

    Ok(())
}
