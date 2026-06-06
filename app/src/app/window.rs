use std::collections::HashMap;
use std::sync::Arc;

use raving_wgpu::{gui::EguiCtx, WindowState};
use tokio::sync::RwLock;
use winit::{
    dpi::PhysicalSize,
    event::WindowEvent,
    event_loop::EventLoopWindowTarget,
    window::{Fullscreen, WindowBuilder, WindowId},
};

use crate::context::ContextState;

use super::{
    settings_menu::{SettingsUiResponse, SettingsWidget},
    AppMsg, AppType, AppWindow,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WindowOptions {
    pub borderless: bool,
    pub fullscreen: bool,
    pub maximized: bool,
}

pub struct AppWindowState {
    pub title: String,
    pub(super) window: WindowState,
    pub(super) app: Box<dyn AppWindow>,
    pub(super) egui: EguiCtx,
}

impl AppWindowState {
    pub(super) fn sleep(self) -> AsleepWindow {
        AsleepWindow {
            title: self.title,
            app: self.app,
            egui: self.egui,
        }
    }

    pub(super) fn init(
        event_loop: &EventLoopWindowTarget<()>,
        state: &raving_wgpu::State,
        options: WindowOptions,
        title: &str,
        constructor: impl FnOnce(&WindowState) -> anyhow::Result<Box<dyn AppWindow>>,
    ) -> anyhow::Result<Self> {
        let window = app_window_builder(title, options).build(event_loop)?;

        let mut win_state = state.prepare_window(window)?;
        resize_window_surface(&mut win_state, &state.device);

        let egui_ctx =
            EguiCtx::init(&state, win_state.surface_format, &event_loop, None);

        let app = constructor(&win_state)?;

        Ok(Self {
            title: title.to_string(),
            window: win_state,
            app,
            egui: egui_ctx,
        })
    }

    pub(super) fn resize(&mut self, state: &raving_wgpu::State) {
        resize_window_surface(&mut self.window, &state.device);
    }

    pub(super) fn on_event<'a>(&mut self, event: &WindowEvent<'a>) -> bool {
        let resp = self.egui.on_event(event);
        let mut consumed = resp.consumed;
        if !consumed {
            consumed = self
                .app
                .on_event(self.window.window.inner_size().into(), event);
        }
        consumed
    }

    pub(super) fn update(
        &mut self,
        tokio_handle: &tokio::runtime::Handle,
        state: &raving_wgpu::State,
        context_state: &mut ContextState,
        dt: f32,
    ) {
        self.app.update(
            tokio_handle,
            state,
            &self.window,
            &mut self.egui,
            context_state,
            dt,
        );
    }

    pub(super) fn render(
        &mut self,
        state: &raving_wgpu::State,
    ) -> anyhow::Result<()> {
        let app = &mut self.app;
        let egui_ctx = &mut self.egui;
        let window = &mut self.window;

        if let Ok(output) = window.surface.get_current_texture() {
            let mut encoder = state.device.create_command_encoder(
                &wgpu::CommandEncoderDescriptor {
                    label: Some(&self.title),
                },
            );

            let output_view = output
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());

            let result = app.render(state, window, &output_view, &mut encoder);
            if let Err(e) = result {
                log::error!("Render error in window {}: {e:?}", &self.title);
            }
            egui_ctx.render(state, window, &output_view, &mut encoder);

            state.queue.submit(Some(encoder.finish()));
            output.present();
        } else {
            resize_window_surface(window, &state.device);
        }

        Ok(())
    }
}

pub struct AsleepWindow {
    pub title: String,
    pub(super) app: Box<dyn AppWindow>,
    pub(super) egui: EguiCtx,
}

impl AsleepWindow {
    pub(super) fn wake(
        self,
        event_loop: &EventLoopWindowTarget<()>,
        state: &raving_wgpu::State,
        options: WindowOptions,
    ) -> anyhow::Result<AppWindowState> {
        let window =
            app_window_builder(&self.title, options).build(event_loop)?;

        let mut win_state = state.prepare_window(window)?;
        resize_window_surface(&mut win_state, &state.device);

        Ok(AppWindowState {
            title: self.title,
            window: win_state,
            app: self.app,
            egui: self.egui,
        })
    }
}

fn app_window_builder(title: &str, options: WindowOptions) -> WindowBuilder {
    WindowBuilder::new()
        .with_title(title)
        .with_decorations(!options.borderless)
        .with_maximized(options.maximized)
        .with_fullscreen(
            options.fullscreen.then_some(Fullscreen::Borderless(None)),
        )
}

pub(super) fn toggle_fullscreen(window_state: &WindowState) {
    let window = &window_state.window;
    let fullscreen = if window.fullscreen().is_some() {
        None
    } else {
        Some(Fullscreen::Borderless(None))
    };
    window.set_fullscreen(fullscreen);
}

pub(super) fn resize_window_surface(
    window_state: &mut WindowState,
    device: &wgpu::Device,
) {
    let inner_size = window_state.window.inner_size();
    let scale_factor = window_state.window.scale_factor();
    let surface_size = round_surface_size_for_scale(inner_size, scale_factor);

    if surface_size.width > 0 && surface_size.height > 0 {
        window_state.size = surface_size;
        window_state.config.width = surface_size.width;
        window_state.config.height = surface_size.height;
        window_state.surface.configure(device, &window_state.config);
    }
}

pub(super) fn round_surface_size_for_scale(
    size: PhysicalSize<u32>,
    scale_factor: f64,
) -> PhysicalSize<u32> {
    let divisor = surface_scale_divisor(scale_factor);

    PhysicalSize::new(
        round_up_to_multiple(size.width, divisor),
        round_up_to_multiple(size.height, divisor),
    )
}

