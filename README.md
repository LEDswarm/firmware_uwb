# firmware-uwb-xtensa-idf

This is a special firmware for the Makerfabs ESP32 UWB board with an integrated radio.

# Getting Started

Make sure you have cargo-espflash installed, then simply run the following command to build and flash to a connected ESP32 device:

```
    cargo espflash flash
```

## Prerequisites

Install `cargo-espflash`:

```
    cargo install cargo-espflash
```

# Controller States

## 1. Discovery

The controller is currently trying to find nearby devices to build a mesh with. If no devices are found within 30 seconds, the controller will switch itself into Master mode, start a new mesh network using the ultra-wideband radio and wait for other controllers to join the session over UWB.

### Status Code

Blinks cyan for a second followed by a pause of equal length.

![timeline of 1-second cyan blinking followed by pause of equal length](https://ghoust.s3.fr-par.scw.cloud/blink_codes/discovery_led_pattern.png)

## 2. Connecting

The controller has found a nearby mesh and is currently trying to join the existing session.

### Status Code

Blinks cyan for a half a second followed by a pause of equal length, the same as the previous code but twice as fast.

![timeline of half-second cyan blinking followed by pause of equal length](https://ghoust.s3.fr-par.scw.cloud/blink_codes/connecting_led_pattern.png)

## 3. Client

The controller is currently connected to an UWB mesh in client (non-master) mode.

### Status Code

Three quick green blinks to indicate a successful connection.

![timeline of half-second cyan blinking followed by pause of equal length](https://ghoust.s3.fr-par.scw.cloud/blink_codes/client_led_pattern.png)

## 4. Server Meditation

The controller is managing a mesh network along with a Wi-Fi hotspot and waits for other controllers to join the session so that it can start a game.

### Status Code

The server meditates with a circulating rainbow until a second UWB device joins the session.

![timeline of half-second cyan blinking followed by pause of equal length](https://ghoust.s3.fr-par.scw.cloud/blink_codes/server_meditation_led_pattern.png)