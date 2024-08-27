fn main() {
    let mut command = std::process::Command::new("cargo");

    command.arg("watch").arg("-x").arg("run");

    command.spawn().expect("Spawn to work");

    // A WebSocket echo server
    let server = std::net::TcpListener::bind("127.0.0.1:9001").unwrap();
    server.accept()
    for stream in server.incoming() {
        std::thread::spawn(move || {
            let mut websocket = tungstenite::accept(stream.unwrap()).unwrap();
            websocket.send(tungstenite::Message::Ping(vec![])).unwrap();

            loop {
                let msg = websocket.read().unwrap();

                dbg!(msg);
            }
        });
    }

    // let mut writer = std::io::BufWriter::new(stdin);
    // let mut reader = std::io::BufReader::new(stdout);

    // let w_buf = bincode::serialize(&SurrogateMessage::Ping).unwrap();

    // writer.write_all(&w_buf).unwrap();
    // writer.write_all(&mut [b'\n']).unwrap();
    // let mut buf = Vec::new();
    // reader.read_until(b'\n', &mut buf).unwrap();

    // let msg = bincode::deserialize::<SurrogateMessage>(&buf).unwrap();

    // dbg!(msg);
}