fn surface_scale_divisor(scale_factor: f64) -> u32 {
    if !scale_factor.is_finite() || scale_factor <= 1.0 {
        return 1;
    }

    scale_factor.ceil().max(1.0) as u32
}

fn round_up_to_multiple(value: u32, multiple: u32) -> u32 {
    if value == 0 || multiple <= 1 {
        return value;
    }

    let remainder = value % multiple;
    if remainder == 0 {
        value
    } else {
        value.saturating_add(multiple - remainder)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounds_odd_wayland_hidpi_height_to_scale_multiple() {
        let size = PhysicalSize::new(820, 45);

        let rounded = round_surface_size_for_scale(size, 2.0);

        assert_eq!(rounded, PhysicalSize::new(820, 46));
    }

    #[test]
    fn leaves_scale_one_and_zero_dimensions_unchanged() {
        assert_eq!(
            round_surface_size_for_scale(PhysicalSize::new(819, 45), 1.0),
            PhysicalSize::new(819, 45)
        );
        assert_eq!(
            round_surface_size_for_scale(PhysicalSize::new(0, 45), 2.0),
            PhysicalSize::new(0, 46)
        );
    }

    #[test]
    fn rounds_fractional_hidpi_to_next_integer_scale_multiple() {
        let size = PhysicalSize::new(100, 101);

        let rounded = round_surface_size_for_scale(size, 1.25);

        assert_eq!(rounded, PhysicalSize::new(100, 102));
    }

    #[test]
    fn window_options_default_to_decorated_windowed() {
        let options = WindowOptions::default();

        assert!(!options.borderless);
        assert!(!options.fullscreen);
        assert!(!options.maximized);
    }
}

#[derive(Default, Clone)]
pub struct AppWindowsWidgetState {
    window_app_map: HashMap<WindowId, AppType>,
    window_wake_state: HashMap<AppType, WindowWakeState>,
}

impl SettingsWidget for AppWindowsWidgetState {
    fn show(
        &mut self,
        ui: &mut egui::Ui,
        settings_ctx: &super::settings_menu::SettingsUiContext,
    ) -> super::settings_menu::SettingsUiResponse {
        let mut windows = self
            .window_wake_state
            .iter()
            .map(|(app_ty, state)| {
                let title = match app_ty {
                    AppType::Viewer1D => "1D Viewer".to_string(),
                    AppType::Viewer2D => "2D Viewer".to_string(),
                    AppType::Custom(name) => name.to_string(),
                };
                (title, app_ty.clone(), state)
            })
            .collect::<Vec<_>>();

        windows.sort_by(|(t1, _, _), (t2, _, _)| t1.cmp(t2));

        let resp = ui.horizontal(|ui| {
            //
            for (label, app_ty, wake_state) in windows {
                let active = wake_state.is_awake();
                let btn = egui::SelectableLabel::new(active, label);

                if ui.add(btn).clicked() {
                    if active {
                        // sleep
                        settings_ctx.send_app_msg_task(AppMsg::WindowDelta(
                            WindowDelta::Close(app_ty),
                        ));
                    } else {
                        // wake
                        settings_ctx.send_app_msg_task(AppMsg::WindowDelta(
                            WindowDelta::Open(app_ty),
                        ));
                    }
                }
            }
        });

        SettingsUiResponse {
            response: resp.response,
        }
    }
}

#[derive(Default)]
pub struct AppWindows {
    pub(super) windows: HashMap<WindowId, AppType>,
    pub(super) apps: HashMap<AppType, AppWindowState>,
    pub(super) sleeping: HashMap<AppType, AsleepWindow>,

    pub(super) widget_state: Arc<RwLock<AppWindowsWidgetState>>,
}

impl AppWindows {
    pub(super) fn update_widget_state(&self) {
        let mut state = self.widget_state.blocking_write();
        self.windows.clone_into(&mut state.window_app_map);

        for (app_ty, app) in self.apps.iter() {
            let wake_state = WindowWakeState::Awake(app.window.window.id());
            state.window_wake_state.insert(app_ty.clone(), wake_state);
        }

        for (app_ty, _app) in self.sleeping.iter() {
            let wake_state = WindowWakeState::Sleeping;
            state.window_wake_state.insert(app_ty.clone(), wake_state);
        }
    }

    pub(super) fn handle_window_delta(
        &mut self,
        event_loop: &EventLoopWindowTarget<()>,
        state: &raving_wgpu::State,
        options: WindowOptions,
        delta: WindowDelta,
    ) -> anyhow::Result<()> {
        match delta {
            WindowDelta::Open(app_ty) => {
                if self.apps.contains_key(&app_ty) {
                    return Ok(());
                }

                let asleep = self.sleeping.remove(&app_ty).ok_or(
                    anyhow::anyhow!("Can't wake a window that's not asleep"),
                )?;
                let state = asleep.wake(event_loop, state, options)?;

                self.windows
                    .insert(state.window.window.id(), app_ty.clone());
                self.apps.insert(app_ty, state);

                Ok(())
            }
            WindowDelta::Close(app_ty) => {
                if let Some(win_id) =
                    self.apps.get(&app_ty).map(|s| s.window.window.id())
                {
                    if self.windows.len() == 1 {
                        anyhow::bail!("Can't close the only open window!");
                    }

                    let _app_ty = self.windows.remove(&win_id);
                    let app = self.apps.remove(&app_ty).unwrap();
                    self.sleeping.insert(app_ty, app.sleep());
                }

                Ok(())
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum WindowDelta {
    Open(super::AppType),
    Close(super::AppType),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum WindowWakeState {
    // Uninitialized,
    Sleeping,
    Awake(WindowId),
}

impl WindowWakeState {
    pub fn is_awake(&self) -> bool {
        matches!(self, Self::Awake(_))
    }
}
