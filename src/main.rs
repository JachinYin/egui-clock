// use the sub command to hide bash window
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    env,
    path::Path,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use eframe::{
    egui::{
        self, CentralPanel, Color32, IconData, RichText, TextEdit, Vec2, ViewportBuilder, Visuals,
        Widget, WindowLevel,
    },
    Frame, HardwareAcceleration, Theme,
};

use kira::manager::{backend::cpal::CpalBackend, AudioManager, AudioManagerSettings};
use kira::sound::streaming::{StreamingSoundData, StreamingSoundHandle};
use kira::sound::{FromFileError, PlaybackState};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, PartialEq)]
enum Status {
    Running,
    Stop,
    Rest,
    RestRunning,
    #[default]
    Wait,
    RestWait,
}

fn main() -> Result<(), eframe::Error> {
    let mut auto_backup = Clock::default();

    auto_backup.init();

    let viewport = ViewportBuilder {
        title: None,
        app_id: Some("Clock".to_string()),
        position: None,
        inner_size: Some(Vec2::new(550.0, 450.0)),
        min_inner_size: Some(Vec2::new(140.0, 140.0)),
        max_inner_size: None,
        fullscreen: Some(false),
        maximized: Some(true),
        resizable: Some(true),
        transparent: Some(true),
        decorations: Some(true),
        icon: application_icon(),
        active: Some(true),
        visible: Some(true),
        fullsize_content_view: Some(true),
        title_shown: Some(true),
        titlebar_buttons_shown: Some(true),
        titlebar_shown: Some(true),
        drag_and_drop: Some(true),
        taskbar: Some(true),
        close_button: Some(true),
        minimize_button: Some(true),
        maximize_button: Some(true),
        window_level: Some(WindowLevel::AlwaysOnTop),
        mouse_passthrough: Some(false),
        window_type: ViewportBuilder::default().window_type,
    };

    let options = eframe::NativeOptions {
        viewport,
        vsync: true,
        multisampling: 0,
        depth_buffer: 0,
        stencil_buffer: 0,
        hardware_acceleration: HardwareAcceleration::Preferred, // 硬件加速
        renderer: eframe::Renderer::Glow,
        follow_system_theme: true, // 跟随系统主题
        default_theme: Theme::Dark,
        run_and_return: true, // 关闭窗口退出程序
        event_loop_builder: None,
        window_builder: None,
        shader_version: None,
        centered: true,
        persist_window: true,
    };

    eframe::run_native(
        "Clock",
        options,
        Box::new(|cc| {
            let mut alpha = 255;
            if auto_backup.setting.transparent >= 0.0 && auto_backup.setting.transparent < 1.0 {
                alpha = (auto_backup.setting.transparent * alpha as f32) as u8;
            }
            // println!("{}", alpha);
            let mut visuals = Visuals::dark();
            // make panels transparent
            visuals.panel_fill = Color32::from_rgba_premultiplied(
                visuals.panel_fill.r(),
                visuals.panel_fill.g(),
                visuals.panel_fill.b(),
                alpha,
            );
            cc.egui_ctx.set_visuals(visuals);
            // cc.egui_ctx
            //     .set_pixels_per_point(cc.egui_ctx.native_pixels_per_point().unwrap_or(1.0) * 1.2);

            // This gives us image support:
            egui_extras::install_image_loaders(&cc.egui_ctx);
            load_fonts(&cc.egui_ctx);
            // 控制缩放
            cc.egui_ctx.set_pixels_per_point(2.0);

            Box::new(auto_backup)
        }),
    )
}

#[derive(Default)]
struct Clock {
    // default_secs: String,
    // default_rest_secs: String,
    countdown: Arc<Mutex<usize>>,
    status: Arc<Mutex<Status>>,

    audio: Audio,
    setting: Setting,
}

impl Clock {
    pub fn init(&mut self) {
        // self.min = Arc::new(Mutex::new(0));

        let min_arc = self.countdown.clone();
        let status_arc = self.status.clone();

        thread::spawn(move || loop {
            if let Ok(mut min) = min_arc.try_lock() {
                if *min > 0 {
                    if let Ok(status) = status_arc.try_lock() {
                        if *status == Status::Running || *status == Status::RestRunning {
                            *min -= 1;
                        }
                    }
                } else {
                    if let Ok(mut status) = status_arc.try_lock() {
                        if *status == Status::Running {
                            *status = Status::Rest;
                        } else if *status == Status::RestRunning {
                            *status = Status::RestWait;
                        } else {
                            *status = Status::Wait;
                        }
                    }
                }
            }
            thread::sleep(Duration::from_secs(1));
        });
    }

