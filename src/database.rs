use std::sync::Arc;
use crate::Config;
use std::time::{Duration, Instant};


pub struct DataBase {
    pub indoor_t_history: HistoryContainer,
    pub indoor_rh_history: HistoryContainer,
    pub indoor_co2_history: HistoryContainer
}

impl DataBase {
    pub fn new(config: Arc<Config>) -> Self {
        DataBase {
            indoor_t_history: HistoryContainer::new(config.history_n_elements,
                                                    Duration::from_secs(config.history_interval_s)),
            indoor_rh_history: HistoryContainer::new(config.history_n_elements,
                                                     Duration::from_secs(config.history_interval_s)),
            indoor_co2_history: HistoryContainer::new(config.history_n_elements,
                                                     Duration::from_secs(config.history_interval_s))
        }
    }
}

pub struct HistoryContainer {
    data: Vec<i32>,
    interval: std::time::Duration,
    current_time_index: usize,
    last_timestamp: std::time::Instant
}

impl HistoryContainer {
    pub fn new(num_elements: usize, interval: std::time::Duration) -> Self {
        Self {
            data: vec![0; num_elements],
            interval,
            current_time_index: 0,
            last_timestamp: Instant::now()
        }
    }
    pub fn insert(&mut self, val: i32) -> bool {
        self.data[0] = val;

        if self.last_timestamp.elapsed() >= self.interval {
            self.last_timestamp = Instant::now();
            self.current_time_index += 1;
            self.current_time_index = if self.current_time_index >= self.data.len() {1}
                                      else {self.current_time_index};
            self.data[self.current_time_index] = val;
            return true;
        }
        return false;
    }
    pub fn get_history(&self) -> Vec<i32> {
        let mut history = vec![0; self.data.len()];
        let last_index = self.data.len() - 1;
        // copy elements with instant value
        history[last_index] = self.data[0];

        if self.current_time_index == 0 {
            return history;
        }

        // copy the most old elements
        let rest_length = last_index - self.current_time_index;
        if rest_length > 0 {
            history[0..rest_length].copy_from_slice(&self.data[(self.current_time_index + 1)..]);
        }
        // copy the most recent elements
        history[rest_length..(rest_length + self.current_time_index)].copy_from_slice(&self.data[1..=self.current_time_index]);
        history
    }
}
