use embedded_svc::wifi::{ClientConfiguration, Wifi, Configuration};
use esp_idf_hal::modem::Modem;
use esp_idf_svc::{
    wifi::EspWifi,
    nvs::EspNvsPartition,
    nvs::NvsDefault,
    eventloop::EspSystemEventLoop,
};

pub struct Wireless<'a> {
    wifi_driver: EspWifi<'a>,
}

impl<'a> Wireless<'a> {
    pub fn new(
        modem:    Modem,
        sys_loop: EspSystemEventLoop,
        nvs:      EspNvsPartition<NvsDefault>,
    ) -> Self {
        let mut wifi_driver = EspWifi::new(
            modem,
            sys_loop,
            Some(nvs)
        ).unwrap();
    
        wifi_driver.set_configuration(&Configuration::Client(ClientConfiguration{
            ssid:     dotenv!("WIFI_SSID").into(),
            password: dotenv!("WIFI_PASSWORD").into(),
            ..Default::default()
        })).unwrap();
    
        wifi_driver.start().unwrap();
        wifi_driver.connect().unwrap();
        while !wifi_driver.is_connected().unwrap(){
            let config = wifi_driver.get_configuration().unwrap();
            println!("Waiting for station {:?}", config);
        }
        println!("Should be connected now");
        println!("IP info: {:?}", wifi_driver.sta_netif().get_ip_info().unwrap());

        Self {
            wifi_driver,
        }
    }

    pub fn print_ip_info(&self) {
        println!("IP info: {:?}", self.wifi_driver.sta_netif().get_ip_info().unwrap());
    }
}