    pub fn start(&mut self) {
        if let Ok(mut min) = self.countdown.try_lock() {
            *min = self.setting.run_secs;
        }
        if let Ok(mut status) = self.status.try_lock() {
            *status = Status::Running;
        }
    }

    pub fn check_status(&mut self) {
        let mut auto_next = false;
        if let Ok(mut status) = self.status.try_lock() {
            if *status == Status::Rest {
                if let Ok(mut min) = self.countdown.try_lock() {
                    *min = self.setting.rest_secs;
                    *status = Status::RestRunning;
                }
            }

            auto_next = *status == Status::RestWait && self.setting.auto_next;
        }
        if auto_next {
            self.start();
        }
    }

    pub fn voice_broadcast(&mut self) {
        if let Ok(status) = self.status.try_lock() {
            if let Ok(sec) = self.countdown.try_lock() {
                if *status == Status::Running {
                    match *sec {
                        90 => self
                            .audio
                            .start_play(&format!("{}/assets/audio/90.mp3", current_dir())),
                        60 => self
                            .audio
                            .start_play(&format!("{}/assets/audio/60.mp3", current_dir())),
                        30 => self
                            .audio
                            .start_play(&format!("{}/assets/audio/30.mp3", current_dir())),
                        10 => self
                            .audio
                            .start_play(&format!("{}/assets/audio/10.mp3", current_dir())),
                        5 => self
                            .audio
                            .start_play(&format!("{}/assets/audio/05.mp3", current_dir())),
                        0 => self
                            .audio
                            .start_play(&format!("{}/assets/audio/rest.mp3", current_dir())),
                        _ => {}
                    }
                }
                if *status == Status::RestRunning && *sec == 0 {
                    self.audio
                        .start_play(&format!("{}/assets/audio/next.mp3", current_dir()));
                }
            }
        }
    }
}

impl eframe::App for Clock {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        self.check_status();
        self.voice_broadcast();

        CentralPanel::default().show(ctx, |ui| {
            // 控制面板
            ui.horizontal(|ui| {
                // let mut auto_next = self.setting.auto_next;
                if ui
                    .checkbox(&mut self.setting.auto_next, "")
                    .on_hover_text("自动开启下一轮")
                    .changed()
                {
                    self.setting.save();
                }

                if ui.button("开始").clicked() {
                    self.start();
                }

                if let Ok(mut status) = self.status.try_lock() {
                    if *status == Status::Running {
                        if ui.button("暂停").clicked() {
                            *status = Status::Stop;
                        }
                    } else if *status == Status::Stop {
                        if ui.button("继续").clicked() {
                            *status = Status::Running;
                        }
                    }
                }
            });

            // 输入框
            ui.horizontal(|ui| {
                ui.label("时间");
                let mut run_secs = self.setting.run_secs.to_string();
                if TextEdit::singleline(&mut run_secs)
                    .desired_width(80.0)
                    .ui(ui)
                    .changed()
                {
                    if run_secs.trim().is_empty() {
                        run_secs = String::from("0");
                    }
                    if let Ok(num) = run_secs.parse() {
                        self.setting.run_secs = num;
                        self.setting.save();
                    }
                }

                ui.label("休息");
                let mut rest_secs = self.setting.rest_secs.to_string();
                if TextEdit::singleline(&mut rest_secs)
                    .desired_width(80.0)
                    .ui(ui)
                    .changed()
                {
                    if rest_secs.trim().is_empty() {
                        rest_secs = String::from("0");
                    }
                    if let Ok(num) = rest_secs.parse() {
                        self.setting.rest_secs = num;
                        self.setting.save();
                    }
                }
            });

            // 大屏展示
            ui.centered_and_justified(|ui| {
                if let Ok(min) = self.countdown.try_lock() {
                    let mut rich_text = RichText::new(min.to_string()).size(self.setting.font_size);
                    if let Ok(status) = self.status.try_lock() {
                        if *status == Status::RestRunning {
                            rich_text = rich_text.color(Color32::DARK_GREEN);
                        } else if *status == Status::Running && *min <= 5 {
                            rich_text = rich_text.color(Color32::DARK_RED);
                        }
                    }
                    ui.label(rich_text);
                }
            });
        });

