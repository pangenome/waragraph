use crossbeam::atomic::AtomicCell;
use raving_wgpu::{gui::EguiCtx, WindowState};
use tokio::{
    runtime::Runtime,
    sync::{mpsc, RwLock},
};
use waragraph_core::graph::{Bp, PathId};
use winit::{
    event::{ElementState, Event, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop, EventLoopWindowTarget},
    window::WindowId,
};

use std::{collections::HashMap, path::PathBuf, sync::Arc};

use anyhow::Result;

use crate::{
    annotations::{AnnotationSet, AnnotationStore},
    color::{ColorSchemeId, ColorStore},
    context::{widget::ContextInspector, ContextState},
    viewer_1d::Viewer1D,
    viewer_2d::Viewer2D,
};

mod window;

pub mod settings_menu;

pub mod workspace;

pub mod resource;

pub use window::AppWindowState;

use self::{
    resource::{AnyArcMap, GraphDataCache},
    settings_menu::SettingsWindow,
    window::{toggle_fullscreen, AppWindows, WindowDelta, WindowOptions},
    workspace::Workspace,
};

#[derive(Clone)]
pub struct SharedState {
    pub graph: Arc<waragraph_core::graph::PathIndex>,

    // pub shared: Arc<RwLock<AnyArcMap>>,
    pub graph_data_cache: Arc<GraphDataCache>,

    pub annotations: Arc<RwLock<AnnotationStore>>,

    pub colors: Arc<RwLock<ColorStore>>,

    pub workspace: Arc<RwLock<Workspace>>,
    // gfa_path: Arc<PathBuf>,
    // tsv_path: Option<Arc<RwLock<PathBuf>>>,
    pub data_color_schemes: Arc<RwLock<HashMap<String, ColorSchemeId>>>,

    pub initial_1d_view_mode: String,

    pub app_msg_send: tokio::sync::mpsc::Sender<AppMsg>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AppType {
    Viewer1D,
    Viewer2D,
    // MainMenu,
    Custom(String),
}

pub struct App {
    pub tokio_rt: Arc<Runtime>,
    pub shared: SharedState,

    context_state: ContextState,

    context_inspector: ContextInspector,

    app_windows: AppWindows,
    window_options: WindowOptions,
    // pub windows: HashMap<WindowId, AppType>,
    // pub apps: HashMap<AppType, AppWindowState>,

    // sleeping: HashMap<AppType, AsleepWindow>,
    settings: SettingsWindow,
    settings_window_tgt: Option<WindowId>,

    app_msg_recv: tokio::sync::mpsc::Receiver<AppMsg>,
}

impl App {
    pub fn init(state: &raving_wgpu::State, args: Args) -> Result<Self> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(4)
            .enable_all()
            .thread_name("waragraph-tokio")
            .build()?;

        let tokio_rt = Arc::new(runtime);

        let path_index = waragraph_core::graph::PathIndex::from_gfa(&args.gfa)?;
        let path_index = Arc::new(path_index);

        let (app_msg_send, app_msg_recv) = mpsc::channel::<AppMsg>(256);

        let mut settings = SettingsWindow::new(
            tokio_rt.handle().clone(),
            app_msg_send.clone(),
        );

        let app_windows = AppWindows::default();

        settings.register_widget(
            "Window",
            "Windows",
            app_windows.widget_state.clone(),
        );

