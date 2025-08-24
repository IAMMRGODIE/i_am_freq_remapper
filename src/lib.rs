pub mod phase_vocoder;
pub mod ring_buffer;

use time::OffsetDateTime;
use std::sync::RwLock;
use crate::egui::Vec2;
use crate::phase_vocoder::InputParams;
use crate::phase_vocoder::PhaseVocoder;
use nih_plug_egui::widgets::ParamSlider;
use nih_plug_egui::egui;
use nih_plug_egui::create_egui_editor;
use nih_plug_egui::EguiState;
use std::sync::Arc;
use nih_plug::prelude::*;

const WINDOW_SIZE: usize = 2048;

#[cfg(feature = "zh_cn_support")]
const FONT: &[u8; 7094212] = include_bytes!("../LXGWNeoXiHei.ttf");

struct Interface {
	pub params: Arc<Arguments>,

	instant: OffsetDateTime,
	processor: [Option<PhaseVocoder>; 2],
}

cfg_if::cfg_if! {
	if #[cfg(all(feature = "en_us", feature = "zh_cn"))] {
		#[derive(Default, PartialEq)]
		enum Language {
			#[default] ZhCn,
			EnUs,
		}

		#[derive(Default)]
		struct GuiInfo {
			show_code: bool,
			language: Language,
		}
	}else if #[cfg(feature = "zh_cn")] {
		#[derive(Default)]
		struct GuiInfo {
			show_code: bool,
		}
	}else if #[cfg(feature = "en_us")] {
		#[derive(Default)]
		struct GuiInfo {
			show_code: bool,
		}
	}else {
		compile_error!{"At least one language must be set."}
	}
}

impl Default for Interface {
	fn default() -> Self {
		Self {
			params: Default::default(),
			instant: OffsetDateTime::now_utc(),
			processor: Default::default(),
		}
	}
}

#[derive(Params)]
pub struct Arguments  {
	#[persist = "editor-state"]
	editor_state: Arc<EguiState>,

	#[id = "a"]
	pub a: FloatParam,
	#[id = "b"]
	pub b: FloatParam,
	#[id = "c"]
	pub c: FloatParam,
	#[id = "d"]
	pub d: FloatParam,

	#[id = "gain"]
	pub gain: FloatParam,

	#[id = "window_size"]
	pub window_size: IntParam,
	#[id = "window_offset"]
	pub window_offset: IntParam,
	#[id = "window_factor"]
	pub window_factor: FloatParam,

	#[persist = "map_code"]
	pub map_code: RwLock<Result<String, String>>,
	#[persist = "update_date"]
	pub date: RwLock<String>,
}

impl Default for Arguments {
	fn default() -> Self {
		fn default_daw_value(name: &str) -> FloatParam {
			FloatParam::new(name, 0.0, FloatRange::Linear{ 
				min: 0.0, 
				max: 1.0 
			}).with_value_to_string(Arc::new(|val| {
				format!("{:.2}", val)
			}))
		}

		Self {
			editor_state: EguiState::from_size(540, 270),
			a: default_daw_value("a"),
			b: default_daw_value("b"),
			c: default_daw_value("c"),
			d: default_daw_value("d"),

			gain: FloatParam::new("gain", 1.0, FloatRange::Linear{ 
				min: 0.0, 
				max: 4.0 
			}).with_value_to_string(Arc::new(|val| {
				let val = val as f64;
				if val.log10().is_nan() {
					String::from("-inf dB")
				}else {
					format!("{:.2} dB", 20.0 * val.log10())
				}
			})),

			window_size: IntParam::new("window_size", 11, IntRange::Linear {
				min: 7, 
				max: 12 
			}).with_value_to_string(Arc::new(|val| {
				format!("{}", 2_i32.pow(val as u32))
			})),
			window_offset: IntParam::new("window_offset", 0, IntRange::Linear {
				min: 0, 
				max: 4096 
			}),

			window_factor: FloatParam::new("window_factor", 0.5, FloatRange::Linear{ 
				min: 0.0, 
				max: 1.0 
			}).with_value_to_string(Arc::new(|val| {
				format!("{:.2}", val)
			})),

			map_code: RwLock::new(Ok(String::new())),
			date: Default::default(),
		}
	}
}

