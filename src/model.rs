use json::*;
use std::sync::Arc;
use chrono::{DateTime, NaiveDateTime, Timelike, Utc};
use crate::ui::{WindowUpdater, KpIndexUI};
use crate::Config;
use std::collections::HashMap;

pub type DataViewMapKeyType = String;
pub type DataViewMapValueType = fn(&Model, WindowUpdater, JsonValue) -> ();
pub type DataViewMap = HashMap<DataViewMapKeyType, DataViewMapValueType>;

pub struct Model {
    pub config: Arc<Config>,
    pub data_view_map: DataViewMap
}


impl Model {
    pub fn new(config: Arc<Config>) -> Self {
        Model {config, data_view_map: DataViewMap::new()}
    }

    pub fn on_notification(&self, updater: WindowUpdater, topic: String, payload: json::JsonValue) {
        if self.data_view_map.contains_key(topic.as_str()) {
            // println!("json_payload = {:?}", payload);
            match self.data_view_map.get(topic.as_str()) {
                Some(callback) => {
                    callback(self, updater, payload);
                },
                None => println!("Received unknown topic"),
            }
        }
    }

    // DataViewMap
    pub fn add_map(&mut self, map: Vec<(&'static str, DataViewMapValueType)>) {
        for element in map {
            let full_topic = make_full_topic(element.0.into(), self.config.as_ref());
            self.data_view_map.insert(full_topic.into(), element.1);
        }
    }

    // callbacks
    pub fn indoor_t_rh_callback(&self, updater: WindowUpdater, json_data: JsonValue) {
        let t = json_data["temperature"].as_i32().unwrap_or(0);
        let rh = json_data["rh"].as_i32().unwrap_or(0);

        updater.update_indoor_t(t);
        updater.update_indoor_rh(rh);
    }
    pub fn indoor_co2_callback(&self, updater: WindowUpdater, json_data: JsonValue) {
        let value = json_data["co2"].as_i32().unwrap_or(0);
        updater.update_indoor_co2(value);
    }
    pub fn space_weather_kp_callback(&self, updater: WindowUpdater, json_data: JsonValue) {
        if !json_data.is_array() {
            println!("Format of received data is invalid! Should be array of elements");
            return;
        }

        let mut kp_data: Vec<KpIndexUI> = Vec::with_capacity(json_data.members().count());
        for element in json_data.members() {
            if !element.is_object() {
                println!("Format of received data element is invalid! Should be object of type KpIndexUI");
                continue;
            }
            kp_data.push(KpIndexUI {
                hour: convert_datetime(element["time_tag"].as_str().unwrap_or("00:00 01-01-2024"),
                                        "%H:%M %d-%m-%Y", "%H", self.config.timezone_offset_h.into()).into(),
                kp: element["kp"].as_f32().unwrap_or(0.0),
            });
        }

        updater.update_kp_index_data(kp_data);
    }
    pub fn space_weather_kp_inst_callback(&self, updater: WindowUpdater, json_data: JsonValue) {
        if !json_data.is_object() {
            println!("Format of received data is invalid! Shouldn't be object of type KpIndexUI");
            return;
        }
        let kp_inst_val = KpIndexUI {
            hour: convert_datetime(json_data["time_tag"].as_str().unwrap_or("00:00 01-01-2024"),
                                    "%H:%M %d-%m-%Y", "%H", self.config.timezone_offset_h.into()).into(),
            kp: json_data["kp"].as_f32().unwrap_or(0.0),
        };

        updater.update_kp_index_instant(kp_inst_val);
    }
    pub fn space_weather_flux_callback(&self, updater: WindowUpdater, json_data: JsonValue) {
        if !json_data.is_array() {
            println!("Format of received data is invalid! Should be array of elements");
            return;
        }
        let current_flux = json_data[0]["flux_gt10mev"].as_f32().unwrap_or(0.0);
        println!("current flux greater 10 Mev: {:#?}", current_flux);
        updater.update_solar_radiation_now(current_flux);
    }
    pub fn space_weather_forecast_callback(&self, updater: WindowUpdater, json_data: JsonValue) {
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
                    updater.update_kp_forecast_3h(kpf_3h.into());
                    updater.update_kp_forecast_24h(kpf_1d.into());
                },
                None => {
                    println!("Couldn't extract Kp forecast from MQTT data");
                }
            }
        }

        match extract_srs_rb_forecast(&json_data["srs"], current_date, self.config.srs_min_prob_thrh.into()) {
            Some((srs_1d, srs_3d)) => {
                println!("srs_1d: {:#?}, srs_3d: {:#?}", srs_1d, srs_3d);
                // FIXME: temporary using 1d forecast data as 3h forecast
                updater.update_solar_radiation_forecast_3h(srs_1d.into());
                updater.update_solar_radiation_forecast_24h(srs_1d.into());
            },
            None => {
                println!("Couldn't extract SRS forecast from MQTT data");
            }
        }

        // Now it's only for debug
        match extract_srs_rb_forecast(&json_data["rb"], current_date, self.config.rb_min_prob_thrh.into()) {
            Some((rb_1d, rb_3d)) => {
                println!("rb_1d: {:#?}, rb_3d: {:#?}", rb_1d, rb_3d);
            },
            None => {
                println!("Couldn't extract RB forecast from MQTT data");
            }
        }
    }
}


fn make_full_topic(sensor_name: &str, config: &Config) -> String {
    let full_topic = config.mqtt_base_topic.clone() + "/" + &config.mqtt_controller_name + "_" + sensor_name + "/state";
    return full_topic;
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

fn convert_datetime(input: &str, in_format: &str, out_format: &str, offset_hours: i64) -> String {
    let mut datetime = NaiveDateTime::parse_from_str(input, in_format).expect("Failed to parse datetime");
    datetime += chrono::Duration::hours(offset_hours);
    return datetime.format(out_format).to_string();
}
