mod backend;
mod central;
mod effect;
mod embedded;

pub use backend::ButtplugToyBackend;

use buttplug_client::ButtplugClientDevice;
use effect::EffectSlot;
pub use effect::PlayOutcome;

use crate::haptics::HapticCommand;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionMode {
    Disconnected,
    ExternalCentral,
    Embedded,
}

#[derive(Debug, Clone)]
pub struct ToyDevice {
    pub id: String,
    pub name: String,
    pub can_vibrate: bool,
}

impl ToyDevice {
    pub fn vibrating(id: impl Into<String>, name: impl Into<String>, can_vibrate: bool) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            can_vibrate,
        }
    }

    pub fn names(devices: &[Self]) -> Vec<String> {
        devices.iter().map(|device| device.name.clone()).collect()
    }
}

#[derive(Debug, Error)]
pub enum ToyError {
    #[error("{0}")]
    Buttplug(String),
    #[error("toy backend is not connected")]
    NotConnected,
}

pub struct ToyHub {
    mode: ConnectionMode,
    central_url: String,
    backend: Option<ButtplugToyBackend>,
    effects: EffectSlot<ButtplugClientDevice>,
}

impl ToyHub {
    pub fn new(central_url: String) -> Self {
        Self {
            mode: ConnectionMode::Disconnected,
            central_url,
            backend: None,
            effects: EffectSlot::default(),
        }
    }

    pub fn mode(&self) -> ConnectionMode {
        self.mode
    }

    pub fn set_central_url(&mut self, url: String) {
        self.central_url = url;
    }

    pub fn devices(&self) -> Vec<ToyDevice> {
        self.backend
            .as_ref()
            .map(ButtplugToyBackend::devices)
            .unwrap_or_default()
    }

    pub async fn connect_embedded(&mut self) -> Result<(), ToyError> {
        let backend = embedded::connect_embedded().await?;
        self.effects.stop();
        self.backend = Some(backend);
        self.mode = ConnectionMode::Embedded;
        Ok(())
    }

    pub async fn connect_central(&mut self) -> Result<(), ToyError> {
        let backend = central::connect_central(&self.central_url).await?;
        self.effects.stop();
        self.backend = Some(backend);
        self.mode = ConnectionMode::ExternalCentral;
        Ok(())
    }

    pub async fn rescan(&self) -> Result<(), ToyError> {
        match &self.backend {
            Some(backend) => backend.rescan().await,
            None => Err(ToyError::NotConnected),
        }
    }

    /// Starts `command` on the named toys and returns at once. It interrupts the
    /// effect still playing unless that one is stronger.
    pub fn play(&self, command: HapticCommand, target_names: &[String]) -> PlayOutcome {
        match &self.backend {
            Some(backend) => self.effects.play(command, backend.targets(target_names)),
            None => PlayOutcome::Started,
        }
    }

    pub async fn disconnect(&mut self) {
        self.effects.stop();
        if let Some(backend) = self.backend.take() {
            backend.disconnect().await;
        }
        self.mode = ConnectionMode::Disconnected;
    }
}