        let shared = {
            let workspace = Arc::new(RwLock::new(Workspace {
                gfa_path: args.gfa,
                tsv_path: args.tsv,
            }));

            {
                let ws = workspace.clone();
                settings.register_widget("General", "Graph & Layout", ws);
            }

            let graph_data_cache = Arc::new(GraphDataCache::init(&path_index));

            let colors = Arc::new(RwLock::new(ColorStore::init(state)));

            let mut data_color_schemes = HashMap::default();

            {
                let mut colors = colors.blocking_write();

                let mut add_entry = |data: &str, color: &str| {
                    let scheme = colors.get_color_scheme_id(color).unwrap();

                    colors.create_color_scheme_texture(state, color);

                    data_color_schemes.insert(data.into(), scheme);
                };

                add_entry("depth", "spectral");
                add_entry("strand", "black_red");
            }

            let mut annotations = AnnotationStore::default();

            for annot_path in args.annotations.iter() {
                if let Some(ext) = annot_path.extension() {
                    let result = if ext == "bed" {
                        AnnotationSet::from_bed(
                            &path_index,
                            None,
                            |name| name.to_string(),
                            annot_path,
                        )
                    } else if ext == "gff" {
                        let attr = args
                            .gff_attr
                            .as_ref()
                            .map(|s| s.as_str())
                            .unwrap_or("Name");

                        // TODO the name and record functions should be configurable
                        AnnotationSet::from_gff(
                            &path_index,
                            None,
                            |name| name.to_string(),
                            // |name| format!("S288C.{name}"),
                            // |name| format!("SGDref#1#{name}"),
                            |record| {
                                let attrs = record.attributes();
                                let label = attrs.iter().find_map(|entry| {
                                    (entry.key() == attr)
                                        .then_some(entry.value())
                                })?;

                                Some(label.to_string())
                            },
                            annot_path,
                        )
                    } else {
                        log::error!("Unknown annotation file extension `{ext:?}`, ignoring");
                        continue;
                    };

                    match result {
                        Ok(set) => {
                            log::warn!(
                                "loaded annotation set with {} annotations",
                                set.annotations.len()
                            );

                            annotations.insert_set(set);
                        }
                        Err(e) => {
                            log::error!(
                                "Error loading annotation file {:?}: {e:?}",
                                annot_path.as_os_str()
                            );
                        }
                    }
                }
            }

            let annotations: Arc<RwLock<AnnotationStore>> =
                Arc::new(RwLock::new(annotations));

            SharedState {
                graph: path_index,

                // shared: Arc::new(RwLock::new(AnyArcMap::default())),
                graph_data_cache,
                annotations,

                colors,

                data_color_schemes: Arc::new(data_color_schemes.into()),

                workspace,

                initial_1d_view_mode: args.initial_1d_view_mode,

                app_msg_send,
            }
        };

        let context_state = ContextState::default();

        let context_inspector = ContextInspector::with_default_widgets(&shared);

        settings.register_widget(
            "Context",
            "Context Inspector",
            context_inspector.settings_widget().clone(),
        );

        Ok(Self {
            tokio_rt,
            shared,

            context_state,
            context_inspector,

            app_windows,
            window_options: args.window_options,
            // windows: HashMap::default(),
            // apps: HashMap::default(),

            // sleeping: HashMap::default(),
            settings,
            settings_window_tgt: None,

            app_msg_recv,
        })
    }

    pub fn init_custom_window(
        &mut self,
        event_loop: &EventLoop<()>,
        state: &raving_wgpu::State,
        id: &str,
        title: Option<&str>,
        constructor: impl FnOnce(&WindowState) -> anyhow::Result<Box<dyn AppWindow>>,
    ) -> Result<()> {
        let id = id.to_string();
        let title = title.map(|s| s.to_string()).unwrap_or(id.clone());
        let app_id = AppType::Custom(id);

        let app = AppWindowState::init(
            event_loop,
            state,
            self.window_options,
            &title,
            constructor,
        )?;

        let winid = app.window.window.id();

        self.app_windows.apps.insert(app_id.clone(), app);
        self.app_windows.windows.insert(winid, app_id);

        Ok(())
    }

    pub fn init_viewer_1d(
        &mut self,
        event_loop: &EventLoopWindowTarget<()>,
        state: &raving_wgpu::State,
    ) -> Result<()> {
        let title = "Waragraph 1D";

        let app = AppWindowState::init(
            event_loop,
            state,
            self.window_options,
            title,
            |window| {
                let dims: [u32; 2] = window.size.into();

                let app = Viewer1D::init(
                    dims,
                    state,
                    &window,
                    self.shared.graph.clone(),
                    &self.shared,
                    &mut self.settings,
                )?;

                Ok(Box::new(app))
            },
        )?;

        let winid = app.window.window.id();

        self.app_windows.apps.insert(AppType::Viewer1D, app);
        self.app_windows.windows.insert(winid, AppType::Viewer1D);

        Ok(())
    }

