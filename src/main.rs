use esp_idf_sys as _; // If using the `binstart` feature of `esp-idf-sys`, always keep this module imported
use std::{
    thread::sleep,
    time::Duration,
};
use esp_idf_hal::{
    peripherals::Peripherals,
};
use esp_idf_svc::{
    wifi::EspWifi,
    nvs::EspDefaultNvsPartition,
    eventloop::EspSystemEventLoop,
};
use embedded_svc::wifi::{ClientConfiguration, Wifi, Configuration};

fn main(){
    esp_idf_sys::link_patches();//Needed for esp32-rs
    println!("Entered Main function!");
    let peripherals = Peripherals::take().unwrap();
    let sys_loop = EspSystemEventLoop::take().unwrap();
    let nvs = EspDefaultNvsPartition::take().unwrap();

    let mut wifi_driver = EspWifi::new(
        peripherals.modem,
        sys_loop,
        Some(nvs)
    ).unwrap();

    wifi_driver.set_configuration(&Configuration::Client(ClientConfiguration{
        ssid: "SSID".into(),
        password: "PASSWORD".into(),
        ..Default::default()
    })).unwrap();

    wifi_driver.start().unwrap();
    wifi_driver.connect().unwrap();
    while !wifi_driver.is_connected().unwrap(){
        let config = wifi_driver.get_configuration().unwrap();
        println!("Waiting for station {:?}", config);
    }
    println!("Should be connected now");
    loop{
        println!("IP info: {:?}", wifi_driver.sta_netif().get_ip_info().unwrap());
        sleep(Duration::new(10,0));
    }

}
