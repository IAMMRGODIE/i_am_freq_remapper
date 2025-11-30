use rhai::packages::Package;
use std::hash::BuildHasher;
use std::hash::RandomState;
use rhai::Scope;
use rhai::AST;
use rhai::Engine;
use rustfft::FftPlanner;
use std::f32::consts::PI;
use crate::ring_buffer::RingBuffer;
use rustfft::Fft;
use rustfft::num_complex::Complex;
use crate::Arc;
use rhai_rand::RandomPackage;

const OVERLAP_RATIO: usize = 4;

lazy_static::lazy_static! {
	static ref RHAI_ENGINE: Engine = {
		let mut engine = Engine::new();
		engine.register_global_module(RandomPackage::new().as_shared_module());
		engine
	};
	static ref HASHER: RandomState = RandomState::new();
	static ref EMPTY_HASH: u64 = HASHER.hash_one("");
}

#[derive(Default)]
pub struct InputParams {
	pub daw_values: [f32; 4],
	// pub sustain_values: [f32; 4],
	pub current_track_id: usize,
	pub bpm: f32,
	pub daw_time: f32,
	pub sys_time: f32, 
	pub window_size: usize,
	pub window_factor: f32,
	pub window_offset: usize,
	pub gain: f32,
	pub sample_rate: f32,
}

pub struct PhaseVocoder {
	sample_rate: f32,
	window_size: usize,
	frame_hop: usize,

	fft: Arc<dyn Fft<f32>>,
	ifft: Arc<dyn Fft<f32>>,

	input_buffer: RingBuffer<f32>,
	output_buffer: RingBuffer<f32>,
	prev_analysis_phase: Vec<f32>,
	// prev_synthesis_phase: Vec<f32>,

	// window: Vec<f32>,
	bin_frequencies: Vec<f32>,

	temp_buffer: Vec<Complex<f32>>,
	output_temp_buffer: Vec<Complex<f32>>,

	input_count: usize,
	output_count: usize,

	map_ast: Option<AST>,
	hash: u64
}

fn window(window_size: usize, index: usize, offset: usize, window_factor: f32) -> f32 {
	let index = (index + offset) % window_size;
	0.5 * (window_factor - (1.0 - window_factor) * (2.0 * PI * index as f32 / window_size as f32).cos())
}

impl PhaseVocoder {
	pub fn new(window_size: usize, sample_rate: f32) -> Self {
		let window_size = window_size.next_power_of_two();
		let window_size = window_size.max(OVERLAP_RATIO);

		let frame_hop = window_size / OVERLAP_RATIO;

		let input_buffer = RingBuffer::new(window_size);
		let output_buffer = RingBuffer::new(window_size);

		let bin_frequencies = (0..window_size).map(|k| k as f32 * sample_rate / window_size as f32).collect();

		let mut planner = FftPlanner::new();
		let fft = planner.plan_fft_forward(window_size);
		let ifft = planner.plan_fft_inverse(window_size);

		let prev_analysis_phase = vec![0.0; window_size];

		let temp_buffer = vec![Complex::ZERO; window_size];
		let output_temp_buffer = vec![Complex::ZERO; window_size];

		Self {
			window_size,
			frame_hop,
			input_buffer,
			output_buffer,
			bin_frequencies,
			fft,
			ifft,
			prev_analysis_phase,
			sample_rate,
			temp_buffer,
			output_temp_buffer,
			input_count: 0,
			output_count: 0,
			map_ast: None,
			hash: *EMPTY_HASH
		}
	}

	pub fn clear_mapper(&mut self) {
		self.map_ast = None;
	}

	pub fn update_mapping(&mut self, code: impl AsRef<str>) -> Result<(), String> {
		let code = code.as_ref();
		if code.is_empty() {
			self.clear_mapper();
			self.hash = *EMPTY_HASH;
			return Ok(())
		}
		let hash = HASHER.hash_one(code);
		if hash == self.hash {
			return Ok(())
		}
		let ast = RHAI_ENGINE.compile(code).map_err(|e| format!("{e}"))?;
		let ori = self.map_ast.replace(ast);
		if let Err(e) = self.frequency_mapper(&Default::default(), 0.0, 0.0) {
			self.map_ast = ori;
			return Err(e)
		}
		self.hash = hash;

		Ok(())
	}

	fn frequency_mapper(
		&self, 
		params: &InputParams,
		frequency: f32, 
		magnitude: f32
	) -> Result<(f32, f32), String> {
		let ast = if let Some(ast) = &self.map_ast {
			ast
		}else {
			return Ok((frequency, magnitude))
		};

		let mut scope = Scope::new();
		scope.push("a", params.daw_values[0]);
		scope.push("b", params.daw_values[1]);
		scope.push("c", params.daw_values[2]);
		scope.push("d", params.daw_values[3]);

		scope.push("sound_channel_id", params.current_track_id as i32);
		scope.push("bpm", params.bpm);
		scope.push("daw_time", params.daw_time);
		scope.push("sys_time", params.sys_time);
		scope.push("window_size", params.window_size as i32);
		scope.push("sample_rate", params.sample_rate);

		scope.push("frequency", frequency);
		scope.push("magnitude", magnitude);

		RHAI_ENGINE.run_ast_with_scope(&mut scope, ast).map_err(|e| format!("{e}"))?;
		
		let (frequency, magnitude) = (
			scope.remove("frequency").unwrap_or(frequency), 
			scope.remove("magnitude").unwrap_or(magnitude)
		);

		Ok((frequency, magnitude))
	}

