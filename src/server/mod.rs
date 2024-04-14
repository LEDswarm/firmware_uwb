use std::sync::mpsc;
use embedded_svc::{http::Method, ws::FrameType};
use esp_idf_hal::sys::{ESP_ERR_INVALID_SIZE, EspError};
use esp_idf_svc::http::server::EspHttpServer;
use ledswarm_protocol::{ClientMessage, InternalMessage};

use crate::RootDocument;

pub mod handlers;

pub const STACK_SIZE: usize = 10240;
// Max payload length
const MAX_LEN: usize = 256;


/// Initialize HTTP server and WebSocket endpoints.
pub fn create_endpoints(msg_tx: mpsc::SyncSender<InternalMessage>) -> anyhow::Result<()> {
    let server_configuration = esp_idf_svc::http::server::Configuration {
        stack_size: 10240,
        ..Default::default()
    };
    let mut server = EspHttpServer::new(&server_configuration).unwrap();

    server.fn_handler("/", Method::Get, |req| {
        let root_doc = RootDocument {
            version: "0.1.0".to_string(),
        };
        let mut response = req.into_ok_response()?;
        response.write(serde_json::to_string(&root_doc)?.as_bytes()).unwrap();
        Ok(())
    }).unwrap();

    server
        .ws_handler("/ws", move |ws| {
            if ws.is_new() {
                // sessions.insert(ws.session(), GuessingGame::new((rand() % 100) + 1));
                println!("New WebSocket session");

                // We need to send at least one message to keep the connection alive.
                let msg = ClientMessage::SetBrightness(0.5);
                let json_string = serde_json::to_string(&msg).unwrap();

                ws.send(
                    FrameType::Text(false),
                    json_string.as_bytes(),
                )?;

                return Ok(());
            } else if ws.is_closed() {
                // sessions.remove(&ws.session());
                println!("Closed WebSocket session");
                return Ok(());
            }
            // let session = sessions.get_mut(&ws.session()).unwrap();

            // NOTE: Due to the way the underlying C implementation works, ws.recv()
            // may only be called with an empty buffer exactly once to receive the
            // incoming buffer size, then must be called exactly once to receive the
            // actual payload.

            let (_frame_type, len) = match ws.recv(&mut []) {
                Ok(frame) => frame,
                Err(e) => return Err(e),
            };

            if len > MAX_LEN {
                ws.send(FrameType::Text(false), "Request too big".as_bytes())?;
                ws.send(FrameType::Close, &[])?;
                return Err(EspError::from_infallible::<ESP_ERR_INVALID_SIZE>());
            }

            let mut buf = [0; MAX_LEN]; // Small digit buffer can go on the stack
            ws.recv(buf.as_mut())?;
            let Ok(user_string) = std::str::from_utf8(&buf[..len]) else {
                ws.send(FrameType::Text(false), "[UTF-8 Error]".as_bytes())?;
                return Ok(());
            };

            // Remove null terminator
            match serde_json::from_str::<ClientMessage>(&user_string[0 .. user_string.len() - 1]) {
                Ok(msg) => msg_tx.send(InternalMessage::ClientMessage(msg)).unwrap(),
                Err(e)  => println!("Failed to parse JSON:\n\n{}\n\n{}", e, user_string),
            }
            
            ws.send(FrameType::Text(false), user_string.as_bytes())?;

            Ok::<(), EspError>(())
        })
        .unwrap();
    
    core::mem::forget(server);

    Ok(())
}