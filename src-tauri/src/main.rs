// Protocol activation starts a short-lived second process on Windows. Using the GUI
// subsystem in debug builds too prevents that process from flashing a console window.
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod desktop;
mod archive;
mod download;
mod frp;
mod logging;
mod model;
mod p2p;
mod settings;
mod toolbox;

use desktop::{autodelay, deeplink, effects, theme, tray, window_state};
use p2p::{host, join};
use tauri::Manager;
use tauri_plugin_window_state::{StateFlags, WindowExt};

const AUTOSTART_ARGUMENT: &str = "--autostart";

#[cfg(all(target_os = "windows", debug_assertions))]
fn attach_parent_console() {
    use windows_sys::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};

    // The debug executable keeps the GUI subsystem to avoid protocol-activation flashes.
    // Attaching an existing parent console restores stdout without creating a new window.
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

fn window_state_flags() -> StateFlags {
    StateFlags::SIZE | StateFlags::POSITION | StateFlags::MAXIMIZED
}

#[tauri::command]
async fn stop_tunnel(app: tauri::AppHandle) -> Result<(), String> {
    match app.state::<p2p::P2pState>().active_mode() {
        Some(p2p::P2pMode::Host) => host::stop(&app),
        Some(p2p::P2pMode::Join) => join::stop(&app).await,
        None => Ok(()),
    }
}

#[tauri::command]
fn restart_application(app: tauri::AppHandle) {
    log::info!("restarting application immediately");
    tauri::process::restart(&app.env());
}

#[tauri::command]
fn frontend_ready(app: tauri::AppHandle) {
    log::debug!("frontend: ready");
    tray::show_when_ready(&app);
}

#[tauri::command]
fn frontend_page_loaded(page: String) {
    log::debug!("frontend: {page} loaded");
}

#[tauri::command]
fn read_text_file(path: String) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|error| error.to_string())
}

#[tauri::command]
fn write_text_file(path: String, content: String) -> Result<(), String> {
    std::fs::write(path, content).map_err(|error| error.to_string())
}

fn main() {
    #[cfg(all(target_os = "windows", debug_assertions))]
    attach_parent_console();

    let launched_by_autostart = std::env::args_os().any(|argument| argument == AUTOSTART_ARGUMENT);

    let app = tauri::Builder::default()
        .plugin(logging::plugin())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![AUTOSTART_ARGUMENT]),
        ))
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            deeplink::stash_restore_links(app, &args);
            tray::show_main_window(app);
            // The single-instance plugin handles this once before the callback. Repeat it
            // after revealing the window so an already-mounted WebView receives the event.
            use tauri_plugin_deep_link::DeepLinkExt;
            app.deep_link().handle_cli_arguments(args.iter());
        }))
        .manage(p2p::P2pState::new())
        .manage(host::HostState::new())
        .manage(join::JoinState::new())
        .manage(frp::FrpState::new())
        .manage(window_state::MainWindowState::new())
        .manage(autodelay::AutoDelay::new())
        .manage(deeplink::PendingDeepLinks::default())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(
            tauri_plugin_window_state::Builder::new()
                .with_state_flags(window_state_flags())
                .skip_initial_state("main")
                .build(),
        )
        .setup(move |app| {
            app.manage(settings::SettingsState::load(app.handle())?);
            let material = app.state::<settings::SettingsState>().window_material();
            let theme = app.state::<settings::SettingsState>().theme();
            log::info!("startup: {material}/{theme}");
            deeplink::setup(app)?;
            join::setup(app);
            if app
                .state::<settings::SettingsState>()
                .remembers_window_state()
                && let Some(window) = app.get_webview_window("main")
            {
                window.restore_state(window_state_flags())?;
            }
            if let Err(error) = theme::apply_material(app.handle(), &material, &theme, None) {
                log::error!("native material failed: {error}");
            }
            tray::setup(app)?;
            if launched_by_autostart
                && app.state::<settings::SettingsState>().starts_silently()
                && let Err(error) = tray::start_silently(app.handle())
            {
                log::error!("silent start failed: {error}");
                tray::show_main_window(app.handle());
            }
            Ok(())
        })
        .on_window_event(tray::handle_window_event)
        .invoke_handler(tauri::generate_handler![
            p2p::get_p2p_status,
            stop_tunnel,
            restart_application,
            frontend_ready,
            frontend_page_loaded,
            read_text_file,
            write_text_file,
            toolbox::run_network_diagnostics,
            toolbox::run_relay_diagnostics,
            host::start_lan_scan,
            host::get_lan_scan,
            host::restart_lan_scan,
            host::stop_lan_scan,
            host::probe_host_port,
            host::start_host,
            join::validate_invite,
            join::start_join,
            join::stop_join,
            frp::get_frp_client_status,
            frp::download_frp_client,
            frp::get_frp_session_status,
            frp::restore_frp_sessions,
            frp::login_sakurafrp,
            frp::login_openfrp,
            frp::open_sakura_keys,
            frp::open_sakura_purchase,
            frp::open_premium,
            frp::logout_frp,
            frp::list_frp_tunnels,
            frp::list_frp_nodes,
            frp::create_frp_tunnel,
            frp::edit_frp_tunnel,
            frp::delete_frp_tunnel,
            frp::start_frp_tunnel,
            frp::stop_frp_tunnel,
            settings::get_preferences,
            settings::get_system_fonts,
            settings::get_system_theme,
            effects::supports_liquid_glass,
            settings::set_theme,
            settings::set_color_theme,
            settings::set_locale,
            settings::set_invite_lifetime,
            settings::set_join_port,
            settings::set_personalization,
            settings::set_application_settings,
            settings::set_connection_settings,
            settings::set_lightweight_settings,
            deeplink::take_pending_links,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| match event {
        tauri::RunEvent::ExitRequested {
            api, code: None, ..
        } => api.prevent_exit(),
        tauri::RunEvent::Exit => {
            app_handle.state::<frp::FrpState>().stop_all();
            log::info!("exit");
        }
        _ => {}
    });
}