    pub fn init_viewer_2d(
        &mut self,
        event_loop: &EventLoopWindowTarget<()>,
        state: &raving_wgpu::State,
    ) -> Result<()> {
        let tsv = if let Some(tsv) =
            self.shared.workspace.blocking_read().tsv_path().cloned()
        {
            tsv
        } else {
            anyhow::bail!("Can't initialize 2D viewer without layout TSV");
        };

        let title = "Waragraph 2D";

        let app = AppWindowState::init(
            event_loop,
            state,
            self.window_options,
            title,
            |window| {
                let app = Viewer2D::init(
                    state,
                    &window,
                    self.shared.graph.clone(),
                    tsv,
                    &self.shared,
                    &mut self.settings,
                )?;

                Ok(Box::new(app))
            },
        )?;

        let winid = app.window.window.id();

        self.app_windows.apps.insert(AppType::Viewer2D, app);
        self.app_windows.windows.insert(winid, AppType::Viewer2D);

        Ok(())
    }

    pub fn run(
        mut self,
        event_loop: EventLoop<()>,
        state: raving_wgpu::State,
    ) -> Result<()> {
        let mut is_ready = false;
        let mut prev_frame_t = std::time::Instant::now();

        self.app_windows.update_widget_state();

        {
            // upload color buffers -- should obviously be handled better,
            // rather than just once at the start!
            let mut colors = self.shared.colors.blocking_write();
            colors.upload_color_schemes_to_gpu(&state)?;
        }

        event_loop.run(
            move |event, event_loop_tgt, control_flow| match &event {
                Event::Resumed => {
                    if !is_ready {
                        is_ready = true;
                    }
                }
                Event::WindowEvent { window_id, event } => {
                    let app_type = self.app_windows.windows.get(&window_id);
                    if app_type.is_none() {
                        return;
                    }
                    let app_type = app_type.unwrap();
                    let app = self.app_windows.apps.get_mut(app_type).unwrap();

                    if is_fullscreen_shortcut(event) {
                        toggle_fullscreen(&app.window);
                        return;
                    }

                    let consumed = app.on_event(event);

                    if !consumed {
                        match &event {
                            WindowEvent::KeyboardInput { input, .. } => {
                                use VirtualKeyCode as Key;

                                let pressed = matches!(
                                    input.state,
                                    ElementState::Pressed
                                );

                                if let Some(Key::Escape) = input.virtual_keycode
                                {
                                    if pressed {
                                        if let Err(e) =
                                            self.shared.app_msg_send.try_send(
                                                AppMsg::ToggleSettingsWindow {
                                                    src: *window_id,
                                                },
                                            )
                                        {
                                            log::error!("{e:?}");
                                        }
                                    }
                                }
                            }
                            WindowEvent::CloseRequested => {
                                *control_flow = ControlFlow::Exit
                            }
                            WindowEvent::Resized(_phys_size) => {
                                if is_ready {
                                    let old_size = app.window.size;
                                    app.resize(&state);
                                    app.app
                                        .on_resize(
                                            &state,
                                            old_size.into(),
                                            app.window.size.into(),
                                        )
                                        .unwrap();
                                }
                            }
                            WindowEvent::ScaleFactorChanged {
                                new_inner_size: _,
                                ..
                            } => {
                                if is_ready {
                                    let old_size = app.window.size;
                                    app.resize(&state);
                                    app.app
                                        .on_resize(
                                            &state,
                                            old_size.into(),
                                            app.window.size.into(),
                                        )
                                        .unwrap();
                                }
                            }
                            _ => {}
                        }
                    }
                }

                Event::RedrawRequested(window_id) => {
                    let app_type = self.app_windows.windows.get(&window_id);
                    if app_type.is_none() {
                        return;
                    }
                    let app_type = app_type.unwrap();

                    let app = self.app_windows.apps.get_mut(app_type).unwrap();
                    app.render(&state).unwrap();
                }
                Event::MainEventsCleared => {
                    let dt = prev_frame_t.elapsed().as_secs_f32();
                    prev_frame_t = std::time::Instant::now();

                    self.context_state.start_frame();

                    while let Ok(msg) = self.app_msg_recv.try_recv() {
                        if let Err(e) =
                            self.process_msg(event_loop_tgt, &state, msg)
                        {
                            log::error!("Error processing AppMsg: {e:?}");
                        }
                    }

                    // TODO: don't really like just having this here,
                    // but good enough for now
                    self.app_windows.update_widget_state();

                    let context_inspector_tgts =
                        self.context_inspector.active_targets();

                    for (app_type, app) in self.app_windows.apps.iter_mut() {
                        app.update(
                            self.tokio_rt.handle(),
                            &state,
                            &mut self.context_state,
                            dt,
                        );

                        if Some(app.window.window.id())
                            == self.settings_window_tgt
                        {
                            self.settings.show(app.egui.ctx());
                        }

                        if context_inspector_tgts.contains(app_type) {
                            egui::Window::new("Context Inspector")
                                .default_pos([100.0, 100.0])
                                .show(app.egui.ctx(), |ui| {
                                    self.context_inspector
                                        .show(&self.context_state, ui);
                                });
                        }

                        app.window.window.request_redraw();
                    }
                }

                _ => {}
            },
        )
    }
}

