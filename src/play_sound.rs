pub mod sound {
    use flume::{Receiver, Sender};
    use once_cell::sync::Lazy;
    use rodio::source::Buffered;
    use rodio::{source::Source, Decoder, OutputStream};
    use std::collections::HashMap;
    use std::fs::File;
    use std::io::BufReader;
    use std::path::PathBuf;
    use std::sync::Mutex;
    use std::thread;
    use std::time::Duration;

    type SoundSource = Buffered<Decoder<BufReader<File>>>;

    static GLOBAL_DATA: Lazy<Mutex<HashMap<String, SoundSource>>> = Lazy::new(|| {
        let m = HashMap::new();
        Mutex::new(m)
    });

    static WORKER_CHANNEL: Lazy<Mutex<Sender<PathBuf>>> = Lazy::new(|| Mutex::new(new_worker()));

    fn new_worker() -> Sender<PathBuf> {
        let (tx, rx) = flume::unbounded();
        thread::spawn(move || {
            worker(rx);
        });
        tx
    }

    pub fn play_sound(name: PathBuf) {
        let mut tx = WORKER_CHANNEL.lock().unwrap();
        if tx.is_disconnected() {
            *tx = new_worker()
        }
        tx.send(name).expect("Couldn't send name to threadpool");
    }

    pub fn worker(rx_channel: Receiver<PathBuf>) {
        let (_stream, stream_handle) = OutputStream::try_default().unwrap();
        loop {
            if let Ok(mut name) = rx_channel.recv_timeout(Duration::from_secs(20)) {
                let source = {
                    let mut sound_map = GLOBAL_DATA.lock().unwrap();
                    sound_map
                        .entry(name.to_str().unwrap().to_string())
                        .or_insert_with(|| {
                            let file = BufReader::new(if let Ok(file) = File::open(&name) {
                                file
                            } else {
                                name.set_file_name(String::from("A.mp3"));
                                File::open(&name).unwrap()
                            });
                            Decoder::new(file).unwrap().buffered()
                        })
                        .clone()
                };
                let sink = rodio::Sink::try_new(&stream_handle).unwrap();
                sink.append(source);
                sink.detach();
            } else {
                thread::sleep(Duration::from_millis(100));
                // Timeout, time to put this thread to sleep to save CPU cycles (open audio OutputStreams use
                // around half a CPU millicore, and then CoreAudio uses another 7-10%)
                break;
            }
        }
    }
}
