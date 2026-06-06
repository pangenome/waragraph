use waragraph::app::App;

use anyhow::Result;

pub fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Warn)
        // .filter_level(log::LevelFilter::Debug)
        .init();

    let args = waragraph::app::parse_args();

    if args.is_err() {
        let name = std::env::args().next().unwrap();
        println!(
            "Usage: {name} [--view-mode depth|path-name|strand] [--fullscreen] [--maximized] [--borderless] <gfa> [tsv]"
        );
        println!("4-column BED file can be provided using the --bed flag");
        println!("Press F11 in a viewer window to toggle fullscreen.");
        println!(
            "Wayland sessions use X11/XWayland by default when available so decorated title bars remain usable; set WINIT_UNIX_BACKEND=wayland to override."
        );
        std::process::exit(0);
    }

    let args = args?;

    waragraph::app::configure_default_window_backend();

    let (event_loop, state) =
        pollster::block_on(raving_wgpu::initialize_no_window())?;

    let mut app = App::init(&state, args)?;

    app.init_viewer_1d(&event_loop, &state)?;

    if app.shared.workspace.blocking_read().tsv_path().is_some() {
        app.init_viewer_2d(&event_loop, &state)?;
    }

    app.run(event_loop, state)
}