	pub fn renew_window_size(&mut self, window_size: usize) -> Option<usize> {
		let window_size = window_size.next_power_of_two();
		let window_size = window_size.max(OVERLAP_RATIO);

		if window_size == self.window_size {
			return None;
		}

		self.window_size = window_size;
		self.frame_hop = window_size / OVERLAP_RATIO;

		self.input_buffer = RingBuffer::new(window_size);
		self.output_buffer = RingBuffer::new(window_size);

		self.bin_frequencies = (0..window_size).map(|k| k as f32 * self.sample_rate / window_size as f32).collect();

		let mut planner = FftPlanner::new();
		self.fft = planner.plan_fft_forward(window_size);
		self.ifft = planner.plan_fft_inverse(window_size);

		self.prev_analysis_phase = vec![0.0; window_size];
		// self.prev_synthesis_phase = vec![0.0; window_size];

		self.temp_buffer = vec![Complex::ZERO; window_size];
		self.output_temp_buffer = vec![Complex::ZERO; window_size];

		self.input_count = 0;
		self.output_count = 0;

		Some(window_size)
	}

	pub fn renew_sample_rate(&mut self, sample_rate: f32) {
		if self.sample_rate == sample_rate {
			return;
		}

		self.sample_rate = sample_rate;
		self.bin_frequencies = (0..self.window_size).map(|k| k as f32 * self.sample_rate / self.window_size as f32).collect();
	}

	pub fn process(&mut self, signal: &mut [f32], input_params: &InputParams) {
		self.renew_window_size(input_params.window_size);
		self.renew_sample_rate(input_params.sample_rate);

		for sample in signal.iter_mut() {
			self.input_buffer.push(*sample);
			self.input_count += 1;
			*sample = self.output_buffer[self.output_count] * 4.0;
			self.output_count = (self.output_count + 1) % self.output_buffer.capacity(); 
			if self.input_count >= self.frame_hop {
				self.output_buffer.extend_defaults(self.frame_hop);
				self.input_count -= self.frame_hop;
				self.output_count = (self.output_count + self.output_buffer.capacity() - self.frame_hop) % self.output_buffer.capacity();
				self.process_inner(input_params);
			}
		}
	}

	fn process_inner(&mut self, input_params: &InputParams) {
		for (i, value) in self.temp_buffer.iter_mut().enumerate() {
			*value = Complex::new(
				window(self.window_size, i, input_params.window_offset, input_params.window_factor) * self.input_buffer[i], 
				0.0
			);
			self.output_temp_buffer[i] = Complex::ZERO;
		}

		self.fft.process(&mut self.temp_buffer);

		for (k, value) in self.temp_buffer.iter().enumerate().take(self.window_size / 2 + 1) {
			if k == 0 {
				self.output_temp_buffer[0] = *value;
				continue;
			}

			let magnitude = value.norm();
			let bin_center_freq = self.bin_frequencies[k];
			let Ok((mapped_freq, magnitude)) = self.frequency_mapper(input_params, bin_center_freq, magnitude) else { unreachable!() };

			if mapped_freq < 0.0 || mapped_freq >= self.sample_rate / 2.0 {
				continue;
			}

			let new_phase = 
				self.prev_analysis_phase[k] + 
				2.0 * PI * bin_center_freq * self.frame_hop as f32 / self.sample_rate;

			self.prev_analysis_phase[k] = value.arg();

			let new_idx = mapped_freq / self.sample_rate * self.window_size as f32;
			let ratio = new_idx.fract();
			let k_low = new_idx.floor() as usize;

			if k_low <= self.window_size / 2 {
				self.output_temp_buffer[k_low] += (1.0 - ratio) * Complex::from_polar(magnitude, new_phase);
			}
			if k_low < self.window_size / 2 {
				self.output_temp_buffer[k_low + 1] += ratio * Complex::from_polar(magnitude, new_phase);
			}
		}

		self.output_temp_buffer[0].im = 0.0;
		self.output_temp_buffer[self.window_size / 2].im = 0.0;
		for i in 1..self.window_size / 2 {
			self.output_temp_buffer[self.window_size - i] = self.output_temp_buffer[i].conj();	
		}

		self.ifft.process(&mut self.output_temp_buffer);

		for i in 0..self.window_size {
			self.output_buffer[i] += 
				self.output_temp_buffer[i].re * 
				window(self.window_size, i, input_params.window_offset, input_params.window_factor) / 
				self.window_size as f32 *
				input_params.gain;
		}

	}
}