use buttplug_client::{
    ButtplugClient, ButtplugClientDevice,
    device::{ClientDeviceCommandValue, ClientDeviceOutputCommand},
};
use buttplug_core::message::OutputType;

use super::effect::Output;
use super::{ToyDevice, ToyError};

pub struct ButtplugToyBackend {
    client: ButtplugClient,
}

impl ButtplugToyBackend {
    pub fn new(client: ButtplugClient) -> Self {
        Self { client }
    }

    pub fn devices(&self) -> Vec<ToyDevice> {
        self.vibrating_devices()
            .into_iter()
            .map(|device| {
                ToyDevice::vibrating(
                    device.index().to_string(),
                    device.name().to_string(),
                    device.output_available(OutputType::Vibrate),
                )
            })
            .collect()
    }

    pub async fn rescan(&self) -> Result<(), ToyError> {
        self.client
            .start_scanning()
            .await
            .map_err(|error| ToyError::Buttplug(error.to_string()))
    }

    pub fn targets(&self, target_names: &[String]) -> Vec<ButtplugClientDevice> {
        self.vibrating_devices()
            .into_iter()
            .filter(|device| target_names.iter().any(|name| name == device.name()))
            .collect()
    }

    pub async fn disconnect(self) {
        let _ = self.client.disconnect().await;
    }

    fn vibrating_devices(&self) -> Vec<ButtplugClientDevice> {
        self.client
            .devices()
            .into_values()
            .filter(|device| device.output_available(OutputType::Vibrate))
            .collect()
    }
}

impl Output for ButtplugClientDevice {
    fn set_level(&self, level: f64) -> impl Future<Output = ()> + Send {
        let device = self.clone();
        async move {
            let _ = device
                .run_output(&ClientDeviceOutputCommand::Vibrate(
                    ClientDeviceCommandValue::Percent(level),
                ))
                .await;
        }
    }
}