impl Plugin for Interface {
	const NAME: &'static str = "I Am Freq Remapper";
	const VENDOR: &'static str = "iamplugins";
	const URL: &'static str = "https://space.bilibili.com/237656277";
	const EMAIL: &'static str = "AnotherFuture@outlook.com";
	const VERSION: &'static str = env!("CARGO_PKG_VERSION");
	const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
		AudioIOLayout {
			main_input_channels: NonZeroU32::new(2),
			main_output_channels: NonZeroU32::new(2),
			..AudioIOLayout::const_default()
		},
		AudioIOLayout {
			main_input_channels: NonZeroU32::new(1),
			main_output_channels: NonZeroU32::new(1),
			..AudioIOLayout::const_default()
		},
	];

	type SysExMessage = ();
	type BackgroundTask = ();

	fn initialize(&mut self, _: &AudioIOLayout, config: &BufferConfig, ctx: &mut impl InitContext<Self>) -> bool {
		let sample_rate = config.sample_rate;
		self.processor[0] = Some(PhaseVocoder::new(WINDOW_SIZE, sample_rate));
		self.processor[1] = Some(PhaseVocoder::new(WINDOW_SIZE, sample_rate));
		ctx.set_latency_samples(WINDOW_SIZE as u32);
		true
	}

	fn params(&self) -> Arc<dyn Params> {
		self.params.clone()
	}

	fn process(&mut self, buf: &mut Buffer<'_>, _: &mut AuxiliaryBuffers<'_>, ctx: &mut impl ProcessContext<Self>) -> ProcessStatus {
		let mut map_code = self.params.map_code.write().unwrap();
		let mut result = Ok(());

		if let Ok(code) = &*map_code {
			for processor in &mut self.processor {
				if let Some(processor) = processor { result = processor.update_mapping(code) }
				if result.is_err() {
					break;
				}
			}
		}

		if let Err(e) = result {
			*map_code = Err(e); 
		}

		let daw_values = [
			self.params.a.value(),
			self.params.b.value(),
			self.params.c.value(),
			self.params.d.value(),
		];

		let gain = self.params.gain.value();
		let window_size = 2_usize.pow(self.params.window_size.value() as u32);
		let window_factor = self.params.window_factor.value();
		let window_offset = self.params.window_offset.value() as usize % window_size;

		ctx.set_latency_samples(window_size as u32);

		let transport = ctx.transport();
		let bpm = transport.tempo.unwrap_or(0.0) as f32;
		let sample_rate = transport.sample_rate;
		let daw_time = transport.pos_seconds().unwrap_or(0.0) as f32;
		let sys_time = (OffsetDateTime::now_utc() - self.instant).as_seconds_f32();

		for (i, samples) in buf.as_slice().iter_mut().enumerate() {
			let processor = &mut self.processor[i % 2];

			let input_params = InputParams {
				daw_values,
				// sustain_values: self.sustain_values[i % 2].read().map(|inner| *inner).unwrap_or_default(),
				current_track_id: i % 2,
				bpm,
				daw_time,
				sys_time,
				window_factor,
				window_offset,
				window_size,
				gain,
				sample_rate,
			};
			
			if let Some(processor) = processor {
				processor.process(samples, &input_params);
			};

		}
		ProcessStatus::Normal
	}

	fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
		let params = self.params.clone();
		create_egui_editor(params.editor_state.clone(), GuiInfo::default(), |_ctx, _| {
			#[cfg(feature = "zh_cn_support")]
			{
				let mut fonts = egui::FontDefinitions::default();
				fonts.font_data.insert("lnxh".to_string(), egui::FontData::from_static(FONT));
				fonts.families.get_mut(&egui::FontFamily::Proportional).unwrap().insert(0, "lnxh".to_string());
				fonts.families.get_mut(&egui::FontFamily::Monospace).unwrap().push("lnxh".to_string());
				_ctx.set_visuals(egui::Visuals {
					dark_mode: true,
					..Default::default()
				});
				_ctx.set_fonts(fonts);
			}
		}, move |ctx, setter, state| {
			egui::CentralPanel::default().show(ctx, |ui| {
				cfg_if::cfg_if! {
					if #[cfg(all(feature = "en_us", feature = "zh_cn"))] {
						
							egui::TopBottomPanel::top("language settings").show(ctx, |ui| {
								ui.allocate_space(Vec2::new(ui.available_width(), 4.0));
								ui.horizontal(|ui| {
									ui.selectable_value(&mut state.language, Language::ZhCn, "中文");
									ui.selectable_value(&mut state.language, Language::EnUs, "English");
								});
								ui.allocate_space(Vec2::new(ui.available_width(), 4.0));
							});
							match state.language {
								Language::ZhCn => zh_cn_ui(ui, setter, &params, state),
								Language::EnUs => en_us_ui(ui, setter, &params, state),
							}
					}else if #[cfg(feature = "zh_cn")] {
						zh_cn_ui(ui, setter, &params, state);
					}else if #[cfg(feature = "en_us")] {
						en_us_ui(ui, setter, &params, state);
					}
				}
			});
		})
	}
}