impl App {
    fn process_msg(
        &mut self,
        event_loop: &EventLoopWindowTarget<()>,
        state: &raving_wgpu::State,
        msg: AppMsg,
    ) -> Result<()> {
        match msg {
            AppMsg::InitViewer1D => {
                if !self.app_windows.apps.contains_key(&AppType::Viewer1D) {
                    // todo
                }
            }
            AppMsg::InitViewer2D => {
                if !self.app_windows.apps.contains_key(&AppType::Viewer2D) {
                    if let Err(e) = self.init_viewer_2d(event_loop, state) {
                        log::error!("Error initializing 2D viewer");
                    }
                }
            }
            AppMsg::OpenSettingsWindow { src } => {
                if self.settings_window_tgt.is_none() {
                    self.settings_window_tgt = Some(src);
                }
            }
            AppMsg::ToggleSettingsWindow { src } => {
                if let Some(tgt) = self.settings_window_tgt.take() {
                    if src != tgt {
                        self.settings_window_tgt = Some(src);
                    }
                } else {
                    self.settings_window_tgt = Some(src);
                }
            }
            AppMsg::WindowDelta(delta) => {
                self.app_windows.handle_window_delta(
                    event_loop,
                    state,
                    self.window_options,
                    delta,
                )?;
            }
        }

        Ok(())
    }
}

pub trait AppWindow {
    fn update(
        &mut self,
        tokio_handle: &tokio::runtime::Handle,
        state: &raving_wgpu::State,
        window: &raving_wgpu::WindowState,
        egui_ctx: &mut EguiCtx,
        context_state: &mut ContextState,
        dt: f32,
    );

    fn on_event(
        &mut self,
        window_dims: [u32; 2],
        event: &winit::event::WindowEvent,
    ) -> bool;

    fn on_resize(
        &mut self,
        state: &raving_wgpu::State,
        old_window_dims: [u32; 2],
        new_window_dims: [u32; 2],
    ) -> anyhow::Result<()>;

    fn render(
        &mut self,
        state: &raving_wgpu::State,
        window: &WindowState,
        swapchain_view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
    ) -> anyhow::Result<()>;
}

#[derive(Debug)]
pub struct Args {
    pub gfa: PathBuf,
    pub tsv: Option<PathBuf>,

    pub annotations: Vec<PathBuf>,
    pub gff_attr: Option<String>,
    pub initial_1d_view_mode: String,
    pub window_options: WindowOptions,
    // pub annotations: Option<PathBuf>,
}

pub fn parse_args() -> std::result::Result<Args, pico_args::Error> {
    let mut pargs = pico_args::Arguments::from_env();

    // let init_range = pargs.opt_value_from_fn("--range", parse_range)?;

    let mut annotations = Vec::new();

    let bed = pargs.opt_value_from_os_str("--bed", parse_path)?;
    if let Some(bed) = bed {
        annotations.push(bed);
    }

    let gff = pargs.opt_value_from_os_str("--gff", parse_path)?;
    if let Some(gff) = gff {
        annotations.push(gff);
    }

    let gff_attr = pargs.opt_value_from_str("--gff-attr")?;
    let initial_1d_view_mode = pargs
        .opt_value_from_fn("--view-mode", parse_1d_view_mode)?
        .unwrap_or_else(|| "depth".to_string());
    let window_options = WindowOptions {
        borderless: pargs.contains("--borderless"),
        fullscreen: pargs.contains("--fullscreen"),
        maximized: pargs.contains("--maximized"),
    };

    let args = Args {
        gfa: pargs.free_from_os_str(parse_path)?,
        tsv: pargs.opt_free_from_os_str(parse_path)?,

        annotations,
        gff_attr,
        initial_1d_view_mode,
        window_options,
        // init_range,
    };

    Ok(args)
}

