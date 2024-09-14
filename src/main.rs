use std::sync::Arc;
use envconfig::Envconfig;
use slint::*;
use ui::*;
use model::Model;
use connector::MQTTConnector;


pub mod ui;
pub mod model;
pub mod connector;
pub mod video;

#[derive(Debug)]
#[derive(Envconfig)]
pub struct Config {
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

fn main() {
    let config = Config::init_from_env().unwrap();
    println!("Using config:\n{:?}", config);
    let config_ref = Arc::new(config);

    // UI
    let ui = AppWindow::new().unwrap();
    let window_updater = WindowUpdater::new(ui.as_weak());

    let mut meteo_model = Model::new(config_ref.clone());

    // data_view_map
    meteo_model.add_map(vec![
        ("htu21d", Model::indoor_t_rh_callback),
        ("mhz19", Model::indoor_co2_callback),
        ("noaa_kp",Model::space_weather_kp_callback),
        ("noaa_kp_inst", Model::space_weather_kp_inst_callback),
        ("noaa_flux", Model::space_weather_flux_callback),
        ("noaa_sw_forecast", Model::space_weather_forecast_callback)
    ]);

    // Connector
    let meteo_model_ref = Arc::new(meteo_model);
    let meteo_model_ref2 = meteo_model_ref.clone();

    let on_notify_cb = move |topic, payload| {
        meteo_model_ref2.on_notification(window_updater.clone(), topic, payload);
    };
    let mut mqtt_connector = MQTTConnector::new("display", config_ref.clone(), on_notify_cb).unwrap();

    for topic in meteo_model_ref.data_view_map.keys() {
        mqtt_connector.subscribe_client(topic);
    }

    // Video
    video::init_pipeline("https://s2.moidom-stream.ru/s/public/0000091581.m3u8",
                        500,
                        WindowUpdater::new(ui.as_weak()));

    ui.run().unwrap();
}