#[cfg(feature = "en_us")]
fn en_us_ui(
	ui: &mut egui::Ui, 
	setter: &ParamSetter<'_>, 
	params: &Arc<Arguments>,
	state: &mut GuiInfo,
) {
	egui::CentralPanel::default().show(ui.ctx(), |ui| {
		egui::SidePanel::right("right")
		.max_width(260.0)
		.default_width(260.0)
		.min_width(260.0)
		.show_inside(ui, |ui| { egui::ScrollArea::both().show(ui, |ui| {
			ui.allocate_space(Vec2::new(ui.available_width(), 0.0));
			ui.label("Params");
			ui.separator();
			ui.horizontal(|ui| {
				ui.label("a");
				ui.add(ParamSlider::for_param(&params.a, setter));
			});
			ui.horizontal(|ui| {
				ui.label("b");
				ui.add(ParamSlider::for_param(&params.b, setter));
			});
			ui.horizontal(|ui| {
				ui.label("c");
				ui.add(ParamSlider::for_param(&params.c, setter));
			});
			ui.horizontal(|ui| {
				ui.label("d");
				ui.add(ParamSlider::for_param(&params.d, setter));
			});
			ui.horizontal(|ui| {
				ui.label("out_gain");
				ui.add(ParamSlider::for_param(&params.gain, setter));
			});
			ui.horizontal(|ui| {
				ui.label("window_size");
				ui.add(ParamSlider::for_param(&params.window_size, setter));
			});
			ui.horizontal(|ui| {
				ui.label("window_factor");
				ui.add(ParamSlider::for_param(&params.window_factor, setter));
			});
			ui.horizontal(|ui| {
				ui.label("window_offset");
				ui.add(ParamSlider::for_param(&params.window_offset, setter));
			});
		})});

		egui::ScrollArea::both().show(ui, |ui| {
			ui.allocate_space(Vec2::new(ui.available_width(), 3.0));
			ui.label("Mapper Pannel");
			ui.separator();
			ui.label("Will read map script from `Documents/mapper.rhai`");
			ui.allocate_space(Vec2::new(0.0, 1.0));
			ui.horizontal(|ui| {
				if ui.button("Load").clicked() {
					if let Some(mut path) = dirs::document_dir() {
						path.push("mapper.rhai");
						let code = std::fs::read_to_string(path).map_err(|err| format!("{}", err));
						*params.map_code.write().unwrap() = code;
						*params.date.write().unwrap() = OffsetDateTime::now_utc().to_string();
					}
				}
				if ui.button("Clear (Double Click)").double_clicked() {
					*params.map_code.write().unwrap() = Ok(String::new());
					*params.date.write().unwrap() = OffsetDateTime::now_utc().to_string();
				}
				if ui.button("Show Code").clicked() {
					state.show_code = !state.show_code;
				}
			});
			ui.allocate_space(Vec2::new(0.0, 1.0));
			let code_info = params.map_code.read().unwrap();
			match &*code_info {
				Ok(inner) => {
					if inner.is_empty() {
						ui.label("Code Cleared!");
					}else {
						let date_info = params.date.read().unwrap();
						ui.label(format!("Loaded at: {}", date_info));
					}
				},
				Err(e) => {
					ui.label("Error!");
					ui.label(e);
				}
			}

			ui.allocate_space(Vec2::new(0.0, 8.0));
			ui.label("Code");
			ui.separator();

			if state.show_code{
				match &*code_info {
					Ok(inner) => {
						if inner.is_empty() {
							ui.label("Code Cleared!");
						}else {
							ui.label(inner);
						}
					},
					Err(e) => {
						ui.label("Error!");
						ui.label(e);
					}
				}
			}else {
				ui.label("collapsed");
			}
		});
	});
}