pub fn configure_default_window_backend() {
    if default_window_backend_for_environment(
        std::env::var_os("WAYLAND_DISPLAY"),
        std::env::var_os("DISPLAY"),
        std::env::var_os("WINIT_UNIX_BACKEND"),
    )
    .as_deref()
        == Some("x11")
    {
        std::env::set_var("WINIT_UNIX_BACKEND", "x11");
    }
}

fn default_window_backend_for_environment(
    wayland_display: Option<std::ffi::OsString>,
    x11_display: Option<std::ffi::OsString>,
    winit_unix_backend: Option<std::ffi::OsString>,
) -> Option<String> {
    let has_wayland_display = wayland_display
        .as_deref()
        .map(|value| !value.is_empty())
        .unwrap_or(false);
    let has_x11_display = x11_display
        .as_deref()
        .map(|value| !value.is_empty())
        .unwrap_or(false);
    let backend_is_explicit = winit_unix_backend
        .as_deref()
        .map(|value| !value.is_empty())
        .unwrap_or(false);

    if has_wayland_display && has_x11_display && !backend_is_explicit {
        Some("x11".to_string())
    } else {
        None
    }
}

fn parse_path(s: &std::ffi::OsStr) -> Result<std::path::PathBuf, &'static str> {
    Ok(s.into())
}

fn parse_1d_view_mode(s: &str) -> Result<String, &'static str> {
    match s {
        "depth" | "path-name" | "path_name" | "strand" => {
            Ok(s.replace('-', "_"))
        }
        _ => Err("view mode must be one of: depth, path-name, strand"),
    }
}

fn is_fullscreen_shortcut(event: &WindowEvent<'_>) -> bool {
    if let WindowEvent::KeyboardInput { input, .. } = event {
        is_fullscreen_key_input(input)
    } else {
        false
    }
}

fn is_fullscreen_key_input(input: &winit::event::KeyboardInput) -> bool {
    matches!(input.state, ElementState::Pressed)
        && matches!(input.virtual_keycode, Some(VirtualKeyCode::F11))
}

#[cfg(test)]
mod tests {
    use super::{
        default_window_backend_for_environment, is_fullscreen_key_input,
        parse_1d_view_mode,
    };
    use winit::event::{ElementState, KeyboardInput, VirtualKeyCode};

    #[test]
    fn parse_1d_view_mode_defaults_and_aliases() {
        assert_eq!(parse_1d_view_mode("depth").unwrap(), "depth");
        assert_eq!(parse_1d_view_mode("path-name").unwrap(), "path_name");
        assert_eq!(parse_1d_view_mode("path_name").unwrap(), "path_name");
        assert_eq!(parse_1d_view_mode("strand").unwrap(), "strand");
        assert!(parse_1d_view_mode("unknown").is_err());
    }

    #[test]
    #[allow(deprecated)]
    fn f11_press_is_fullscreen_shortcut() {
        let input = KeyboardInput {
            scancode: 0,
            state: ElementState::Pressed,
            virtual_keycode: Some(VirtualKeyCode::F11),
            modifiers: Default::default(),
        };

        assert!(is_fullscreen_key_input(&input));
    }

    #[test]
    #[allow(deprecated)]
    fn f11_release_is_not_fullscreen_shortcut() {
        let input = KeyboardInput {
            scancode: 0,
            state: ElementState::Released,
            virtual_keycode: Some(VirtualKeyCode::F11),
            modifiers: Default::default(),
        };

        assert!(!is_fullscreen_key_input(&input));
    }

    #[test]
    fn defaults_wayland_session_to_x11_when_xwayland_is_available() {
        assert_eq!(
            default_window_backend_for_environment(
                Some("wayland-0".into()),
                Some(":0".into()),
                None
            ),
            Some("x11".to_string())
        );
    }

    #[test]
    fn keeps_explicit_winit_backend() {
        assert_eq!(
            default_window_backend_for_environment(
                Some("wayland-0".into()),
                Some(":0".into()),
                Some("wayland".into())
            ),
            None
        );
    }

    #[test]
    fn does_not_select_x11_without_x_display() {
        assert_eq!(
            default_window_backend_for_environment(
                Some("wayland-0".into()),
                None,
                None
            ),
            None
        );
    }
}

#[derive(Debug, Clone)]
pub enum AppMsg {
    InitViewer1D,
    InitViewer2D,
    OpenSettingsWindow { src: WindowId },
    ToggleSettingsWindow { src: WindowId },
    WindowDelta(WindowDelta),
}
