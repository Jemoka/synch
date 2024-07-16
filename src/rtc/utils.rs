use anyhow::Result;

use std::default::Default;

use webrtc::api::{APIBuilder, API};
use webrtc::api::media_engine::MediaEngine;
use webrtc::interceptor::registry::Registry;
use webrtc::api::setting_engine::SettingEngine;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::api::interceptor_registry::register_default_interceptors;

pub fn get_api() -> Result<API> {
    // Create a MediaEngine object to configure the supported codec
    let mut m = MediaEngine::default();

    // Register default codecs
    m.register_default_codecs()?;

    // Create a SettingEngine and enable Detach
    let mut s = SettingEngine::default();
    s.detach_data_channels();

    // Create a InterceptorRegistry. This is the user configurable RTP/RTCP Pipeline.
    let mut registry = Registry::new();

    // Use the default set of Interceptors
    registry = register_default_interceptors(registry, &mut m)?;

    // Create the API object with the MediaEngine
    let api = APIBuilder::new()
        .with_media_engine(m)
        .with_interceptor_registry(registry)
        .with_setting_engine(s)
        .build();

    Ok(api)
}

pub fn get_config_from_stun_servers(stun_servers: &[&str]) -> RTCConfiguration {
    let ice_servers = stun_servers
        .into_iter()
        .map(|x| RTCIceServer {
            urls: vec![x.to_string()],
            ..Default::default()
        }).collect();

    RTCConfiguration {
        ice_servers: ice_servers,
        ..Default::default()
    }
}