        // 定时刷新页面
        if let Ok(status) = self.status.try_lock() {
            if *status == Status::Running || *status == Status::RestRunning {
                ctx.request_repaint_after(Duration::from_millis(10));
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Setting {
    run_secs: usize,
    rest_secs: usize,
    auto_next: bool,
    font_size: f32,
    transparent: f32,
}
impl Setting {
    pub fn new() -> Self {
        Self {
            run_secs: 45,
            rest_secs: 30,
            auto_next: false,
            font_size: 50.0,
            transparent: 1.0,
        }
    }

    pub fn save(&self) {
        if let Ok(data) = serde_json::to_string(self) {
            let _ = Self::write_data(&format!("{}/data/config.json", current_dir()), data);
        }
    }

    /// 文件是否存在 可以判断 路径是否存在，文件、文件夹都可以
    pub fn file_exist(path: &str) -> bool {
        Path::new(path).exists()
    }

    /// read data from file
    pub fn read_data(path: &str) -> Result<String, String> {
        if !Self::file_exist(&path) {
            return Err(format!("file not exist: {}", &path));
        }
        match std::fs::read_to_string(&path) {
            Err(err) => {
                let msg = format!("read file {} error: {}", &path, err);
                // log::log_err(msg.to_string());
                Err(msg)
            }
            Ok(data) => Ok(data),
        }
    }

    /// save data to file
    pub fn write_data(path: &str, data: String) -> Result<(), String> {
        match std::fs::write(&path, &data) {
            Err(err) => {
                let msg = format!("write data error {}; path:{}, data:{}", err, &path, &data);
                // log::log_err(msg.to_string());
                Err(msg)
            }
            _ => Ok(()),
        }
    }
}
impl Default for Setting {
    fn default() -> Self {
        let dir = format!("{}/data/", current_dir());
        let path = format!("{}/data/config.json", current_dir());

        if !Self::file_exist(&dir) {
            let _ = std::fs::create_dir_all(dir);
        }

        if let Ok(data) = Self::read_data(&path) {
            if let Ok(value) = serde_json::from_str::<Setting>(&data) {
                value
            } else {
                Self::new()
            }
        } else {
            Self::new()
        }
    }
}

// 语音播报
struct Audio {
    manager: AudioManager,
    sound_handle: Option<StreamingSoundHandle<FromFileError>>,
}

impl Default for Audio {
    fn default() -> Self {
        let manager = AudioManager::<CpalBackend>::new(AudioManagerSettings::default()).unwrap();
        Audio {
            manager,
            sound_handle: None,
        }
    }
}

impl Audio {
    pub fn start_play(&mut self, path: &str) {
        if let Some(sound_handle) = &self.sound_handle {
            if sound_handle.state() == PlaybackState::Playing {
                return;
            }
        }

        if let Ok(sound_data) = StreamingSoundData::from_file(path) {
            // self.sound_data = Some(sound_data);
            let play = self.manager.play(sound_data).unwrap();
            self.sound_handle = Some(play);
        }
    }
}

// 处理应用使用的字体
pub fn load_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "my_font".to_owned(),
        egui::FontData::from_static(include_bytes!("../assets/myfont.ttf")),
    );
    fonts
        .families
        .get_mut(&egui::FontFamily::Proportional)
        .unwrap()
        .insert(0, "my_font".to_owned());
    fonts
        .families
        .get_mut(&egui::FontFamily::Monospace)
        .unwrap()
        .push("my_font".to_owned());

    ctx.set_fonts(fonts);
}

// 处理应用图标
pub fn application_icon() -> Option<Arc<IconData>> {
    let icon_data = include_bytes!("../assets/icon.png");
    let img = image::load_from_memory_with_format(icon_data, image::ImageFormat::Png).unwrap();
    let rgba_data = img.into_rgba8();
    let (width, height) = (rgba_data.width(), rgba_data.height());
    let rgba: Vec<u8> = rgba_data.into_raw();
    Some(Arc::<IconData>::new(IconData {
        rgba,
        width,
        height,
    }))
}

// 获取当前程序运行路径
pub fn current_dir() -> String {
    match env::current_dir() {
        Ok(path) => path.display().to_string(),
        Err(_) => ".".to_string(),
    }
}
