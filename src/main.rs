extern crate rumqttc;
extern crate json;
extern crate envconfig;
extern crate chrono;

use std::thread;
use std::sync::Arc;
use std::time::Duration;
use rumqttc::{MqttOptions, Event, Incoming, Client, QoS};
use json::*;
use envconfig::Envconfig;
use std::collections::HashMap;
use slint::*;
use chrono::{DateTime, NaiveDateTime, Timelike, Utc};
use std::rc::Rc;

slint::include_modules!();

#[derive(Debug)]
#[derive(Envconfig)]
struct Config {
    #[envconfig(from = "MQTT_BROKER_HOST", default = "localhost")]
    pub mqtt_host: String,

    #[envconfig(from = "MQTT_BROKER_PORT", default = "1883")]
    pub mqtt_port: u16,

    #[envconfig(from = "MQTT_BROKER_KEEP_ALIVE", default = "5")]
    pub mqtt_keep_alive: u16,

    #[envconfig(from = "MQTT_BROKER_BASE_TOPIC", default = "homeassistant/sensor")]
    pub mqtt_base_topic: String,

    #[envconfig(from = "MQTT_CONTROLLER_NAME", default = "cubieboard")]
    pub mqtt_controller_name: String,

    #[envconfig(from = "DEFAULT_TIMEZONE_OFFSET_H", default = "3")]
    pub timezone_offset_h: i8,

    #[envconfig(from = "SRS_MIN_PROBABILITY_THRH_PRCNT", default = "50")]
    pub srs_min_prob_thrh: u8,

    #[envconfig(from = "RB_MIN_PROBABILITY_THRH_PRCNT", default = "50")]
    pub rb_min_prob_thrh: u8,
}

type GuiCallbackMap = HashMap<String, fn(Weak<AppWindow>, JsonValue, Arc<Config>) -> ()>;

fn main() {
    let config = Config::init_from_env().unwrap();
    println!("Using config:\n{:?}", config);

    let mut mqttoptions = MqttOptions::new("rumqtt-sync-gui", &config.mqtt_host, config.mqtt_port);
    mqttoptions.set_keep_alive(Duration::from_secs(config.mqtt_keep_alive.into()));
    println!("Connecting to MQTT broker...");

    let (mut client, mut connection) = Client::new(mqttoptions, 10);

    let mut topic_gui_map: GuiCallbackMap = HashMap::new();
    topic_gui_map.insert(make_full_topic("htu21d", &config), update_indoor_t_rh);
    topic_gui_map.insert(make_full_topic("mhz19", &config), update_indoor_co2);
    topic_gui_map.insert(make_full_topic("noaa_kp", &config), update_space_weather_kp);
    topic_gui_map.insert(make_full_topic("noaa_kp_inst", &config), update_space_weather_kp_inst);
    topic_gui_map.insert(make_full_topic("noaa_flux", &config), update_space_weather_flux);
    topic_gui_map.insert(make_full_topic("noaa_sw_forecast", &config), update_sw_forecast);

    let topic_gui_map_arc = Arc::new(topic_gui_map);
    let topic_gui_map_arc2 = topic_gui_map_arc.clone();

    let ui = AppWindow::new().unwrap();
    let ui_weak = ui.as_weak();

    let config_arc = Arc::new(config);

    // Connection handler thread
    let thread_join_handle = thread::spawn(move || {
        // The `EventLoop`/`Connection` must be regularly polled(`.next()` in case of `Connection`) in order
        //  to send, receive and process packets from the broker, i.e. move ahead.
        for (_, notification) in connection.iter().enumerate() {
            // println!("MQTT notification = {:?}", notification);

            if let Ok(Event::Incoming(Incoming::Publish(packet))) = notification {
                let topic = packet.topic.clone();
                println!("topic = {:?}", topic.as_str());
                if topic_gui_map_arc.contains_key(topic.as_str()) {
                    let json_payload = String::from_utf8_lossy(&packet.payload);
                    println!("json_payload = {:?}", json_payload);
                    let payload: json::JsonValue = json::parse(json_payload.as_ref()).unwrap();

                    match topic_gui_map_arc.get(topic.as_str()) {
                        Some(func) => {
                            func(ui_weak.clone(), payload, config_arc.clone());
                        },
                        None => println!("Received unknown topic"),
                    }
                }
            }
        }
    });

    for topic in topic_gui_map_arc2.keys() {
        client.subscribe(topic, QoS::AtLeastOnce).unwrap();
    }

    ui.run().unwrap();

    let _res = thread_join_handle.join();
}