#[cfg(feature = "zh_cn")]
fn zh_cn_ui(
	ui: &mut egui::Ui,
	setter: &ParamSetter<'_>, 
	params: &Arc<Arguments>,
	state: &mut GuiInfo,
) {
	egui::CentralPanel::default().show(ui.ctx(), |ui| {
		egui::SidePanel::right("侧边栏")
		.max_width(270.0)
		.default_width(270.0)
		.min_width(270.0)
		.show_inside(ui, |ui| { egui::ScrollArea::both().show(ui, |ui| {
			ui.allocate_space(Vec2::new(ui.available_width(), 0.0));
			ui.label("参数");
			ui.separator();
			ui.horizontal(|ui| {
				ui.label("a");
				ui.add(ParamSlider::for_param(&params.a, setter));
			});
			ui.horizontal(|ui| {
				ui.label("b");
				ui.add(ParamSlider::for_param(&params.b, setter));
			});
			ui.horizontal(|ui| {
				ui.label("c");
				ui.add(ParamSlider::for_param(&params.c, setter));
			});
			ui.horizontal(|ui| {
				ui.label("d");
				ui.add(ParamSlider::for_param(&params.d, setter));
			});
			ui.horizontal(|ui| {
				ui.label("输出增益");
				ui.add(ParamSlider::for_param(&params.gain, setter));
			});
			ui.horizontal(|ui| {
				ui.label("FFT 窗长");
				ui.add(ParamSlider::for_param(&params.window_size, setter));
			});
			ui.horizontal(|ui| {
				ui.label("窗口参数");
				ui.add(ParamSlider::for_param(&params.window_factor, setter));
			});
			ui.horizontal(|ui| {
				ui.label("窗口延迟");
				ui.add(ParamSlider::for_param(&params.window_offset, setter));
			});
		})});

		egui::ScrollArea::both().show(ui, |ui| {
			ui.allocate_space(Vec2::new(ui.available_width(), 3.0));
			ui.label("映射器边栏");
			ui.separator();
			ui.label("将会从 `文档/mapper.rhai` 读取映射脚本");
			ui.allocate_space(Vec2::new(0.0, 1.0));
			ui.horizontal(|ui| {
				if ui.button("加载").clicked() {
					if let Some(mut path) = dirs::document_dir() {
						path.push("mapper.rhai");
						let code = std::fs::read_to_string(path).map_err(|err| format!("{}", err));
						*params.map_code.write().unwrap() = code;
						*params.date.write().unwrap() = OffsetDateTime::now_utc().to_string();
					}
				}
				if ui.button("清空 (双击)").double_clicked() {
					*params.map_code.write().unwrap() = Ok(String::new());
					*params.date.write().unwrap() = OffsetDateTime::now_utc().to_string();
				}
				if ui.button("展示代码").clicked() {
					state.show_code = !state.show_code;
				}
			});
			ui.allocate_space(Vec2::new(0.0, 1.0));
			let code_info = params.map_code.read().unwrap();
			match &*code_info {
				Ok(inner) => {
					if inner.is_empty() {
						ui.label("代码已清空!");
					}else {
						let date_info = params.date.read().unwrap();
						ui.label(format!("上次加载时间： {}", date_info));
					}
				},
				Err(e) => {
					ui.label("错误!");
					ui.label(e);
				}
			}

			ui.allocate_space(Vec2::new(0.0, 8.0));
			ui.label("代码展示");
			ui.separator();

			if state.show_code {
				match &*code_info {
					Ok(inner) => {
						if inner.is_empty() {
							ui.label("代码已清空!");
						}else {
							ui.label(inner);
						}
					},
					Err(e) => {
						ui.label("错误!");
						ui.label(e);
					}
				}
			}else {
				ui.label("已折叠");
			}
		});
	});
}

impl Vst3Plugin for Interface {
	const VST3_CLASS_ID: [u8; 16] = *b"IAmFreqRemapper_";
	const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[Vst3SubCategory::Fx, Vst3SubCategory::Tools];
}

nih_export_vst3!(Interface);