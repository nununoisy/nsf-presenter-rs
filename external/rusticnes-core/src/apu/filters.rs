#![allow(non_snake_case)]

use std::f32::consts::PI;

pub trait DspFilter: Send {
    fn consume(&mut self, sample: f32);
    fn output(&self) -> f32;
}

pub struct IdentityFilter {
    sample: f32
}

impl IdentityFilter {
    pub fn new() -> IdentityFilter {
        return IdentityFilter {
            sample: 0.0
        }
    }
}

impl DspFilter for IdentityFilter {
    fn consume(&mut self, new_input: f32) {
        self.sample = new_input;
    }

    fn output(&self) -> f32 {
        return self.sample;
    }
}

pub struct HighPassIIR {
    alpha: f32,
    previous_output: f32,
    previous_input: f32,
    delta: f32,
}

impl HighPassIIR {
    pub fn new(sample_rate: f32, cutoff_frequency: f32) -> HighPassIIR {
        let delta_t = 1.0 / sample_rate;
        let time_constant = 1.0 / cutoff_frequency;
        let alpha = time_constant / (time_constant + delta_t);
        return HighPassIIR {
            alpha: alpha,
            previous_output: 0.0,
            previous_input: 0.0,
            delta: 0.0,
        }
    }
}

impl DspFilter for HighPassIIR {
    fn consume(&mut self, new_input: f32) {
        self.previous_output = self.output();
        self.delta = new_input - self.previous_input;
        self.previous_input = new_input;
    }

    fn output(&self) -> f32 {
        return self.alpha * self.previous_output + self.alpha * self.delta;
    }
}

pub struct LowPassIIR {
    alpha: f32,
    previous_output: f32,
    delta: f32,
}

impl LowPassIIR {
    pub fn new(sample_rate: f32, cutoff_frequency: f32) -> LowPassIIR {
        let delta_t = 1.0 / sample_rate;
        let time_constant = 1.0 / (2.0 * PI * cutoff_frequency);
        let alpha = delta_t / (time_constant + delta_t);
        return LowPassIIR {
            alpha: alpha,
            previous_output: 0.0,
            delta: 0.0,
        }
    }
}

impl DspFilter for LowPassIIR {
    fn consume(&mut self, new_input: f32) {
        self.previous_output = self.output();
        self.delta = new_input - self.previous_output;
    }

    fn output(&self) -> f32 {
        return self.previous_output + self.alpha * self.delta;
    }
}

fn blackman_window(index: usize, window_size: usize) -> f32 {
    let i = index as f32;
    let M = window_size as f32;
    return 0.42 - 0.5 * ((2.0 * PI * i) / M).cos() + 0.08 * ((4.0 * PI * i) / M).cos();
}

fn sinc(index: usize, window_size: usize, fc: f32) -> f32 {
    let i = index as f32;
    let M = window_size as f32;
    let shifted_index = i - (M / 2.0);
    if index == (window_size / 2) {
        return 2.0 * PI * fc;
    } else {
        return (2.0 * PI * fc * shifted_index).sin() / shifted_index;
    }
}

fn normalize(input: Vec<f32>) -> Vec<f32> {
    let sum: f32 = input.clone().into_iter().sum();
    let output = input.into_iter().map(|x| x / sum).collect();
    return output;
}

fn windowed_sinc_kernel(fc: f32, window_size: usize) -> Vec<f32> {
    let mut kernel: Vec<f32> = Vec::new();
    for i in 0 ..= window_size {
        kernel.push(sinc(i, window_size, fc) * blackman_window(i, window_size));
    }
    return normalize(kernel);
}

pub struct LowPassFIR {
    kernel: Vec<f32>,
    inputs: Vec<f32>,
    input_index: usize,
}

impl LowPassFIR {
    pub fn new(sample_rate: f32, cutoff_frequency: f32, window_size: usize) -> LowPassFIR {
        let fc = cutoff_frequency / sample_rate;
        let kernel = windowed_sinc_kernel(fc, window_size);
        let mut inputs = Vec::new();
        inputs.resize(window_size + 1, 0.0);
        
        return LowPassFIR {
            kernel: kernel,
            inputs: inputs,
            input_index: 0,
        }
    }
}

impl DspFilter for LowPassFIR {
    fn consume(&mut self, new_input: f32) {
        self.inputs[self.input_index] = new_input;
        self.input_index = (self.input_index + 1) % self.inputs.len();
    }

    fn output(&self) -> f32 {
        let mut output: f32 = 0.0;
        for i in 0 .. self.inputs.len() {
            let buffer_index = (self.input_index + i) % self.inputs.len();
            output += self.kernel[i] * self.inputs[buffer_index];
        }
        return output;
    }
}

// essentially a thin wrapper around a DspFilter, with some bonus data to track
// state when used in a larger chain
pub struct ChainedFilter {
    wrapped_filter: Box<dyn DspFilter>,
    sampling_period: f32,
    period_counter: f32,
}

pub struct FilterChain {
    filters: Vec<ChainedFilter>,
}

impl FilterChain {
    pub fn new() -> FilterChain {
        let identity = IdentityFilter::new();
        return FilterChain {
            filters: vec![ChainedFilter{
                wrapped_filter: Box::new(identity),
                sampling_period: 1.0,
                period_counter: 0.0,
            }],
        }
    }

    pub fn add(&mut self, filter: Box<dyn DspFilter>, sample_rate: f32) {
        self.filters.push(ChainedFilter {
            wrapped_filter: filter,
            sampling_period: (1.0 / sample_rate),
            period_counter: 0.0
        });
    }

    pub fn consume(&mut self, input_sample: f32, delta_time: f32) {
        // Always advance the identity filter with the new current sample
        self.filters[0].wrapped_filter.consume(input_sample);
        // Now for every remaining filter in the chain, advance and sample the previous
        // filter as required
        for i in 1 .. self.filters.len() {
            let previous = i - 1;
            let current = i;
            self.filters[current].period_counter += delta_time;
            while self.filters[current].period_counter >= self.filters[current].sampling_period {
                self.filters[current].period_counter -= self.filters[current].sampling_period;
                let previous_output = self.filters[previous].wrapped_filter.output();
                self.filters[current].wrapped_filter.consume(previous_output);
            }
        }
    }

    pub fn output(&self) -> f32 {
        let final_filter = self.filters.last().unwrap();
        return final_filter.wrapped_filter.output();
    }
}