fn make_full_topic(sensor_name: &str, config: &Config) -> String {
    let full_topic = config.mqtt_base_topic.clone() + "/" + &config.mqtt_controller_name + "_" + sensor_name + "/state";
    return full_topic;
}

fn update_indoor_t_rh(window_weak: Weak<AppWindow>, json_data: JsonValue, _config: Arc<Config>) {
    window_weak.upgrade_in_event_loop(move |window| {
        // TODO: json_data.has_key("")
        let value = json_data["temperature"].as_i32().unwrap_or(0);
        window.global::<IndoorAdapter>().set_current_temp(value);
        let value = json_data["rh"].as_i32().unwrap_or(0);
        window.global::<IndoorAdapter>().set_current_rh(value);
    }).unwrap();
}

fn update_indoor_co2(window_weak: Weak<AppWindow>, json_data: JsonValue, _config: Arc<Config>) {
    window_weak.upgrade_in_event_loop(move |window| {
        let value = json_data["co2"].as_i32().unwrap_or(0);
        window.global::<IndoorAdapter>().set_current_co2(value);
    }).unwrap();
}

fn convert_datetime(input: &str, in_format: &str, out_format: &str, offset_hours: i64) -> String {
    let mut datetime = NaiveDateTime::parse_from_str(input, in_format).expect("Failed to parse datetime");
    datetime += chrono::Duration::hours(offset_hours);
    return datetime.format(out_format).to_string();
}

// TODO: use ModelRC for updating chart data
fn update_space_weather_kp(window_weak: Weak<AppWindow>, json_data: JsonValue, config: Arc<Config>) {
    window_weak.upgrade_in_event_loop(move |window| {
        if !json_data.is_array() {
            println!("Format of received data is invalid! Should be array of elements");
            return;
        }
        let chart_data = VecModel::default();
        for element in json_data.members() {
            if !element.is_object() {
                println!("Format of received data element is invalid! Should be object of type KpIndex");
                continue;
            }
            chart_data.push(KpIndex {
                hour: convert_datetime(element["time_tag"].as_str().unwrap_or("00:00 01-01-2024"),
                                       "%H:%M %d-%m-%Y", "%H", config.timezone_offset_h.into()).into(),
                kp: element["kp"].as_f32().unwrap_or(0.0),
            });
        }

        // save current instant KP value from the last element ("row") of KP data array
        let current_kps_rc = window.global::<SpaceWeatherAdapter>().get_kp_index_data();
        let current_kps = current_kps_rc.as_any().downcast_ref::<VecModel<KpIndex>>().expect("error when downcasting sw kp");
        chart_data.push(current_kps.row_data(current_kps.row_count() - 1).unwrap_or_default());

        window.global::<SpaceWeatherAdapter>().set_kp_index_data(Rc::new(chart_data).into());
    }).unwrap();
}

