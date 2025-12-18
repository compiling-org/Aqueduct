use crate::error::Result;
use mdns_sd::{ServiceDaemon, ServiceInfo, ServiceEvent};
use log::info;

const SERVICE_TYPE: &str = "_omt._tcp.local.";

pub struct Discovery {
    mdns: ServiceDaemon,
}

impl Discovery {
    pub fn new() -> Result<Self> {
        let mdns = ServiceDaemon::new().map_err(|e| crate::error::AqueductError::Discovery(e.to_string()))?;
        Ok(Self { mdns })
    }

    pub fn register_source(&self, device_name: &str, source_name: &str, port: u16) -> Result<()> {
        let instance_name = format!("{} ({})", device_name, source_name);
        let hostname = format!("{}.local.", device_name);
        let properties = [("version", "1.0")]; // Example property

        let service_info = ServiceInfo::new(
            SERVICE_TYPE,
            &instance_name,
            &hostname,
            "", // ip will be auto-detected
            port,
            &properties[..],
        ).map_err(|e| crate::error::AqueductError::Discovery(e.to_string()))?;

        self.mdns.register(service_info)
            .map_err(|e| crate::error::AqueductError::Discovery(e.to_string()))?;
        
        info!("Registered source: {}", instance_name);
        Ok(())
    }

    pub fn browse_sources<F>(&self, callback: F) -> Result<()> 
    where F: Fn(ServiceEvent) + Send + 'static 
    {
        let receiver = self.mdns.browse(SERVICE_TYPE)
            .map_err(|e| crate::error::AqueductError::Discovery(e.to_string()))?;

        std::thread::spawn(move || {
            while let Ok(event) = receiver.recv() {
                callback(event);
            }
        });

        Ok(())
    }
}
