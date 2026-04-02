use std::collections::HashMap;

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo, Receiver};

use crate::types::{CoreError, PeerId};

const SERVICE_TYPE: &str = "_ghostmesh._tcp.local.";
const TXT_KEY_PEER_ID: &str = "peer_id";

#[derive(Debug, Clone)]
pub struct DiscoveredPeer {
    pub peer_id: PeerId,
    pub addresses: Vec<String>,
}

pub struct MdnsDiscovery {
    daemon: ServiceDaemon,
    receiver: Receiver<ServiceEvent>,
    local_peer_id: PeerId,
    discovered: HashMap<PeerId, Vec<String>>,
}

impl MdnsDiscovery {
    pub fn new(peer_id: &PeerId, port: u16) -> Result<Self, CoreError> {
        let daemon = ServiceDaemon::new().map_err(mdns_error)?;

        let peer_id_hex = hex::encode(peer_id);
        let instance_name = format!("ghostmesh-{}", &peer_id_hex[..8]);

        let host_name = format!("{instance_name}.local.");
        let properties = [(TXT_KEY_PEER_ID, peer_id_hex.as_str())];

        let service_info = ServiceInfo::new(
            SERVICE_TYPE,
            &instance_name,
            &host_name,
            "",
            port,
            &properties[..],
        )
        .map_err(|e| CoreError::Net(format!("mDNS service info error: {e}")))?;

        daemon.register(service_info).map_err(mdns_error)?;

        let receiver = daemon.browse(SERVICE_TYPE).map_err(mdns_error)?;

        Ok(Self {
            daemon,
            receiver,
            local_peer_id: *peer_id,
            discovered: HashMap::new(),
        })
    }

    pub fn poll_discoveries(&mut self) {
        while let Ok(event) = self.receiver.try_recv() {
            match event {
                ServiceEvent::ServiceResolved(info) => {
                    if let Some(peer_id) = parse_peer_id_from_service(&info) {
                        if peer_id == self.local_peer_id {
                            continue;
                        }
                        let addresses: Vec<String> = info
                            .get_addresses()
                            .iter()
                            .map(|addr| format!("{addr}:{}", info.get_port()))
                            .collect();
                        if !addresses.is_empty() {
                            self.discovered.insert(peer_id, addresses);
                        }
                    }
                }
                ServiceEvent::ServiceRemoved(_, fullname) => {
                    self.discovered
                        .retain(|_, _| !fullname.contains(SERVICE_TYPE));
                }
                _ => {}
            }
        }
    }

    pub fn discovered_peers(&mut self) -> Vec<DiscoveredPeer> {
        self.poll_discoveries();
        self.discovered
            .iter()
            .map(|(peer_id, addresses)| DiscoveredPeer {
                peer_id: *peer_id,
                addresses: addresses.clone(),
            })
            .collect()
    }

    pub fn shutdown(self) -> Result<(), CoreError> {
        let _status_receiver = self.daemon.shutdown().map_err(mdns_error)?;
        Ok(())
    }
}

fn parse_peer_id_from_service(info: &ServiceInfo) -> Option<PeerId> {
    let peer_id_hex = info.get_property_val_str(TXT_KEY_PEER_ID)?;
    let bytes = hex::decode(peer_id_hex).ok()?;
    if bytes.len() != 16 {
        return None;
    }
    let mut peer_id = [0u8; 16];
    peer_id.copy_from_slice(&bytes);
    Some(peer_id)
}

fn mdns_error(e: mdns_sd::Error) -> CoreError {
    CoreError::Net(format!("mDNS error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_peer_id_from_txt_record() {
        let peer_id: PeerId = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
                                0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10];
        let peer_id_hex = hex::encode(peer_id);
        let properties = [(TXT_KEY_PEER_ID, peer_id_hex.as_str())];
        let info = ServiceInfo::new(
            SERVICE_TYPE,
            "test-instance",
            "test.local.",
            "",
            9473,
            &properties[..],
        )
        .unwrap();

        let parsed = parse_peer_id_from_service(&info).unwrap();
        assert_eq!(parsed, peer_id);
    }

    #[test]
    fn parse_peer_id_invalid_hex_returns_none() {
        let properties = [(TXT_KEY_PEER_ID, "not-valid-hex")];
        let info = ServiceInfo::new(
            SERVICE_TYPE,
            "test-instance",
            "test.local.",
            "",
            9473,
            &properties[..],
        )
        .unwrap();

        assert!(parse_peer_id_from_service(&info).is_none());
    }

    #[test]
    fn parse_peer_id_wrong_length_returns_none() {
        let properties = [(TXT_KEY_PEER_ID, "0102030405")];
        let info = ServiceInfo::new(
            SERVICE_TYPE,
            "test-instance",
            "test.local.",
            "",
            9473,
            &properties[..],
        )
        .unwrap();

        assert!(parse_peer_id_from_service(&info).is_none());
    }

    #[test]
    fn daemon_creates_and_shuts_down() {
        let peer_id: PeerId = [0xAA; 16];
        let discovery = MdnsDiscovery::new(&peer_id, 9473).unwrap();
        discovery.shutdown().unwrap();
    }

    #[test]
    #[ignore]
    fn discovers_local_peer_via_mdns() {
        let peer_a: PeerId = [0x01; 16];
        let peer_b: PeerId = [0x02; 16];

        let _discovery_a = MdnsDiscovery::new(&peer_a, 9473).unwrap();
        let mut discovery_b = MdnsDiscovery::new(&peer_b, 9474).unwrap();

        std::thread::sleep(std::time::Duration::from_secs(3));

        let peers = discovery_b.discovered_peers();
        let found_a = peers.iter().any(|p| p.peer_id == peer_a);
        assert!(found_a, "peer A should be discovered by peer B");
    }
}