fn update_space_weather_kp_inst(window_weak: Weak<AppWindow>, json_data: JsonValue, config: Arc<Config>) {
    window_weak.upgrade_in_event_loop(move |window| {
        if !json_data.is_object() {
            println!("Format of received data is invalid! Shouldn't be object of type KpIndex");
            return;
        }
        let kp_inst_val = KpIndex {
            hour: convert_datetime(json_data["time_tag"].as_str().unwrap_or("00:00 01-01-2024"),
                                   "%H:%M %d-%m-%Y", "%H", config.timezone_offset_h.into()).into(),
            kp: json_data["kp"].as_f32().unwrap_or(0.0),
        };

        // update instant KP data value in the last element ("row") of KP data array
        let current_kps_rc = window.global::<SpaceWeatherAdapter>().get_kp_index_data();
        let current_kps = current_kps_rc.as_any().downcast_ref::<VecModel<KpIndex>>().expect("error when downcasting sw kp");
        current_kps.set_row_data(current_kps.row_count() - 1, kp_inst_val);
    }).unwrap();
}

fn update_space_weather_flux(window_weak: Weak<AppWindow>, json_data: JsonValue, _: Arc<Config>) {
    window_weak.upgrade_in_event_loop(move |window| {
        if !json_data.is_array() {
            println!("Format of received data is invalid! Should be array of elements");
            return;
        }
        let current_flux = json_data[0]["flux_gt10mev"].as_f32().unwrap_or(0.0);
        println!("current flux greater 10 Mev: {:#?}", current_flux);
        window.global::<SpaceWeatherAdapter>().set_solar_radiation_now(current_flux);
    }).unwrap();
}

fn update_sw_forecast(window_weak: Weak<AppWindow>, json_data: JsonValue, config: Arc<Config>) {
    window_weak.upgrade_in_event_loop(move |window| {
        if !json_data.is_object() {
            println!("Format of received data is invalid! Should be object");
            return;
        }

        // pub struct SWForecast {
        //     pub kp: Vec<KPForecast>,
        //     pub srs: Vec<SRSRBForecast>,
        //     pub rb: Vec<SRSRBForecast>,
        // }

        if !(json_data.has_key("kp") && json_data.has_key("srs") && json_data.has_key("rb")) {
            println!("Received json data does not contain the necessary keys!");
            return;
        }

        let current_date: DateTime<Utc> = Utc::now();

        if json_data.has_key("kp") {
            match extract_kp_forecast(&json_data["kp"], current_date) {
                Some((kpf_3h, kpf_1d, kpf_3d)) => {
                    println!("kpf_3h: {:#?}, kpf_1d: {:#?}, kpf_3d: {:#?}", kpf_3h, kpf_1d, kpf_3d);
                    window.global::<SpaceWeatherAdapter>().set_kp_forecast_3h(kpf_3h.into());
                    window.global::<SpaceWeatherAdapter>().set_kp_forecast_24h(kpf_1d.into());
                },
                None => {
                    println!("Couldn't extract Kp forecast from MQTT data");
                }
            }
        }

        match extract_srs_rb_forecast(&json_data["srs"], current_date, config.srs_min_prob_thrh.into()) {
            Some((srs_1d, srs_3d)) => {
                println!("srs_1d: {:#?}, srs_3d: {:#?}", srs_1d, srs_3d);
                // FIXME: temporary using 1d forecast data as 3h forecast
                window.global::<SpaceWeatherAdapter>().set_solar_radiation_forecast_3h(srs_1d.into());
                window.global::<SpaceWeatherAdapter>().set_solar_radiation_forecast_24h(srs_1d.into());
            },
            None => {
                println!("Couldn't extract SRS forecast from MQTT data");
            }
        }

        // Now it's only for debug
        match extract_srs_rb_forecast(&json_data["rb"], current_date, config.rb_min_prob_thrh.into()) {
            Some((rb_1d, rb_3d)) => {
                println!("rb_1d: {:#?}, rb_3d: {:#?}", rb_1d, rb_3d);
            },
            None => {
                println!("Couldn't extract RB forecast from MQTT data");
            }
        }
    }).unwrap();
}

