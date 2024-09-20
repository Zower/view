use miette::IntoDiagnostic;
use std::net::TcpListener;
use tungstenite::Message;

use crate::{ClientMessage, ServerMessage};

pub fn run() -> miette::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:9001").into_diagnostic()?;

    // Let's spawn the handling of each connection in a separate task.
    while let Ok((stream, _)) = listener.accept() {
        dbg!("New connection");
        let mut ws = tungstenite::accept(stream).expect("Could not accept stream");

        loop {
            let message = ws.read().unwrap();

            let Message::Binary(data) = message else {
                panic!()
            };

            let message = bincode::deserialize::<ClientMessage>(&data).unwrap();

            dbg!(message);

            ws.send(Message::Binary(
                bincode::serialize(&ServerMessage::NoState).into_diagnostic()?,
            ))
            .into_diagnostic()?;
        }
    }

    Ok(())
}
