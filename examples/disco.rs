use futures_util::TryStreamExt;
use sonos::prelude::*;

#[tokio::main]
async fn main() -> sonos::Result<()> {
    env_logger::init();

    let devices = sonos::discover(std::time::Duration::from_secs(15)).await?;

    futures_util::pin_mut!(devices);

    while let Some(device) = devices.try_next().await? {
        use sonos::av_transport::GetMediaInfoRequest;

        match device
            .get_media_info(GetMediaInfoRequest { instance_id: 0 })
            .await
        {
            Ok(info) => {
                println!("{info:#?}");
            }
            Err(err) => {
                // log::error!("device: {device:#?}");
                log::error!("{:?} {err:#}", device.device_spec().model_description);
            }
        }
    }

    Ok(())
}