// Data format:
// struct KPForecast {
//     pub date: String,
//     pub hour: u8,
//     pub value: f32,
// }
fn extract_kp_forecast(kp_vec_json: &JsonValue, current_datetime: DateTime<Utc>) -> Option<(f32, f32, f32)> {
    let current_date = current_datetime.format("%b %d %Y").to_string();
    let current_hour = current_datetime.hour();
    if !kp_vec_json.is_array() {
        println!("Format of 'kp' data is invalid! Should be array of elements");
        return None;
    }
    let mut kp_3h = 0.0;
    let mut kp_1d = 0.0;
    let mut kp_3d = 0.0;
    let mut interval_1d_started: bool = false;
    let mut interval_3d_started: bool = false;
    let mut interval_1d_counter = 0;
    for kp in kp_vec_json.members() {
        let date = kp["date"].as_str().unwrap_or_default();
        let kp_val = kp["value"].as_f32().unwrap_or_default();
        if date == current_date {
            let mut hour = match kp["hour"].as_u32() {
                Some(val) => val,
                None => { continue; }
            };
            if hour > 23 {
                continue;
            }
            if hour == 0 {
                hour = 24;
            }
            let interval_start = if hour < 3 { 0 } else { hour - 3 };
            if current_hour <= hour && current_hour >= interval_start {
                kp_3h = kp_val;
                interval_1d_started = true;
                interval_3d_started = true;
            }
        }
        if interval_1d_started {
            if interval_1d_counter < 8 {
                interval_1d_counter = interval_1d_counter + 1;
                kp_1d = if kp_val > kp_1d { kp_val } else { kp_1d };
            }
        }
        if interval_3d_started {
            kp_3d = if kp_val > kp_3d { kp_val } else { kp_3d };
        }
    }
    return Some((kp_3h, kp_1d, kp_3d));
}

// Data format:
// struct SRSRBForecast {
//     pub date: String,
//     pub s1: u8,
//     pub s2: u8,
//     pub s3: u8,
//     pub s4: u8,
//     pub s5: u8,
// }
fn extract_srs_rb_forecast(srs_vec_json: &JsonValue, current_datetime: DateTime<Utc>, min_prob_thrh: u8) -> Option<(f32, f32)> {
    let current_date = current_datetime.format("%b %d %Y").to_string();

    let mut srs_1d_max_storm_level = 0;
    let mut srs_3d_max_storm_level = 0;
    let mut interval_3d_started: bool = false;
    for srs in srs_vec_json.members() {
        let date = srs["date"].as_str().unwrap_or_default();
        let (max_storm_level, _) = get_max_storm(srs, min_prob_thrh).unwrap_or((0, 0));
        if date == current_date {
            srs_1d_max_storm_level = max_storm_level;
            interval_3d_started = true;
        }
        if interval_3d_started && max_storm_level > srs_3d_max_storm_level {
            srs_3d_max_storm_level = max_storm_level;
        }
    }

    let srs_1d = convert_srs_level_to_flux(srs_1d_max_storm_level);
    let srs_3d = convert_srs_level_to_flux(srs_3d_max_storm_level);
    return Some((srs_1d, srs_3d));
}

fn get_max_storm(srs_json: &JsonValue, prob_thrh: u8) -> Option<(u8, u8)> {
    let srs = [
        srs_json["s5"].as_u8().unwrap_or(0),
        srs_json["s4"].as_u8().unwrap_or(0),
        srs_json["s3"].as_u8().unwrap_or(0),
        srs_json["s2"].as_u8().unwrap_or(0),
        srs_json["s1"].as_u8().unwrap_or(0)
    ];
    let mut max_storm_level: u8 = 0;
    let mut probability: u8 = 0;
    for (index, s) in srs.iter().enumerate() {
        if *s > prob_thrh {
            max_storm_level = (5 - index) as u8;
            probability = *s;
            break;
        }
    }
    return Some((max_storm_level, probability));
}

fn convert_srs_level_to_flux(level: u8) -> f32 {
    if level == 1 {
        return 11.0;
    } else if level == 2 {
        return 101.0;
    } else if level == 3 {
        return 1001.0;
    } else if level == 4 {
        return 10001.0;
    } else if level == 5 {
        return 100001.0;
    }
    return 0.0;
}
