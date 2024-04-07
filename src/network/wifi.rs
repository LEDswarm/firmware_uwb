use esp_idf_svc::{
    hal::{
        prelude::*,
        i2c::*,
    },
    wifi::{AsyncWifi, EspWifi, WifiDriver, AuthMethod, ClientConfiguration, AccessPointConfiguration, Configuration},
    eventloop::EspSystemEventLoop,
    nvs::EspDefaultNvsPartition,
};

use esp_idf_svc::netif::NetifConfiguration;
use esp_idf_svc::netif::NetifStack;
use esp_idf_svc::netif::EspNetif;

use esp_idf_svc::ipv4::{
    Configuration as IpConfiguration, Mask, Subnet,
};
use esp_idf_svc::sys::EspError;

use embedded_svc::ipv4::RouterConfiguration;
use embedded_svc::ipv4::Ipv4Addr;
use std::str::FromStr;

use esp_idf_svc::http::server::EspHttpServer;

use std::sync::mpsc;

use crate::controller::ControllerMode;

// Max payload length
const MAX_LEN: usize = 8;

// Need lots of stack to parse JSON
pub const STACK_SIZE: usize = 10240;

// Wi-Fi channel, between 1 and 11
const CHANNEL: u8 = 11;

// Expects IPv4 address
const DEVICE_IP: &str = "192.168.1.1";
// Expects IPv4 address
const GATEWAY_IP: &str = "192.168.1.1";
// Expects a number between 0 and 32, defaults to 24
const GATEWAY_NETMASK: Option<&str> = option_env!("GATEWAY_NETMASK");

pub struct WifiController<'a> {
    wifi: AsyncWifi<EspWifi<'a>>,
    tx: mpsc::Sender<ControllerMode>,
}

impl<'a> WifiController<'a> {
    pub fn new(
        wifi: AsyncWifi<EspWifi<'a>>,
        tx: mpsc::Sender<ControllerMode>,
    ) -> Self {
        Self {
            wifi,
            tx,
        }
    }

    pub async fn join_network(&mut self, ssid: &str) -> Result<(), EspError> {
        println!("--> Setting client config");
        self.wifi.set_configuration(&Configuration::Client(ClientConfiguration {
            ssid: ssid.into(),
            auth_method: AuthMethod::None,
            ..Default::default()
        }))?;

        println!("Starting Wi-Fi in client mode");
        self.wifi.start().await?;

        self.tx.send(ControllerMode::Client {
            id: 1,
            game: None,
        }).unwrap();

        Ok(())
    }

    pub async fn create_network(&mut self) -> Result<(), EspError> {
        println!("--> Setting access point configuration");
        self.wifi.set_configuration(&Configuration::AccessPoint(AccessPointConfiguration {
            ssid: "LEDswarm".into(),
            password: "LEDswarm".into(),
            ..Default::default()
        })).unwrap();

        println!("--> Starting access point");

        // Start Wifi
        self.wifi.start().await?;

        let ip_info = self.wifi.wifi().sta_netif().get_ip_info()?;

        println!("--> Wifi DHCP info: {:?}", ip_info);

        // Keep wifi running beyond when this function returns (forever)
        // Do not call this if you ever want to stop or access it later.
        // Otherwise it should be returned from this function and kept somewhere
        // so it does not go out of scope.
        // https://doc.rust-lang.org/stable/core/mem/fn.forget.html
        // core::mem::forget(wifi);
        Ok(())
    }

    /// Initiate the controller Wi-Fi, either connecting to an existing network or creating a new one.
    pub async fn join_or_create_network(&mut self) -> Result<(), EspError> {
        self.wifi.set_configuration(&Configuration::Client(ClientConfiguration {
            ssid: "XYZ".into(),
            password: "XYZ".into(),
            auth_method: AuthMethod::WPA2Personal,
            ..Default::default()
        }))?;
        self.wifi.start().await?;

        let networks = self.wifi.scan().await?;
        println!("--> Looking for LEDswarm network");

        if networks.iter().any(|network| network.ssid == "LEDswarm") {
            println!("--> Found LEDswarm network");
            match self.tx.send(ControllerMode::Connecting) {
                Ok(_) => println!("--> Set ControllerMode::Connecting"),
                Err(e) => println!("--> Failed to send ControllerMode::Connecting: {}", e),
            }
            self.join_network("LEDswarm").await?;
        } else {
            println!("--> Creating new LEDswarm network");
            match self.tx.send(ControllerMode::ServerMeditation) {
                Ok(_) => println!("--> Set ControllerMode::ServerMeditation"),
                Err(e) => println!("--> Failed to send ControllerMode::ServerMeditation: {}", e),
            }
            self.create_network().await?;
        }

        Ok(())
    }
}

pub fn create_server<'a>(peripherals: &'a mut Peripherals, sys_loop: EspSystemEventLoop, nvs: EspDefaultNvsPartition) -> EspHttpServer<'static> {
    let wifi_driver = WifiDriver::new(&mut peripherals.modem, sys_loop.clone(), Some(nvs)).unwrap();

    let netmask = GATEWAY_NETMASK.unwrap_or("24");
    let netmask = u8::from_str(netmask).unwrap();
    let gateway_addr = Ipv4Addr::from_str(GATEWAY_IP).unwrap();

    let mut wifi = EspWifi::wrap_all(
        wifi_driver,
        EspNetif::new_with_conf(&NetifConfiguration {
            ip_configuration: IpConfiguration::Router(
                RouterConfiguration {
                    subnet: Subnet {
                        gateway: gateway_addr,
                        mask: Mask(netmask),
                    },
                    dns: None,
                    secondary_dns: None,
                    dhcp_enabled: false,
                }),
            ..NetifConfiguration::wifi_default_client()
        }).unwrap(),
        EspNetif::new(NetifStack::Ap).unwrap(),
    ).unwrap();

    wifi.set_configuration(&Configuration::AccessPoint(AccessPointConfiguration {
        ssid: "LEDswarm".into(),
        password: "LEDswarm".into(),
        ..Default::default()
    })).unwrap();

    // Start Wifi
    wifi.start().unwrap();

    let ip_info = wifi.sta_netif().get_ip_info().unwrap();

    println!("--> Wifi DHCP info: {:?}", ip_info);

    let server_configuration = esp_idf_svc::http::server::Configuration {
        stack_size: STACK_SIZE,
        ..Default::default()
    };

    // Keep wifi running beyond when this function returns (forever)
    // Do not call this if you ever want to stop or access it later.
    // Otherwise it should be returned from this function and kept somewhere
    // so it does not go out of scope.
    // https://doc.rust-lang.org/stable/core/mem/fn.forget.html
    core::mem::forget(wifi);

    EspHttpServer::new(&server_configuration).unwrap()
}