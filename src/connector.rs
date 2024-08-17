use std::thread;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::time::Duration;
use rumqttc::{MqttOptions, Event, Incoming, Client, QoS};
use json::*;
use crate::Config;


pub struct MQTTConnector {
    pub client: rumqttc::Client,
    handle: Option<thread::JoinHandle<()>>,
    stop_flag: Arc<AtomicBool>
}

impl MQTTConnector {
    pub fn new<F>(name: &str, config: Arc<Config>, callback: F) -> Result<Self>
    where
        F: Fn(String, json::JsonValue) + Send + 'static
    {
        let mut mqttoptions = MqttOptions::new(name, &config.mqtt_host, config.mqtt_port);
        mqttoptions.set_keep_alive(Duration::from_secs(config.mqtt_keep_alive.into()));
        println!("Connecting to MQTT broker...");

        let (client, mut connection) = Client::new(mqttoptions, 10);

        let stop_flag_orig = Arc::new(AtomicBool::new(false));
        let stop_flag = stop_flag_orig.clone();

        // Connection handler thread
        let handle = thread::spawn(move || {
            // TODO: this won't work if connecions hanged up
            // replace iter() on some recv (recv_timeout) and handle notifications one by one with checking stop_flag
            while !stop_flag.load(Ordering::Acquire) {
                // The `EventLoop`/`Connection` must be regularly polled(`.next()` in case of `Connection`) in order
                //  to send, receive and process packets from the broker, i.e. move ahead.
                for (_, notification) in connection.iter().enumerate() {
                    if stop_flag.load(Ordering::Acquire) {
                        println!("Connector thread: received stop signal, closing...");
                        break;
                    }

                    // println!("MQTT notification = {:?}", notification);

                    if let Ok(Event::Incoming(Incoming::Publish(packet))) = notification {
                        let payload = String::from_utf8_lossy(&packet.payload);
                        let payload = json::parse(payload.as_ref()).unwrap();
                        println!("received packet with topic = {:?}", packet.topic);
                        callback(packet.topic, payload);
                    }
                }
            }
        });

        let connector = Self {
            client,
            handle: Some(handle),
            stop_flag: stop_flag_orig
        };
        Ok(connector)
    }
    pub fn subscribe_client(&mut self, topic: &String) {
        self.client.subscribe(topic, QoS::AtLeastOnce).unwrap();
    }
}

impl Drop for MQTTConnector {
    fn drop(&mut self) {
        self.stop_flag.store(true, Ordering::Release);
        // wating for thread finished
        if let Some(handle) = self.handle.take() {
            if let Err(e) = handle.join() {
                println!("Connector: failed to close thread: {:?}", e);
            }
        }
    }
}
