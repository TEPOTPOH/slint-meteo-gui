use slint::*;
use std::rc::Rc;

slint::include_modules!();

pub type KpIndexUI = KpIndex;

#[derive(Clone)]
pub struct WindowUpdater {
    pub window_weak: Weak<AppWindow>
}

impl WindowUpdater {
    pub fn new(window_weak: Weak<AppWindow>) -> Self {
        Self {window_weak}
    }

    // gui element updaters
    pub fn update_indoor_t(&self, value: i32) {
        self.window_weak.upgrade_in_event_loop(move |window| {
            window.global::<IndoorAdapter>().set_current_temp(value);
        }).unwrap();
    }
    pub fn update_indoor_rh(&self, value: i32) {
        self.window_weak.upgrade_in_event_loop(move |window| {
            window.global::<IndoorAdapter>().set_current_rh(value);
        }).unwrap();
    }
    pub fn update_indoor_co2(&self, value: i32) {
        self.window_weak.upgrade_in_event_loop(move |window| {
            window.global::<IndoorAdapter>().set_current_co2(value);
        }).unwrap();
    }
    pub fn update_solar_radiation_now(&self, value: f32) {
        self.window_weak.upgrade_in_event_loop(move |window| {
            window.global::<SpaceWeatherAdapter>().set_solar_radiation_now(value);
        }).unwrap();
    }
    pub fn update_kp_forecast_3h(&self, value: f32) {
        self.window_weak.upgrade_in_event_loop(move |window| {
            window.global::<SpaceWeatherAdapter>().set_kp_forecast_3h(value.into());
        }).unwrap();
    }
    pub fn update_kp_forecast_24h(&self, value: f32) {
        self.window_weak.upgrade_in_event_loop(move |window| {
            window.global::<SpaceWeatherAdapter>().set_kp_forecast_24h(value.into());
        }).unwrap();
    }
    pub fn update_solar_radiation_forecast_3h(&self, value: f32) {
        self.window_weak.upgrade_in_event_loop(move |window| {
            window.global::<SpaceWeatherAdapter>().set_solar_radiation_forecast_3h(value.into());
        }).unwrap();
    }
    pub fn update_solar_radiation_forecast_24h(&self, value: f32) {
        self.window_weak.upgrade_in_event_loop(move |window| {
            window.global::<SpaceWeatherAdapter>().set_solar_radiation_forecast_24h(value.into());
        }).unwrap();
    }
    pub fn update_kp_index_data(&self, data: Vec<KpIndexUI>) {
        self.window_weak.upgrade_in_event_loop(move |window| {
            let chart_data = VecModel::from(data);
            // save current instant KP value from the last element ("row") of KP data array
            let current_kps_rc = window.global::<SpaceWeatherAdapter>().get_kp_index_data();
            let current_kps = current_kps_rc.as_any().downcast_ref::<VecModel<KpIndexUI>>().expect("error when downcasting sw kp");
            chart_data.push(current_kps.row_data(current_kps.row_count() - 1).unwrap_or_default());

            window.global::<SpaceWeatherAdapter>().set_kp_index_data(Rc::new(chart_data).into());
        }).unwrap();
    }
    pub fn update_kp_index_instant(&self, value: KpIndexUI) {
        self.window_weak.upgrade_in_event_loop(move |window| {
            // update instant KP data value in the last element ("row") of KP data array
            let current_kps_rc = window.global::<SpaceWeatherAdapter>().get_kp_index_data();
            let current_kps = current_kps_rc.as_any().downcast_ref::<VecModel<KpIndexUI>>().expect("error when downcasting sw kp");
            current_kps.set_row_data(current_kps.row_count() - 1, value);
        }).unwrap();
    }
    pub fn update_video_frame(&self, data: slint::SharedPixelBuffer<slint::Rgb8Pixel>) {
        self.window_weak.upgrade_in_event_loop(move |window| {
            window.global::<VideoAdapter>().set_video_frame(slint::Image::from_rgb8(data));
        }).unwrap();
    }
}
